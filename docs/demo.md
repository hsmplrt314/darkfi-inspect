# darkfi-inspect demo

`darkfi-inspect` is a developer tool for observing and inspecting DarkFi nodes and network behavior.

The examples below are based on behavior tested against a live DarkFi node.

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

The block inspection verifies that the returned block matches the requested height, links to its predecessor, and has a timestamp gap within the current heuristic range.

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

The important distinction is that the tool reports the observation and its confidence instead of pretending that every anomaly is proven to be a consensus failure.

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
calls_proofs: OK [CONFIRMED] call count matches proof-group count (1)
calls_signatures: OK [CONFIRMED] call count matches signature-group count (1)
```

The transaction inspector reports the transaction hash, call structure, proof/signature group counts, and structural consistency checks.

It does not attempt to decode arbitrary contract-specific calldata or claim that ZK proofs or signatures are cryptographically valid without the additional verification context required by DarkFi.

## JSON output

The same transaction can be requested as structured JSON:

```text
$ ./target/debug/darkfi-inspect --json inspect tx 8daf70f08516698100048c2280a7259f9b45a7c9c61938db0ceca3649d3668e9

{
  "hash": "8daf70f08516698100048c2280a7259f9b45a7c9c61938db0ceca3649d3668e9",
  "calls": 1,
  "proofs": 1,
  "signatures": 1,
  "call_details": [
    {
      "index": 0,
      "contract_id": "BZHKGQ26bzmBithTQYTJtjo2QdCqpkR9tjSBopT4yf4o",
      "function_code": 2,
      "data_length": 534,
      "parent": null,
      "children": []
    }
  ],
  "summary": {
    "passed": 2,
    "failed": 0,
    "unknown": 0
  },
  "checks": [
    {
      "name": "calls_proofs",
      "state": "Pass",
      "confidence": "Confirmed",
      "message": "call count matches proof-group count (1)"
    },
    {
      "name": "calls_signatures",
      "state": "Pass",
      "confidence": "Confirmed",
      "message": "call count matches signature-group count (1)"
    }
  ]
}
```

The JSON form is intended to make the output useful for scripts, monitoring, CI, and future tooling built on top of `darkfi-inspect`.

## Current direction

The current implementation covers basic node observation plus block and transaction inspection.

The inspection layer is intentionally conservative: checks are added when they can be grounded in data returned by DarkFi and verified against real node behavior, rather than presenting speculative diagnostics as facts.
