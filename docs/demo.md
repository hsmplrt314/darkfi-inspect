# darkfi-inspect demo

`darkfi-inspect` is a developer tool for observing, inspecting, and eventually diagnosing DarkFi nodes and network behavior.

## Inspect a healthy block

```text
$ ./target/debug/darkfi-inspect inspect block 43931

Block 43931
Hash:     5b84b47e5acce4ed752cf8756091a981a58fdd0f773329d3bcc633499b1a2a8a
Previous: 7650893c640ea7f2dad7b557768703088d7d6dd5c2ebde415736b9914822a8a7
Txs:      1

chain_linkage: OK [CONFIRMED] previous hash matches block 43930
timestamp_sanity: OK [MEDIUM] 94s since block 43930 — within plausible range (target: 120s)
```

The block links correctly to its predecessor and its timestamp gap is within the current heuristic range.

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

## JSON output

The same inspection can be requested as structured JSON:

```text
$ ./target/debug/darkfi-inspect --json inspect block 43931
{
  "height": 43931,
  "hash": "5b84b47e5acce4ed752cf8756091a981a58fdd0f773329d3bcc633499b1a2a8a",
  "previous": "7650893c640ea7f2dad7b557768703088d7d6dd5c2ebde415736b9914822a8a7",
  "txs": 1,
  "checks": [
    {
      "name": "chain_linkage",
      "state": "Pass",
      "confidence": "Confirmed",
      "message": "previous hash matches block 43930"
    },
    {
      "name": "timestamp_sanity",
      "state": "Pass",
      "confidence": "Medium",
      "message": "94s since block 43930 — within plausible range (target: 120s)"
    }
  ],
  "summary": {
    "passed": 2,
    "failed": 0,
    "unknown": 0
  }
}
```
This is intended to make the output useful for scripts, monitoring, CI, and future tooling built on top of `darkfi-inspect`.

## Current direction

The project is intentionally still early.

The current implementation covers basic node observation and block inspection. The next checks and commands should be driven by actual DarkFi developer needs rather than by trying to build a huge list of diagnostics for its own sake.
