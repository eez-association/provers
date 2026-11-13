# provers

Proof backends for [eez-rollup0](https://github.com/eez-association/eez-rollup0).
Each directory holds one prover with its own Cargo workspace, lockfile, reth
and EVM versions, Docker image and CI workflow.

A prover talks to the node only through the `prove.v1`/`prove.v2` gRPC
contract. It uses the protocol crates from eez-rollup0 as git dependencies
pinned to one eez-rollup0 commit.
