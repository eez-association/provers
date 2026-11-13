# provers

Proof backends for [eez-rollup0](https://github.com/eez-association/eez-rollup0).
Each directory holds one prover with its own Cargo workspace, lockfile, reth
and EVM versions, Docker image and CI workflow.

| Directory | Prover |
| --- | --- |
| [`stateless-ecdsa/`](stateless-ecdsa) | The stateless proof signer (`eez-proof-signer`). It re-executes each settlement window from execution witnesses with its own EVM and signs it with an ECDSA attestation key. |

A prover talks to the node only through the `prove.v1`/`prove.v2` gRPC
contract. It uses the protocol crates from eez-rollup0 as git dependencies
pinned to one eez-rollup0 commit.
