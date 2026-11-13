# EVM golden fixtures

`execution.json` holds scenarios recorded on this workspace's revm EEZ EVM
(`crates/eez-prover-evm`): a genesis, a chain of RLP-encoded blocks, and for
each block either `invalid` or its post-state root, receipts root, logs bloom
and gas used.

Every EEZ EVM replays them and must reproduce every value exactly: this crate,
and eez-rollup0's node EVM (`crates/eez-evm`), which reads this file through its
`provers/` submodule.

```bash
cargo test -p eez-prover-evm --test golden
```

The scenarios cover native `0x76` mints (commit, revert rollback, overflow),
fee collection (positive, zero base fee, a self-destructing collector), mixed
system and user nonce chains, EIP-2935 history, contract creation, logs,
access lists and EIP-7702 delegation. The state root is computed over the full
post-state, so any difference in touched, created or deleted accounts shows up.

Only this crate records them. `base-genesis.json` is eez-rollup0's L2 genesis
reduced to its config and the EIP-2935 history contract; every scenario starts
from it.

```bash
cargo test -p eez-prover-evm --test golden -- --ignored regenerate
```
