#!/usr/bin/env python3
"""Check one prover's normal/build dependency closure from cargo metadata on stdin."""
import json
import sys

backend = sys.argv[1]
assert backend in ('stateful', 'stateless'), 'expected stateful or stateless'
metadata = json.load(sys.stdin)
packages = {p['id']: p for p in metadata['packages']}
nodes = {n['id']: n for n in metadata['resolve']['nodes']}


def closure(name):
    start = [p['id'] for p in packages.values() if p['name'] == name]
    assert len(start) == 1, f'expected one {name}, got {start}'
    seen, pending = set(), start
    while pending:
        package = pending.pop()
        if package in seen:
            continue
        seen.add(package)
        pending.extend(d['pkg'] for d in nodes[package]['deps']
                       if any(k['kind'] != 'dev' for k in d['dep_kinds']))
    return [packages[p] for p in seen]


graph = closure(f'eez-prover-{backend}')
names = {p['name'] for p in graph}
assert 'eez-proof-signer' in names
assert not names & {'eez-node', 'eez-composer'}
reth = {p['source'] for p in graph
        if p['name'].startswith('reth-') and (p['source'] or '').startswith('git+')}
if backend == 'stateless':
    assert {'eez-prover-evm', 'revm'} <= names
    assert not names & {'eez-evm', 'eez-follower', 'eez-prover-stateful'}
    assert not any(n.startswith('evm2') for n in names)
    assert {p['version'].split('.')[0] for p in graph if p['name'] == 'revm'} == {'43'}
    assert len(reth) == 1 and 'fa9e76ab4fd69d8323c3bbbbe43470f64aa47cb7' in next(iter(reth)), reth
    print('stateless: revm 43 / reth 2.7; no follower or evm2')
else:
    assert {'eez-follower', 'eez-evm', 'evm2'} <= names
    assert not names & {'revm', 'alloy-evm', 'eez-prover-evm', 'eez-prover-stateless'}
    assert len(reth) == 2, reth
    assert sum('reth?tag=v2.11.0#' in source for source in reth) == 1, reth
    assert sum('reth-core?rev=275ce4cf' in source for source in reth) == 1, reth
    follower = {p['name'] for p in closure('eez-follower')}
    assert not follower & {'eez-prover-stateful', 'eez-prover-stateless', 'eez-proof-signer'}
    print('stateful: default evm2 follower / reth 2.11; no revm or follower-to-prover dependency')
