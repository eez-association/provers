//! Golden execution fixtures (`testing/evm-golden/execution.json`).
//!
//! Each scenario is a genesis plus a chain of blocks, recorded with the
//! post-state root, receipts root, logs bloom and gas used of every block (or
//! that the block is invalid). Every EEZ EVM replays the same blocks and must
//! reproduce every value exactly: this crate and eez-rollup0's `crates/eez-evm`,
//! which reads this file through its `provers/` submodule.
//!
//! This crate records them (revm):
//! `cargo test -p eez-prover-evm --test golden -- --ignored regenerate`.

use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use alloy_consensus::{
    BlockBody, EMPTY_ROOT_HASH, Header, SignableTransaction, TxEip1559, TxEip2930, TxEip7702,
    TxLegacy, TxReceipt, proofs::calculate_receipt_root,
};
use alloy_eips::{
    eip2930::{AccessList, AccessListItem},
    eip7685::EMPTY_REQUESTS_HASH,
    eip7702::Authorization,
};
use alloy_genesis::{Genesis, GenesisAccount};
use alloy_primitives::{
    Address, B256, Bloom, Bytes, KECCAK256_EMPTY, TxKind, U256, address, bytes, keccak256,
    logs_bloom,
};
use alloy_signer::SignerSync;
use alloy_signer_local::PrivateKeySigner;
use eez_primitives::{Block, EEZL2_ADDRESS, EezTxEnvelope, SYSTEM_ADDRESS, SystemTransaction};
use eez_prover_evm::EezEvmConfig;
use reth_chainspec::ChainSpec;
use reth_evm::{ConfigureEvm, execute::Executor};
use reth_primitives_traits::SealedBlock;
use revm::{
    database::{BundleState, InMemoryDB},
    state::{AccountInfo, Bytecode},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Fixtures {
    scenarios: Vec<Scenario>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Scenario {
    name: String,
    genesis: Genesis,
    /// RLP-encoded blocks, executed in order on the genesis state.
    blocks: Vec<Bytes>,
    expected: Vec<Outcome>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Outcome {
    /// The block fails execution; the state is left at the previous block.
    invalid: bool,
    state_root: B256,
    receipts_root: B256,
    logs_bloom: Bloom,
    gas_used: u64,
}

fn fixtures_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testing/evm-golden/execution.json")
}

#[test]
fn golden_blocks_replay_exactly() {
    let raw = std::fs::read_to_string(fixtures_path()).expect("read golden fixtures");
    let fixtures: Fixtures = serde_json::from_str(&raw).expect("parse golden fixtures");
    assert!(!fixtures.scenarios.is_empty());
    for scenario in &fixtures.scenarios {
        assert_eq!(
            replay(&scenario.genesis, &scenario.blocks),
            scenario.expected,
            "scenario {}",
            scenario.name
        );
    }
}

/// Executes `blocks` in order and returns each block's outcome.
fn replay(genesis: &Genesis, blocks: &[Bytes]) -> Vec<Outcome> {
    let config = EezEvmConfig::new(Arc::new(ChainSpec::from_genesis(genesis.clone()))).unwrap();
    let mut state = genesis.alloc.clone();
    blocks
        .iter()
        .map(|rlp| {
            let block = alloy_rlp::decode_exact::<Block>(rlp).expect("decode block");
            let block = SealedBlock::seal_slow(block)
                .try_recover()
                .expect("recover senders");
            let Ok(output) = config.executor(database(&state)).execute(&block) else {
                return Outcome {
                    invalid: true,
                    ..Default::default()
                };
            };
            apply(&mut state, &output.state);
            let receipts = &output.result.receipts;
            Outcome {
                invalid: false,
                state_root: alloy_trie::root::state_root_ref_unhashed(&state),
                receipts_root: calculate_receipt_root(
                    &receipts
                        .iter()
                        .map(TxReceipt::with_bloom_ref)
                        .collect::<Vec<_>>(),
                ),
                logs_bloom: logs_bloom(receipts.iter().flat_map(TxReceipt::logs)),
                gas_used: output.result.gas_used,
            }
        })
        .collect()
}

fn database(state: &BTreeMap<Address, GenesisAccount>) -> InMemoryDB {
    let mut db = InMemoryDB::default();
    for (address, account) in state {
        let code = account.code.clone().map(Bytecode::new_raw);
        db.insert_account_info(
            *address,
            AccountInfo {
                balance: account.balance,
                nonce: account.nonce.unwrap_or_default(),
                code_hash: code.as_ref().map_or(KECCAK256_EMPTY, Bytecode::hash_slow),
                code,
                ..Default::default()
            },
        );
        for (slot, value) in account.storage.iter().flatten() {
            db.insert_account_storage(
                *address,
                U256::from_be_bytes(slot.0),
                U256::from_be_bytes(value.0),
            )
            .unwrap();
        }
    }
    db
}

/// Applies one block's changes to the full state.
fn apply(state: &mut BTreeMap<Address, GenesisAccount>, bundle: &BundleState) {
    for (address, account) in &bundle.state {
        let Some(info) = &account.info else {
            state.remove(address);
            continue;
        };
        let entry = state.entry(*address).or_default();
        if account.was_destroyed() {
            entry.storage = None;
        }
        entry.balance = info.balance;
        entry.nonce = (info.nonce != 0).then_some(info.nonce);
        if info.code_hash == KECCAK256_EMPTY {
            entry.code = None;
        } else if entry.code.as_ref().map(keccak256) != Some(info.code_hash) {
            let code = info
                .code
                .clone()
                .or_else(|| bundle.contracts.get(&info.code_hash).cloned())
                .expect("changed code is in the bundle");
            entry.code = Some(code.original_bytes());
        }
        let storage = entry.storage.get_or_insert_with(BTreeMap::new);
        for (slot, value) in &account.storage {
            let slot = B256::from(*slot);
            if value.present_value.is_zero() {
                storage.remove(&slot);
            } else {
                storage.insert(slot, B256::from(value.present_value));
            }
        }
        if storage.is_empty() {
            entry.storage = None;
        }
    }
}

// ── Recording ────────────────────────────────────────────────────────────

const BENEFICIARY: Address = address!("00000000000000000000000000000000000000be");
/// `CALLER PUSH1 0 SSTORE PUSH1 32 PUSH1 0 LOG0 STOP`: a write and a log.
const STORE_LOG: Address = address!("00000000000000000000000000000000000000c0");
/// `PUSH1 0 PUSH1 0 REVERT`.
const REVERTER: Address = address!("00000000000000000000000000000000000000c1");
const FRESH: Address = address!("00000000000000000000000000000000000000f1");

fn key(byte: u8) -> PrivateKeySigner {
    PrivateKeySigner::from_bytes(&B256::repeat_byte(byte)).unwrap()
}

fn account(balance: u64) -> GenesisAccount {
    GenesisAccount {
        balance: U256::from(balance) * U256::from(10u64.pow(18)),
        ..Default::default()
    }
}

fn contract(code: Bytes) -> GenesisAccount {
    GenesisAccount {
        code: Some(code),
        ..Default::default()
    }
}

/// The eez-rollup0 L2 genesis config (all forks through Osaka at genesis,
/// `feeCollector` set) and its EIP-2935 history contract, plus `accounts`.
fn genesis(accounts: impl IntoIterator<Item = (Address, GenesisAccount)>) -> Genesis {
    let base: Genesis = serde_json::from_str(include_str!(
        "../../../testing/evm-golden/base-genesis.json"
    ))
    .unwrap();
    let history = address!("0000f90827f1c53a10cb7a02335b175320002935");
    let mut alloc = BTreeMap::from([(history, base.alloc[&history].clone())]);
    alloc.extend([
        (key(0xa1).address(), account(100)),
        (key(0xb2).address(), account(100)),
        (STORE_LOG, contract(bytes!("3360005560206000a000"))),
        (REVERTER, contract(bytes!("60006000fd"))),
        (EEZL2_ADDRESS, contract(bytes!("3360005500"))),
    ]);
    alloc.extend(accounts);
    Genesis { alloc, ..base }
}

fn fee_collector(genesis: &Genesis) -> Address {
    serde_json::from_value(genesis.config.extra_fields["feeCollector"].clone()).unwrap()
}

fn block(genesis: &Genesis, number: u64, base_fee: u64, txs: Vec<EezTxEnvelope>) -> Bytes {
    let header = Header {
        number,
        timestamp: genesis.timestamp + 12 * number,
        gas_limit: 30_000_000,
        base_fee_per_gas: Some(base_fee),
        beneficiary: BENEFICIARY,
        parent_hash: keccak256(number.to_be_bytes()),
        // An L2 block's shape since Prague: empty withdrawals and requests.
        withdrawals_root: Some(EMPTY_ROOT_HASH),
        excess_blob_gas: Some(0),
        blob_gas_used: Some(0),
        parent_beacon_block_root: Some(B256::ZERO),
        requests_hash: Some(EMPTY_REQUESTS_HASH),
        ..Default::default()
    };
    let body = BlockBody {
        transactions: txs,
        withdrawals: Some(Default::default()),
        ..Default::default()
    };
    alloy_rlp::encode(Block::new(header, body)).into()
}

fn system(chain_id: u64, nonce: u64, value: u64) -> EezTxEnvelope {
    SystemTransaction {
        chain_id,
        nonce,
        to: EEZL2_ADDRESS,
        value: U256::from(value),
        input: Bytes::new(),
    }
    .into()
}

fn sign<T: SignableTransaction<alloy_primitives::Signature>>(
    signer: &PrivateKeySigner,
    tx: T,
) -> alloy_consensus::Signed<T> {
    let signature = signer.sign_hash_sync(&tx.signature_hash()).unwrap();
    tx.into_signed(signature)
}

fn call(chain_id: u64, signer: &PrivateKeySigner, nonce: u64, to: TxKind) -> EezTxEnvelope {
    sign(
        signer,
        TxEip1559 {
            chain_id,
            nonce,
            gas_limit: 200_000,
            max_fee_per_gas: 100,
            max_priority_fee_per_gas: 2,
            to,
            ..Default::default()
        },
    )
    .into()
}

/// Each scenario with the validity every block must have.
fn scenarios() -> Vec<(Scenario, Vec<bool>)> {
    let alice = key(0xa1);
    let bob = key(0xb2);
    let base = genesis([]);
    let chain = base.config.chain_id;
    let collector = fee_collector(&base);
    let mut out = Vec::new();
    let mut push = |name: &str, genesis: Genesis, blocks: Vec<Bytes>, valid: Vec<bool>| {
        out.push((
            Scenario {
                name: name.into(),
                genesis,
                blocks,
                expected: Vec::new(),
            },
            valid,
        ));
    };

    let g = genesis([]);
    let blocks = (1..=3).map(|n| block(&g, n, 7, vec![])).collect();
    push("empty_blocks_record_history", g, blocks, vec![true; 3]);

    let g = genesis([]);
    let blocks = vec![
        block(&g, 1, 7, vec![system(chain, 0, 13)]),
        block(&g, 2, 7, vec![system(chain, 1, 0)]),
    ];
    push("native_mint_commits", g, blocks, vec![true, true]);

    let g = genesis([(EEZL2_ADDRESS, contract(bytes!("3360005560006000fd")))]);
    let blocks = vec![block(&g, 1, 7, vec![system(chain, 0, 13)])];
    push("native_mint_rolls_back_on_revert", g, blocks, vec![true]);

    let g = genesis([(
        SYSTEM_ADDRESS,
        GenesisAccount {
            balance: U256::MAX - U256::from(5),
            ..Default::default()
        },
    )]);
    let blocks = vec![block(&g, 1, 7, vec![system(chain, 0, 13)])];
    push("native_mint_overflow_rejects_block", g, blocks, vec![false]);

    let g = genesis([]);
    let access_list = AccessList(vec![AccessListItem {
        address: STORE_LOG,
        storage_keys: vec![B256::ZERO],
    }]);
    let blocks = vec![block(
        &g,
        1,
        7,
        vec![
            call(chain, &alice, 0, TxKind::Call(STORE_LOG)),
            sign(
                &bob,
                TxEip2930 {
                    chain_id: chain,
                    nonce: 0,
                    gas_price: 100,
                    gas_limit: 200_000,
                    to: TxKind::Call(STORE_LOG),
                    access_list,
                    ..Default::default()
                },
            )
            .into(),
            sign(
                &alice,
                TxEip1559 {
                    chain_id: chain,
                    nonce: 1,
                    gas_limit: 200_000,
                    max_fee_per_gas: 100,
                    max_priority_fee_per_gas: 2,
                    to: TxKind::Create,
                    input: bytes!("3360005560206000a000"),
                    ..Default::default()
                },
            )
            .into(),
            sign(
                &bob,
                TxLegacy {
                    chain_id: Some(chain),
                    nonce: 1,
                    gas_price: 100,
                    gas_limit: 21_000,
                    to: TxKind::Call(FRESH),
                    value: U256::from(5),
                    ..Default::default()
                },
            )
            .into(),
            call(chain, &alice, 2, TxKind::Call(REVERTER)),
        ],
    )];
    push("fees_logs_creation_access_lists", g, blocks, vec![true]);

    let g = genesis([]);
    let blocks = vec![block(
        &g,
        1,
        0,
        vec![call(chain, &alice, 0, TxKind::Call(STORE_LOG))],
    )];
    push(
        "zero_base_fee_leaves_collector_untouched",
        g,
        blocks,
        vec![true],
    );

    // `COINBASE SELFDESTRUCT`: the collector sends its balance away mid-block.
    let g = genesis([(
        collector,
        GenesisAccount {
            code: Some(bytes!("41ff")),
            ..account(1)
        },
    )]);
    let blocks = vec![block(
        &g,
        1,
        7,
        vec![
            call(chain, &alice, 0, TxKind::Call(collector)),
            call(chain, &bob, 0, TxKind::Call(STORE_LOG)),
        ],
    )];
    push("self_destructing_collector", g, blocks, vec![true]);

    let g = genesis([]);
    let blocks = vec![block(
        &g,
        1,
        7,
        vec![
            system(chain, 0, 1),
            call(chain, &alice, 0, TxKind::Call(STORE_LOG)),
            system(chain, 1, 2),
            call(chain, &alice, 1, TxKind::Call(REVERTER)),
            call(chain, &bob, 0, TxKind::Call(STORE_LOG)),
        ],
    )];
    push(
        "nonce_chain_mixes_system_and_user_transactions",
        g,
        blocks,
        vec![true],
    );

    let g = genesis([]);
    let authorization = Authorization {
        chain_id: U256::from(chain),
        address: STORE_LOG,
        nonce: 0,
    };
    let signature = alice
        .sign_hash_sync(&authorization.signature_hash())
        .unwrap();
    let blocks = vec![
        block(
            &g,
            1,
            7,
            vec![
                sign(
                    &bob,
                    TxEip7702 {
                        chain_id: chain,
                        nonce: 0,
                        gas_limit: 200_000,
                        max_fee_per_gas: 100,
                        max_priority_fee_per_gas: 2,
                        to: alice.address(),
                        authorization_list: vec![authorization.into_signed(signature)],
                        ..Default::default()
                    },
                )
                .into(),
            ],
        ),
        block(
            &g,
            2,
            7,
            vec![call(chain, &bob, 1, TxKind::Call(alice.address()))],
        ),
    ];
    push("eip7702_delegation", g, blocks, vec![true, true]);
    out
}

#[test]
#[ignore = "records testing/evm-golden/execution.json on the revm EVM"]
fn regenerate() {
    let scenarios = scenarios()
        .into_iter()
        .map(|(mut scenario, valid)| {
            scenario.expected = replay(&scenario.genesis, &scenario.blocks);
            let actual = scenario
                .expected
                .iter()
                .map(|o| !o.invalid)
                .collect::<Vec<_>>();
            assert_eq!(actual, valid, "scenario {} validity", scenario.name);
            scenario
        })
        .collect();
    let json = serde_json::to_string_pretty(&Fixtures { scenarios }).unwrap();
    std::fs::write(fixtures_path(), json + "\n").unwrap();
}
