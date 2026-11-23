# provers

Proof backends for [eez-rollup0](https://github.com/eez-association/eez-rollup0).
Each directory holds one prover with its own Cargo workspace, lockfile, reth
and EVM versions, Docker image and CI workflow.

| Directory | Prover |
| --- | --- |
| [`stateless-ecdsa/`](stateless-ecdsa) | The stateless proof signer (`eez-proof-signer`). It re-executes each settlement window from execution witnesses with its own EVM and signs it with an ECDSA attestation key. |
| [`stateful-ecdsa/`](stateful-ecdsa) | The stateful proof signer (`eez-prover-stateful`). It runs the default evm2 follower and signs against its local canonical L2 database. |

The Composer talks to either prover through the `prove.v1`/`prove.v2` gRPC
contract. Both use the shared `eez-proof-signer` library and protocol crates
from eez-rollup0 as pinned Git dependencies. Stateful also imports the follower
and its execution stack. Stateless retains revm 43/reth 2.7; stateful uses
evm2/reth 2.11. Each workspace can upgrade its execution dependencies separately.
The follower has no dependency on either prover.
