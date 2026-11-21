# State audit reproduction procedure

Run from the audited checkout. The Rust harness imports the actual daemon module and calls actual public workspace, MCP, chunker and bus APIs. Its dispatcher is a synthetic ingest callback, not a real harness invocation. All state is synthetic and written under `/tmp/Infinity-Context/state-reproduction-<pid>/`.

```bash
mkdir -p /tmp/Infinity-Context/state-probe/src
cp docs/codex/evidence/state-probe.Cargo.toml /tmp/Infinity-Context/state-probe/Cargo.toml
cp docs/codex/evidence/state-probe.rs /tmp/Infinity-Context/state-probe/src/main.rs
CARGO_TARGET_DIR=/tmp/Infinity-Context/state-probe/target cargo build --offline \
  --manifest-path /tmp/Infinity-Context/state-probe/Cargo.toml
/tmp/Infinity-Context/state-probe/target/debug/icx-state-audit-probe
```

If the checkout is elsewhere, replace the absolute checkout prefixes in the copied manifest and the harness's `#[path]` attribute before compiling. This changes scratch copies only. The harness is not a workspace member and does not alter Cargo.lock. A fresh temporary lockfile resolves the declared dependency constraints from locally cached crates; pinning it to the audited lockfile would improve strict reproduction across dependency caches.

The sandbox denied binding the private Unix socket (`Operation not permitted`). The same built probe was then run with reviewed escalation, confined to these scratch workspaces. No production daemon was killed. `Child::kill()` targets only the process this probe spawned.

| Execution | Exit | Evidence |
| --- | ---: | --- |
| Initial sandbox build/run | 101 | Failed only when binding the private Unix socket; earlier public-API probes had completed. Superseded by the successful host execution, not counted as a source test failure. |
| Final offline debug build | 0 | `state-probe-build.log`; unused `try_hook` warning arises because only the server half of the imported module is needed |
| Final escalated probe run | 0 | `state-probe-results.jsonl` and `state-probe-runtime.log` |

Expected observations:

- Chunker panic is caught; the default panic hook prints its overflow location to stderr, but the harness continues and exits zero.
- Four byte-exact secret-free fixtures pass.
- Five malformed MCP calls yield successes (contract defect), with exact request/response pairs retained.
- An intentionally manipulated coherent atomic slot has `seq=257` with sampled head 1 and reported loss 0. This is a public-layout corruption model, not a natural-race timing measurement.
- Truncating the scratch fabric reinitializes its header.
- Separate-process bus sends and observes 100 events; latency includes a 50 µs poll sleep and system scheduling.
- Killing the child after `ok:true` and before write-behind loses the scratch artifact on restart.
- A separate one-request daemon receives a synthetic failure injection: after ingest, its database file is renamed to a preserved scratch copy and its path temporarily replaced by a directory. Final flush fails; the response is still successful and `serve` exits successfully. The harness removes only its empty blocking directory, restores the preserved database and confirms the acknowledged artifact is absent. This verifies failure reporting, not a disk-full reproduction or all five retry transitions.

Unexecuted scenarios: five consecutive disk-full failures, competing real writers after failed flush, a slow/trickling daemon client, actual four-agent propagation, TLS cancellation, randomized process death at every slot/store instruction, and power failure. Their source-backed implications must remain distinguished from the executed probes.

## Curl cancellation countertest

```bash
python3 docs/codex/evidence/state-curl-cancel.py
```

Run with local-listener permission if the sandbox restricts loopback binding. This starts only a synthetic local HTTP SSE server and actual `curl`, closes its stdout after the first data event, waits at most three seconds, and cleans up both fixture processes. The reviewed execution exited zero; `state-curl-cancel.json` records curl status 23 and exit within 16.093 ms. It disproves the source-only claim that a continuously streaming provider keeps curl alive after stdout closes. It is not a full HTTPS proxy integration test. Lead 034 was removed from confirmed findings.
