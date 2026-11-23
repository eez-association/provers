# eez-prover-stateful

This crate is a node-backed implementation of the `prove.v1.Prover/ProveStream`
service. The `eez-prover-stateful` executable embeds the shared L1-derived follower
with its default `eez-evm` (evm2) configuration, so it reads its canonical L2 state directly.

The implementation imports `eez-proof-signer`. Both backends use
the same request admission, settlement gates, public-input recomputation, and
attestation code. The comparison is therefore limited to execution evidence:

| Stateless signer | Stateful signer |
| --- | --- |
| Replays each block from its streamed witness | Imports blocks through Reth and reuses local execution evidence |
| Trust input is an operator-supplied chain document | Trust input is the follower's chain spec and canonical database |
| Runs as a standalone daemon | Runs as a separately addressable gRPC service inside the follower process |

`Begin` checks the exact L1-derived safe anchor. `Block` imports candidate
execution through the follower's Reth engine and binds its retained block,
receipts and parent state. Sync checkpoints are replayed and cached when
needed; ordinary blocks reuse normal engine execution.

`Finalize` selects the checked terminal under the follower's reconciliation
lock. Reth enforces safe/finalized ancestry, and a rejected or unavailable
selection cannot produce a proof. Imported execution stays in Reth; sessions
retain only references and supplemental evidence.

## Running

Run `eez-prover-stateful node ...` with the same reth chain, datadir and network
arguments as a follower. `EEZ_STATEFUL_PROOF_SIGNER_ADDR` and
`EEZ_STATEFUL_PROOF_SIGNER_KEY` are required. `EEZ_SEQUENCER_ADDRESS` is rejected:
proving follows L1-derived heads. Historical peer synchronization remains
supported; `EEZ_SYNC_L1_ONLY=1` is optional. An external L1 RPC is sufficient,
with trace RPC and beacon access when the posted data requires them.

The ordinary `eez-follower` no longer hosts signing. Reuse its datadir only with
compatible reth/database versions; the stateful workspace pins reth 2.11/evm2 independently of the stateless workspace.

The service uses the deployment settings shared with the stateless signer:
`EEZ_ROLLUP_ID`, `EEZ_VKEY`, `EEZ_PROOF_SYSTEM`, `EEZ_ATTESTER_ADDRESS`,
and `EEZ_L2_SYSTEM_ADDRESS` (the reserved native system address). Its attestation
secret uses a distinct name, `EEZ_STATEFUL_PROOF_SIGNER_KEY`, so enabling it does not put the
node into Composer mode.

The current `prove.v1` stream requires execution witnesses under the shared wire
contract. They remain subject to admission quotas but this backend ignores
their contents.
