# darkfi-inspect demo

`darkfi-inspect` is a developer-oriented CLI for observing, inspecting, and diagnosing DarkFi nodes and related network state.

The examples below are based on behavior tested against live DarkFi services. Live-state values such as heights, hashes, peer counts, and EventGraph counts are historical snapshots and may change.

## Live DarkFi testnet

The examples below are based on a live DarkFi testnet node with the following peers connected at the time of testing:

```text
Peer 1: tcp+tls://neo.not.org:18340/
Peer 2: tcp+tls://node1.testnet.dark.fi:18340/
Peer 3: tcp+tls://195.3.221.59:18340/
Peer 4: tcp+tls://node0.testnet.dark.fi:18340/
```
The peer list is a snapshot from the time of testing and may change as nodes connect or disconnect.

## CLI quick reference

The main commands used with `darkfi-inspect` are:

```text
$ ./target/debug/darkfi-inspect ping
$ ./target/debug/darkfi-inspect peers
$ ./target/debug/darkfi-inspect status
$ ./target/debug/darkfi-inspect events
$ ./target/debug/darkfi-inspect diagnose
$ ./target/debug/darkfi-inspect inspect block <HEIGHT>
$ ./target/debug/darkfi-inspect inspect tx <TX_HASH>
```
Use `--json` with supported commands when structured output is needed:

```text
$ ./target/debug/darkfi-inspect --json diagnose
$ ./target/debug/darkfi-inspect inspect --json block <HEIGHT>
$ ./target/debug/darkfi-inspect inspect --json tx <TX_HASH>
```

For the complete command list:

```text
$ ./target/debug/darkfi-inspect --help
```

## Inspect a healthy block

```text
$ ./target/debug/darkfi-inspect inspect block 44424

Block 44424
  Hash:     243e4abb77d5b68ac023c1e88d394787c2f6bba1ec0a36b7f4524771e221124b
  Previous: 916bb94d58fcb25a2fbb50d5d489399718b5518d6985e98996c94e077b41b2ef
  Txs:      1

  block_height         [PASS] [CONFIRMED] block reports requested height 44424
  chain_linkage        [PASS] [CONFIRMED] previous hash matches block 44423
  timestamp_sanity     [PASS] [CONFIRMED] timestamp satisfies DarkFi rules
```

The block inspector verifies that the returned block matches the requested height, links to its predecessor, and applies the current DarkFi timestamp sanity rules.

## Inspect a suspicious block

```text
$ ./target/debug/darkfi-inspect inspect block 1

Block 1
  Hash:     da23295ecdfd12e868df6d78c3a2b0e06ecf70d7115bbd57da924ed008af0bfd
  Previous: 6612ed20b3cd85b5d5e0cf1f5f50c7cf9860853da36a0a306c19292e38fa6848
  Txs:      1

  chain_linkage        [PASS] [CONFIRMED] previous hash matches block 0
  timestamp_sanity     [FAIL] [HIGH] timestamp violates the current DarkFi timestamp sanity rules
```

The timestamp check applies DarkFi's current timestamp rules directly and reports the result with an explicit confidence level.

## Inspect a transaction

Transactions can be retrieved directly by their transaction hash:

```text
$ ./target/debug/darkfi-inspect inspect tx 8daf70f08516698100048c2280a7259f9b45a7c9c61938db0ceca3649d3668e9

Transaction 8daf70f08516698100048c2280a7259f9b45a7c9c61938db0ceca3649d3668e9
  Calls:      1
  Proof sets: 1
  Sig sets:   1
  Call 0
    Contract:  BZHKGQ26bzmBithTQYTJtjo2QdCqpkR9tjSBopT4yf4o
    Function:  2
    Data:      534 byte(s)
    Parent:    None
    Children:  []
  tx_hash              [PASS] [CONFIRMED] transaction hash matches the requested hash
  call_tree            [PASS] [CONFIRMED] transaction call forest is structurally valid (1 call(s))
  calls_proofs         [PASS] [CONFIRMED] call count matches proof-group count (1)
  calls_signatures     [PASS] [CONFIRMED] call count matches signature-group count (1)
```
The transaction inspector reports the real DarkFi `Transaction` returned by `blockchain.get_tx`, including call structure, proof/signature group counts, and DarkFi's canonical call-forest integrity check.

The `call_tree` check validates the serialized `DarkLeaf` forest structure using DarkFi's own `dark_forest_leaf_vec_integrity_check` implementation. The inspector does not attempt to decode arbitrary contract-specific calldata or claim that attached ZK proofs or signatures are cryptographically valid without the additional verification context required by DarkFi.

## Diagnose the node

`diagnose` combines information from multiple DarkFi services into one diagnostic snapshot instead of requiring the developer to query each subsystem separately.

It combines individual checks into an overall verdict and surfaces actionable findings when a check fails or cannot be established.

The following is a representative healthy live snapshot:

```text
$ ./target/debug/darkfi-inspect diagnose

DarkFi node diagnostic
Diagnosis: HEALTHY

  block_target         [PASS] [CONFIRMED] consensus block target: 120
  chain                [PASS] [CONFIRMED] last confirmed block: 45757 (09104206...57fef244)
  chain_tip            [PASS] [CONFIRMED] last confirmed block 45757 matches fetched block hash
  chain_linkage        [PASS] [CONFIRMED] previous hash matches block 45756
  best_fork            [PASS] [CONFIRMED] best fork next height: 45763
  chain_depth          [PASS] [CONFIRMED] confirmation depth is 5 blocks
  peers                [PASS] [CONFIRMED] 3 peer(s) connected
  rpc                  [PASS] [CONFIRMED] RPC responsive (1 ms)
  eventgraph_parents   [PASS] [CONFIRMED] 410 events, 387 non-null parent references, all parents resolved
  eventgraph_rotation  [PASS] [CONFIRMED] 24 consecutive hourly rotation timestamps present through the current rotation
  eventgraph_epoch     [PASS] [CONFIRMED] 24 genesis timestamps aligned to the canonical DarkIRC hourly epoch
  eventgraph_current   [PASS] [CONFIRMED] latest genesis matches canonical current rotation 1786734000000
  eventgraph_genesis   [PASS] [CONFIRMED] 24 rotating genesis IDs match the canonical DarkIRC genesis identity

No obvious issues detected.

Summary: 13 passed, 0 failed, 0 unknown
```

The diagnostic currently checks:

* consensus block target
* chain position and last confirmed block
* confirmed chain-tip hash
* chain linkage to the previous block
* best fork position
* observed confirmation depth
* connected peers
* RPC responsiveness
* EventGraph parent-reference closure
* consecutive hourly EventGraph rotation periods
* canonical DarkIRC EventGraph epoch alignment
* current EventGraph rotation against wall-clock time
* EventGraph genesis identity against the canonical DarkIRC construction

The EventGraph checks use the existing DarkIRC `eventgraph.get_info` RPC. `darkfi-inspect` does not modify DarkFi itself; it consumes the information already exposed by the running service.

## EventGraph diagnostics

The EventGraph snapshot can contain hundreds of events across multiple rotating DAGs.

A useful diagnostic property is parent closure: every non-null parent reference in the returned snapshot should resolve to another event in that snapshot.

For a healthy live snapshot:

```text
  eventgraph_parents [PASS] [CONFIRMED] 410 events, 387 non-null parent references, all parents resolved
```

The tool intentionally does not treat event count or layer density as a health invariant. EventGraph state changes naturally as DAGs rotate and old history is pruned.

The rotation check looks for consecutive hourly genesis timestamps in the current rolling window:

```text
  eventgraph_rotation [PASS] [CONFIRMED] 24 consecutive hourly rotation timestamps present through the current rotation
```

This is intended to catch a missing rotation period without requiring the developer to manually inspect the raw EventGraph response.

The epoch check verifies that layer-0 genesis timestamps are aligned to the canonical DarkIRC EventGraph epoch: the configured genesis origin and hourly rotation boundaries.

For a healthy live snapshot:

```text
  eventgraph_epoch [PASS] [CONFIRMED] 24 genesis timestamps aligned to the canonical DarkIRC hourly epoch
```

The current-rotation check compares the latest layer-0 genesis timestamp in the snapshot against the rotation boundary implied by the current wall-clock time.

For a healthy live snapshot:

```text
  eventgraph_current [PASS] [CONFIRMED] latest genesis matches canonical current rotation 1786734000000
```

The genesis-identity check independently reconstructs the expected EventGraph genesis ID from the canonical DarkIRC genesis construction and compares it with the IDs returned by the node.

For a healthy live snapshot:

```text
  eventgraph_genesis [PASS] [CONFIRMED] 24 rotating genesis IDs match the canonical DarkIRC genesis identity
```

Together with the epoch check, this verifies not only that genesis timestamps are correctly aligned, but that the corresponding EventGraph IDs are consistent with the canonical DarkIRC identity construction.

Together, these checks distinguish between different kinds of EventGraph problems:

* **parent closure** — references in the returned DAG resolve correctly
* **rotation continuity** — expected hourly rotation periods are present
* **epoch alignment** — genesis timestamps follow the canonical DarkIRC epoch
* **current rotation** — the latest observed genesis is aligned with the rotation that should currently be active
* **genesis identity** — rotating genesis IDs match the canonical DarkIRC genesis construction

## JSON output

All diagnostic results can also be emitted as structured JSON:

$ ./target/debug/darkfi-inspect --json diagnose

```json
{
  "checks": [
    {
      "confidence": "Confirmed",
      "message": "consensus block target: 120",
      "name": "block_target",
      "state": "Pass"
    },
    {
      "confidence": "Confirmed",
      "message": "last confirmed block: ...",
      "name": "chain",
      "state": "Pass"
    },
    {
      "confidence": "Confirmed",
      "message": "last confirmed block ... matches fetched block hash",
      "name": "chain_tip",
      "state": "Pass"
    },
    {
      "confidence": "Confirmed",
      "message": "previous hash matches block ...",
      "name": "chain_linkage",
      "state": "Pass"
    },
    {
      "confidence": "Confirmed",
      "message": "best fork next height: ...",
      "name": "best_fork",
      "state": "Pass"
    },
    {
      "confidence": "Confirmed",
      "message": "confirmation depth is 5 blocks",
      "name": "chain_depth",
      "state": "Pass"
    },
    {
      "confidence": "Confirmed",
      "message": "... peer(s) connected",
      "name": "peers",
      "state": "Pass"
    },
    {
      "confidence": "Confirmed",
      "message": "RPC responsive (... ms)",
      "name": "rpc",
      "state": "Pass"
    },
    {
      "confidence": "Confirmed",
      "message": "... events, ... non-null parent references, all parents resolved",
      "name": "eventgraph_parents",
      "state": "Pass"
    },
    {
      "confidence": "Confirmed",
      "message": "24 consecutive hourly rotation timestamps present through the current rotation",
      "name": "eventgraph_rotation",
      "state": "Pass"
    },
    {
      "confidence": "Confirmed",
      "message": "... genesis timestamps aligned to the canonical DarkIRC hourly epoch",
      "name": "eventgraph_epoch",
      "state": "Pass"
    },
    {
      "confidence": "Confirmed",
      "message": "latest genesis matches canonical current rotation ...",
      "name": "eventgraph_current",
      "state": "Pass"
    },
    {
      "confidence": "Confirmed",
      "message": "... rotating genesis IDs match the canonical DarkIRC genesis identity",
      "name": "eventgraph_genesis",
      "state": "Pass"
    }
  ],
  "verdict": "HEALTHY",
  "findings": [],
  "summary": {
    "failed": 0,
    "passed": 13,
    "unknown": 0
  }
}
```

The JSON form is intended for scripts, monitoring, CI, and future tooling built on top of `darkfi-inspect`.

`verdict` provides the overall diagnostic state, while `findings` contains higher-level descriptions of failed or unknown checks. When all checks pass, `findings` is empty.

## Current direction

The project is moving beyond simple RPC observation toward developer diagnostics.

The guiding idea is not to replace the explorer or reproduce logs. The goal is to correlate information from different DarkFi subsystems and answer a more useful question:

> Does what I'm seeing make sense, and where should I look next?

Checks are added when they can be grounded in data returned by DarkFi and verified against real node behavior. The tool should prefer concrete evidence and useful diagnostics over speculative warnings.
