# Stateful ECDSA prover

This workspace runs `eez-prover-stateful`: the default L1-derived follower plus
an ECDSA signing service over its local canonical L2 database. It uses the
existing evm2 stateful backend from eez-rollup0.

The follower, `eez-evm`, and shared `eez-proof-signer` library are Git dependencies
pinned to the same eez-rollup0 revision. The follower has no prover dependency;
its startup hook supplies the provider, chain spec, committer, and task executor.
There is no EVM selection or custom follower implementation.

This workspace owns its reth 2.11/evm2 pins and lockfile. The sibling
`stateless-ecdsa/` workspace retains reth 2.7/revm 43 and can upgrade separately.

```bash
cd stateful-ecdsa
cargo build --locked --bin eez-prover-stateful
cargo nextest run --locked --workspace
cargo metadata --locked --format-version 1 | python3 ../scripts/check-execution-deps.py stateful
```

Build the image from the repository root with
`docker build -t eez-prover-stateful stateful-ecdsa`.

See [the crate documentation](crates/eez-prover-stateful/README.md) for service
behavior, configuration, and L1 endpoint requirements. Run
`eez-prover-stateful node ...` with follower chain/datadir arguments and required
`EEZ_STATEFUL_PROOF_SIGNER_ADDR` and `EEZ_STATEFUL_PROOF_SIGNER_KEY` settings.
