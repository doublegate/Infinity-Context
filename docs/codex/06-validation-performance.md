# Validation, test strength, and measured performance

Audited revision: `a996d59f1eb468e7c386747e15f4c7e58d08410e`. This report distinguishes execution evidence, static conclusions and remaining experiments. Commands and exit codes are retained in `evidence/validation-gates.json`, with one log per gate. Host-only reruns have separate logs. No production code was modified.

## Method and environment

Intel Core i9-10850K, 20 logical CPUs, Linux x86-64, rustc 1.100.0 / LLVM 23.1.5. Complete version output is in `evidence/validation-environment.json`. The inherited Cargo configuration uses 16 build jobs, `clang`, mold, split debuginfo and `target-cpu=native`. Baseline therefore must not be represented as a portable-default build. The separate benchmark runner explicitly overrides RUSTFLAGS for `x86-64` and `native`, using different target directories.

All builds, sanitizer artifacts, mutation copies, synthetic state and coverage output are under `/tmp/Infinity-Context/validation/`. Reusable runners and compact logs are under this report's evidence directory. Compilation is offline and uses existing dependencies/tools; no provider calls are made. Concurrent fuzz compilation and mutation work contend for the host. Timings are informational on a shared active workstation, not controlled-hardware percentile guarantees.

## Gate interpretation

Format and strict Clippy pass. Warning-free workspace rustdoc passes. Initial sandboxed workspace tests fail six daemon/proxy CLI tests because socket bind returns EPERM. A separate host-permission rerun completes successfully. This is an execution-environment limitation, not six confirmed production bugs. Coverage has the same initial socket restriction and is independently rerun with host socket permission. Consult the final measured-status appendix and exact logs for final exits; a partial coverage run is not a coverage percentage.

`doctor` against an uninitialized state does not verify a populated store or real agent hook registration. The workspace tests cover populated store integrity paths, and the security workstream separately probes persistence. Passing doctor must not be presented as complete integration success.

## ICX-CODEX-072 — 10k scan timing is extrapolated, not measured

- Category: measurement limitation. Severity: Low. Confidence: high. Revision: audited SHA.
- Location: `crates/icx-cli/src/main.rs:1412`, `docs/performance.md` Hamming-scan table.
- Source proof: the benchmark allocates exactly 1,000 candidates, times one scan, then assigns `per_10k_us = hdc_us * 10.0`. The CLI prints that derived number as microseconds per 10k nodes; the performance table repeats a 10k figure under measured results.
- Prerequisite/impact: readers use the 10k value for sizing; cache behavior, bandwidth and scheduling at 10k are assumed linear without actual measurement. No memory-safety or remote security impact is alleged.
- Counterevidence: the loop has linear algorithmic work, so multiplication is a reasonable first estimate. It is not evidence of a measured time or a tail percentile. The docs correctly identify the whole benchmark as synthetic/in-process.
- Recommendation: label it explicitly as an extrapolation, or allocate and measure 10k candidates separately; report fixture dimensions, ten-run dispersion and CPU flags.
- Acceptance: an automated output check distinguishes `measured n=1000` from `estimated n=10000`, or evidence contains an actual 10k scan series. No API/wire change.
- Prior ID: ICX-AUD-028 is substantially fixed for broad benchmark honesty, with this remaining measurement caveat.

## ICX-CODEX-073 — Mutation samples expose unpinned behavior

- Category: test assurance gap. Severity: Medium. Confidence: high for observed surviving mutants; not every survivor is a production defect. Revision: audited SHA.
- Locations: `crates/icx-core/src/dag.rs:260`; HDC index `lib.rs:356,534`; `surprise.rs:216`; codec `redact.rs:376,460`; see exact logs and generated inventories.
- Evidence: cargo-mutants inventories all generated mutations per crate and executes the round-robin `1/100` shard independently for each of all eight crates. The initial five completed crates contain surviving replacements of `find_turn_any_role` with a fabricated constant, HDC multiplication with division, surprise arithmetic, redaction value-span/assignment comparisons and log skeleton behavior. Exact mutation strings and outcomes are recorded; no production file is patched in place.
- Impact: line coverage and aggregate integration success cannot establish that package tests detect these semantic changes. Redaction boundaries are particularly material because secrets can reach storage before recovery is possible.
- Counterevidence: some survivors can be semantically equivalent under intended preconditions or caught by downstream workspace tests; default cargo-mutants runs package tests, not an independently guaranteed whole-workspace mutation oracle. A timeout is not automatically a caught mutant, and a build failure is unviable. The report never imputes the sample's score to the full inventory or claims every survivor exploitable.
- Recommendation: triage each survivor with explicit boundary inputs and independent expected output; retain minimal redactor vectors and cross-crate DAG lookup tests, then run a complete workspace-aware mutation campaign in a dedicated runner. Add bounded incremental mutation checks for critical changed code.
- Acceptance: each retained survivor is either killed by a meaningful regression oracle, demonstrated equivalent under documented invariants, or recorded as an accepted gap; full-campaign caught/missed/unviable/timeout counts are separately published. No production compatibility change is required to strengthen tests.
- Prior ID ICX-AUD-039 remains partially fixed. Property tests and four fuzz targets now exist; this independently confirms remaining oracle gaps without relying on the old mutation score.

## Fuzzing and mutation scope

The four shipped fuzz targets are `ahtl_stream`, `hdc_snapshot`, `mcp_jsonrpc`, and `redact`. The runner requests `-max_total_time=900` and 2GiB RSS limit for each target, separate writable corpora and artifact directories, with nightly sanitizer builds. Compilation time is not fuzz time. Final status and fuzzer statistics are in the measured-status appendix and `validation-fuzz.json`. A crash is retained as a separate defect lead, not hidden by a successful other target.

This campaign starts from empty generated corpora; it does not pretend to exhaust the grammar, long/deep packet cases, semantic redaction recall, proxy HTTP/SSE, or real agent transports. Existing harness source is inspected, and uncovered reachable parsers remain explicit follow-up work. Full planned exhaustive mutation execution is not complete: only a deterministic approximately 1% shard per crate is attempted here, plus complete generated inventories. Unexecuted mutations remain exclusions, not passing tests.

## Benchmark methodology and interpretation

`evidence/validation-benchmark.py` builds portable and native variants separately, warms each once, then launches ten independent benchmark processes. Exact stdout and process wall time are retained per run. These component means/single-scan times are not p99 and must not be relabeled as such. The runner does not include build time in process runtime. Shared-host compilation/fuzzing interference limits comparison; broad dispersions invalidate small claimed gains.

The CLI's own fixture uses 1,200 repetitive test lines for the headline 214x skeleton-plus-handle ratio. Its varied-log output explicitly admits it is not a lossless encoding. The prefix metric is byte-weighted synthetic reuse, with a 300-repeat pinned head; it is not measured provider cache utilization. The hook metric calls the in-memory adapter directly, with no process start, SQLite, daemon transport or disk flush. Bus timing uses the in-process backend. These are useful component regressions, with bounded capability claims.

Measured optimization opportunities should be drawn from portable/native differences, recall scaling, and daemon/store evidence. No production optimization is implemented in this audit. Before adopting an optimization, rerun on a quiet host, preserve exact flags, verify correctness with the same corpus, and report before/after dispersion and memory alongside latency.

## Prior findings

| Prior ID | Reassessment | Evidence and limits |
| --- | --- | --- |
| ICX-AUD-001 | Partially verified remediation; broader latency claims require scale tests | Source persists HDC snapshots and ships daemon. This component benchmark does not prove 10k real-binary hook p99. |
| ICX-AUD-002 | Confirmed fixed for labeling; fixture-dependent effectiveness remains unverified | CLI/docs now call the number byte proxy; provider response-token measurements not performed. |
| ICX-AUD-027 | Confirmed fixed for varied CDC fixture | Current CAS fixture uses deterministic varied lines and reports chunk count. |
| ICX-AUD-028 | Substantially fixed, remaining extrapolation caveat | Native flags recorded, portable/native repeats measured here; ICX-CODEX-072. |
| ICX-AUD-039 | Partially fixed | Property tests/fuzz exist; new mutation survivors remain, ICX-CODEX-073. |
| ICX-AUD-040 | Pins and deny policy fixed; no complete CI success predicate in release | Supply-chain report ICX-CODEX-070. |
| ICX-AUD-049 | Separate mathematical/retrieval report owns reassessment | Component target query is planted and cannot alone demonstrate general recall. |
| ICX-AUD-050 | Source remediation visible; measured implementation parity is separate | No assertion that one microbenchmark proves saturation behavior. |
| ICX-AUD-055 | Partially verified | Two-handle same-process test is distinct from four OS processes/agents; no four-agent timing established here. |

## Reviewed-file disposition

All eight manifests, root Cargo configuration/lock/toolchain, four fuzz target sources, CLI benchmark/doctor code and integration test entry points, test inventories, property/recall and bus regression suites, CI validation commands, `docs/performance.md`, `docs/build-and-tooling.md`, `docs/testing-strategy.md`, `docs/claude/07-performance-and-benchmarks.md`, `08-test-quality.md`, `10-ci-supply-chain.md`, and `12-v1.1.0-remediation-status.md` were inspected at the depths stated above. Execution logs distinguish direct tests from source review. Full source review of other crates belongs to the codec/state workstreams; executing them is not a substitute for semantic review.


## Measured status (completed baseline)

| Gate | Outcome |
| --- | --- |
| Format / strict Clippy / warning-free rustdoc | PASS |
| Workspace tests | Sandbox6 CLI socket failures; host rerun PASS |
| 95% line coverage | Host gate PASS; LCOV18,925/19,890=95.1483% |
| Synthetic benchmark baseline | FAIL Hamming scan291.50us vs150us local gate under concurrent work |
| Doctor | PASS; no populated-store or actual agent registration guarantee |
| cargo-deny | Initial invalid option, then read-only DB lock failure; corrected isolated DB PASS all4 stages; DB fetch succeeds |
| cargo-audit | PASS locally cached1533-advisory DB; read-only index lock warning |
| cargo-about / cargo-machete | PASS |
| REUSE3.3 | Initial uv tool-directory write blocked; cached read-only executable PASS608/608 |
| Gitleaks | Installed8.24.3 reports7 historical synthetic-fixture hits; exact checksum-verified CI8.30.1 PASS506 commits25.42MB no leaks |
| actionlint / Antigravity selftest / CLA selftest | PASS |

Advisory database revision after isolated refresh: `f291af9b1cc0d21dfcea1709d6b037963cef262b`, committed2026-11-20T13:47:55+01:00. This is dependency advisories, not a certification of native C components.

The pinned scanner result supersedes any implication that the old scanner's7 fixture detections indicate live secrets. Only rule/file/line/commit metadata is retained for those old-version hits; no secret values are published.

Ten-run shared-host benchmark summary (warm-up excluded): portable0/10 complete SLO passes, native1/10. Hamming scan medians/min/max: portable460.67/315.17/681.38us; native656.535/126.67/4685.31us. Native appearing slower during intermittent multi-millisecond stalls demonstrates uncontrolled interference; these figures do **not** establish that portable compilation is faster. No optimization benefit is accepted from this series. Repeat on an idle dedicated host before drawing quantitative comparisons. All raw outputs are retained.

## Final fuzz and mutation outcomes

Each fuzz target received a 900-second execution budget after compilation. Three completed
901 seconds of actual fuzzing; their final LeakSanitizer shutdown failed because the execution
environment forbids its ptrace operation. All three outer commands exited 1. These are **not
clean sanitizer passes**, and the empty shutdown artifacts are not demonstrated parser bugs.
Single-input replay of each empty artifact exits 0 with leak detection disabled only for that
child; host ptrace policy was unchanged. See [replays](evidence/state-debug-sanitizer-replay.json)
and [exact outer commands/exits](evidence/validation-fuzz.json).

| Target | Actual completed executions | Final disposition |
| --- | --- | --- |
| `ahtl_stream` | 3,656,818 in 901 seconds | No in-run assertion found; sanitizer shutdown restricted |
| `hdc_snapshot` | 4,687,530 in 901 seconds | No in-run assertion found; sanitizer shutdown restricted |
| `mcp_jsonrpc` | 2,212,528 in 901 seconds | No in-run assertion found; sanitizer shutdown restricted |
| `redact` | Stopped before budget on assertion | Fuzzer exit 77, outer command exit 1; minimized and independently reproduced as006 |

The redactor's initial outer wall time of799.48 seconds includes compilation, so it is not
799.48 seconds of fuzzing. Extending the suspicious case consisted of minimization, independent
production-library replay and valid-YAML counterexamples. Its property failure is distinguished
from production panic and from the separate credential leak001.

After initial round-robin1/100 samples, expanded round-robin1/20 samples selected257 mutations
across all eight crates. All257 have per-mutant outcomes. They are overlapping samples, so the
initial and expanded campaigns must not be added as unique coverage. The complete generated
mutation space remains unexecuted. Corrected expanded results are:

| Crate | Selected | Caught | Missed | Unviable | Timeout |
| --- | --- | --- | --- | --- | --- |
| core | 28 | 19 | 6 | 3 | 0 |
| codec | 59 | 41 | 17 | 0 | 1 |
| store | 9 | 8 | 1 | 0 | 0 |
| bus | 15 | 9 | 5 | 1 | 0 |
| hdc | 46 | 28 | 17 | 1 | 0 |
| adapters | 46 | 24 | 13 | 8 | 1 |
| proxy | 27 | 12 | 9 | 2 | 4 |
| cli | 27 | 14 | 11 | 0 | 2 |

Several workers exhausted temporary storage while copying large ignored source artifacts;
some campaign summaries consequently have null end times and failed wrapper exits despite
complete per-mutant records. Four outcomes were contaminated by ENOSPC and were retried in a
small isolated Cargo/crates snapshot: core boolean replacement caught, bus shift replacement
missed, both store replacements caught. The table substitutes only those four exact outcomes;
neighboring regex matches in the retry are not added. Infrastructure failure is not compiler
unviability. See [final monitor](evidence/state-debug-monitor.json),
[retry results](evidence/state-debug-retry-summary.json) and expanded per-crate logs.
Timeouts are not counted as caught; missed mutations still require semantic triage before
becoming separate production findings. Audit-owned scratch was cleaned without removing
production or pre-existing source artifacts.

Final REUSE3.3 recheck with audit deliverables present passes678/678 scanner-reported files,
exit0; see [closeout log](evidence/closeout-reuse.log). This checks metadata consistency,
not the qualified redistribution-rights questions in report07.
