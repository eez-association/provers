# stateless-ecdsa

The stateless proof signer. For each settlement window the composer streams the
blocks, their execution witnesses and the postBatch over `prove.v2`. The signer
re-executes every block from its witness, checks the settlement, and signs the
batch's public inputs with its ECDSA attestation key for `ECDSAProofSystem`.

It runs reth v2.7.0 (`fa9e76a`), revm 43 and the stateless fork
(`eez-association/stateless@935f25a`), independently of the reth and EVM the
node links, so it cross-checks the node's execution with a second EVM.

## Layout

```
Cargo.toml          # workspace: dependencies, reth [patch], profiles, lints
Cargo.lock
deny.toml           # cargo-deny bans and sources
.cargo/audit.toml   # cargo-audit accepted advisories
Dockerfile          # the eez-proof-signer image
crates/
  eez-prover-stateless/  # stateless backend + the `eez-proof-signer` binary
  eez-prover-evm/        # the EVM the signer runs (revm copy of eez-rollup0's crates/eez-evm)
testing/evm-golden/      # golden execution fixtures, recorded here
```

## Crates from eez-rollup0

The signer pipeline itself (gRPC service, window validation, settlement checks,
attestation) is eez-rollup0's `eez-proof-signer` library. The stateless backend
here plugs into it, as does the stateful backend in eez-rollup0. This workspace
takes it and the other protocol crates as git dependencies on one eez-rollup0
commit: `eez-primitives`, `eez-protocol`, `eez-proof-signer`,
`eez-payload-codec`, `eez-control-rpc`, `eez-prover` and `eez-prover-client`
(`eez-blob` comes through them).

- Those crates name no git-pinned reth, EVM or stateless crate, so they build
  against this workspace's reth v2.7.0 graph. They inherit their dependency
  requirements from eez-rollup0's root manifest.
- `eez-primitives` keeps the revm `TxEnv` conversions behind its `revm` feature,
  which this workspace enables.
- To move to a newer eez-rollup0, change every `rev` together, then run
  `cargo check` without `--locked` to update `Cargo.lock`.

## Keeping in step with the node

- A change to the EEZ execution rules (0x76 native transactions, the fee
  collector, the block executor) lands in eez-rollup0's `crates/eez-evm` and in
  `crates/eez-prover-evm` together.
- Both EVMs replay `testing/evm-golden/execution.json` and must match it
  exactly. eez-rollup0 keeps a byte-identical copy; only this workspace records.
- `crates/eez-prover-stateless/tests/conformance/` holds copies of eez-rollup0's
  `prove.v2` conformance vectors and replayer
  (`crates/eez-control-rpc/conformance/`), replayed against the real binary.
  Refresh them when moving to a newer eez-rollup0.

## Building and testing

```bash
cargo build --locked -p eez-prover-stateless --bin eez-proof-signer
cargo nextest run --locked --workspace
docker build -t eez-proof-signer .
```

Configuration and the backend layout are in
[`crates/eez-prover-stateless/README.md`](crates/eez-prover-stateless/README.md)
and [`.env.example`](crates/eez-prover-stateless/.env.example).
