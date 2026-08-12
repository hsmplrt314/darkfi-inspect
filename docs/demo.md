# darkfi-inspect demo

`darkfi-inspect` is a developer-oriented CLI for observing, inspecting, and diagnosing DarkFi nodes and related network state.

The examples below are based on behavior tested against live DarkFi services.

## Inspect a healthy block

```text
$ ./target/debug/darkfi-inspect inspect block 44424

Block 44424
  Hash:     243e4abb77d5b68ac023c1e88d394787c2f6bba1ec0a36b7f4524771e221124b
  Previous: 916bb94d58fcb25a2fbb50d5d489399718b5518d6985e98996c94e077b41b2ef
  Txs:      1

block_height: OK [CONFIRMED] block reports requested height 44424
chain_linkage: OK [CONFIRMED] previous hash matches block 44423
timestamp_sanity: OK [MEDIUM] 103s since block 44423 — within plausible range (target: 120s)
```

The block inspector verifies that the returned block matches the requested height, links to its predecessor, and has a timestamp gap within the current heuristic range.

## Inspect a suspicious block

```text
$ ./target/debug/darkfi-inspect inspect block 1

Block 1
  Hash:     da23295ecdfd12e868df6d78c3a2b0e06ecf70d7115bbd57da924ed008af0bfd
  Previous: 6612ed20b3cd85b5d5e0cf1f5f50c7cf9860853da36a0a306c19292e38fa6848
  Txs:      1

chain_linkage: OK [CONFIRMED] previous hash matches block 0
timestamp_sanity: MISMATCH [MEDIUM] 2645s gap since block 0 is unusually large (target: 120s)
```

The important distinction is that the tool reports the observation and its confidence instead of automatically presenting every anomaly as a proven consensus failure.

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
tx_hash: OK [CONFIRMED] transaction hash matches the requested hash
calls_proofs: OK [CONFIRMED] call count matches proof-group count (1)
calls_signatures: OK [CONFIRMED] call count matches signature-group count (1)
```

The transaction inspector reports the real DarkFi `Transaction` returned by `blockchain.get_tx`, including call structure, proof/signature group counts, and structural consistency checks.

It does not attempt to decode arbitrary contract-specific calldata or claim that ZK proofs or signatures are cryptographically valid without the additional verification context required by DarkFi.

## Diagnose the node

`diagnose` combines information from multiple DarkFi services into one diagnostic snapshot instead of requiring the developer to query each subsystem separately.

```text
$ ./target/debug/darkfi-inspect diagnose

DarkFi node diagnostic
chain: OK [CONFIRMED] last confirmed block: 44559 (6a040708a5a6e30d24170613e928241e98262b9fd6ad8c9ab889a367f4f6a09c)
best_fork: OK [CONFIRMED] best fork next height: 44565
chain_depth: OK [CONFIRMED] confirmation depth is 5 blocks
peers: OK [CONFIRMED] 3 peer(s) connected
rpc: OK [CONFIRMED] RPC responsive (4 ms)
eventgraph_parents: OK [CONFIRMED] 346 events, 324 non-null parent references, all parents resolved
eventgraph_rotation: OK [CONFIRMED] 24 consecutive hourly rotation timestamps present
Summary: 7 passed, 0 failed, 0 unknown
```

The diagnostic currently checks:

* chain position and last confirmed block
* best fork position
* observed confirmation depth
* connected peers
* RPC responsiveness
* EventGraph parent-reference closure
* consecutive hourly EventGraph rotation periods

The EventGraph checks use the existing DarkIRC `eventgraph.get_info` RPC. `darkfi-inspect` does not modify DarkFi itself; it consumes the information already exposed by the running service.

## EventGraph diagnostics

The EventGraph snapshot can contain hundreds of events across multiple rotating DAGs.

A useful diagnostic property is parent closure: every non-null parent reference in the returned snapshot should resolve to another event in that snapshot.

For a healthy live snapshot:

```text
eventgraph_parents: OK [CONFIRMED] 346 events, 324 non-null parent references, all parents resolved
```

The tool intentionally does not treat event count or layer density as a health invariant. EventGraph state changes naturally as DAGs rotate and old history is pruned.

The rotation check looks for consecutive hourly genesis timestamps in the current rolling window:

```text
eventgraph_rotation: OK [CONFIRMED] 24 consecutive hourly rotation timestamps present
```

This is intended to catch a missing rotation period without requiring the developer to manually inspect the raw EventGraph response.

## JSON output

All diagnostic results can also be emitted as structured JSON:

```text
$ ./target/debug/darkfi-inspect --json diagnose

{
  "checks": [
    {
      "confidence": "Confirmed",
      "message": "last confirmed block: 44559 (6a040708a5a6e30d24170613e928241e98262b9fd6ad8c9ab889a367f4f6a09c)",
      "name": "chain",
      "state": "Pass"
    },
    {
      "confidence": "Confirmed",
      "message": "best fork next height: 44565",
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
      "message": "3 peer(s) connected",
      "name": "peers",
      "state": "Pass"
    },
    {
      "confidence": "Confirmed",
      "message": "RPC responsive (3 ms)",
      "name": "rpc",
      "state": "Pass"
    },
    {
      "confidence": "Confirmed",
      "message": "346 events, 324 non-null parent references, all parents resolved",
      "name": "eventgraph_parents",
      "state": "Pass"
    },
    {
      "confidence": "Confirmed",
      "message": "24 consecutive hourly rotation timestamps present",
      "name": "eventgraph_rotation",
      "state": "Pass"
    }
  ],
  "summary": {
    "failed": 0,
    "passed": 7,
    "unknown": 0
  }
}
```

The JSON form is intended for scripts, monitoring, CI, and future tooling built on top of `darkfi-inspect`.

## Current direction

The project is moving beyond simple RPC observation toward developer diagnostics.

The guiding idea is not to replace the explorer or reproduce logs. The goal is to correlate information from different DarkFi subsystems and answer a more useful question:

> Does what I'm seeing make sense, and where should I look next?

Checks are added when they can be grounded in data returned by DarkFi and verified against real node behavior. The tool should prefer concrete evidence and useful diagnostics over speculative warnings.
