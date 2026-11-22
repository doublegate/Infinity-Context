# v1.2.0 implementation and qualification

Branch: `codex-audit`. Manifest version: 1.2.0. **Released 2026-11-21 (pull request #22).** Audit evidence against a996d59 remains unchanged in `docs/codex/`.
The [implementation tracker](../../../to-dos/codex-audit-remediation.md) and
[project status](../../../PROJECT-STATUS.md) distinguish completed work from open gates.

## Implemented behavior

- YAML scalar coverage, stable redaction and hostile Profile A framing regressions.
- Exact lexical postings augment approximate retrieval; snapshots 04 persist postings and read 03.
- Finite/range/cache consistency validation, checked bus sequences and overflow-safe chunk sizes.
- Automatic bounded icx-managed prompt/window and structured recall frames; unsent bus stays queued.
- Daemon saves before success; SQLite WAL FULL. Failed saves return errors.
- Strict MCP envelope/argument/canonical-tool contracts. Codex capture does not promise replacement.
- Exact-SHA CI/REUSE release gates, independent measured 1k/10k scans, JSON build/fixture provenance.
- Current compatibility decisions in [ADR 0006](../../adr/0006-v1-2-0-context-and-compatibility.md).

## Evidence and pending qualification

The 27 frozen corpus rows produce 100% recall@1/@5 for their synthetic unique identifiers.
The actual >10k turn ingestion/budget test passes; old content still dereferences exactly.
Neither is a live model-reasoning or whole-provider-request guarantee. Account external user
input/tool schemas and the model's completion reserve when configuring `icx context-budget`.

Three initial parser fuzz campaigns completed 900+ seconds with normal sanitizer shutdown.
After the retained counterexamples were fixed, final redactor qualification passed 859,078
executions/901 seconds and seeded 03/04 snapshot qualification passed 2,231,269/901 seconds.
Logs retain failures instead of relabeling them. Initial mutation samples covered all
eight crates; full generated mutation space is not exhausted. Post-snapshot targeted proofs
remain separate from sample counts. Codex's portable/native strict benchmark series ran beside
unrelated compilation and failed. The pre-release review diagnosed the one reproducible failure
(the 10k exhaustive scan is memory-bandwidth bound and its ceiling was an unmeasured
extrapolation), added an exact parallel scan and a measured ceiling, and the final quiet-host
native series passed all gates in 10 of 10 runs; see [performance](../../performance.md).

The [qualification ledger](evidence/qualification-summary.json) recorded 24 PASS, 2 FAIL and 4 WARN
checks at the end of Codex's work (the 2 FAILs were the strict performance series). The
pre-release review resolved the performance failure as above and, from v1.2.0 on, finding 071 by owner decision
(publications replaced by a [source manifest](../../../ref-docs/unicode-braille-patterns/SOURCES.md); the v1.1.0 tag, its source archives and git history still contain them, and rewriting published history is a separate owner decision);
finding 074 stays open by owner decision pending counsel. The release is qualified by the
exact-commit CI run of the pull request. The review's own fixes are listed under
"Release-review fixes" in the [CHANGELOG](../../../CHANGELOG.md).
Codex's last full workspace run passed 224 tests with 0 ignored, including long-horizon
qualification, at 19,571/20,559 = 95.1943% line coverage. Strict Clippy passes after test-formatting
correction; the earlier failure remains recorded. Dependency/advisory, license, documentation,
secret-history and workflow-policy checks pass. The final nonignored working-tree source scan
passes with narrowly scoped exceptions for two reviewed public source hashes and one exact
synthetic audit fixture. Six positive/negative controls verify that other credentials remain
detectable; the gate also runs in CI. Initial scan failures and
[individual adjudication](evidence/secrets-working-tree-adjudication.json) remain visible.
A full-directory scan additionally flags ignored generated documentation/fuzzer inputs;
its [separate adjudication](evidence/secrets-ignored-generated-adjudication.json) explains scope.
Initial local package checks exposed incorrect musl linkage and host GNU symbols newer than
the advertised minimum. Corrected builds of all four variants now pass static/symbol checks,
checksums, deterministic archives and Ubuntu 22.04/glibc 2.35 smoke tests. The initial negatives
remain recorded. See [local package qualification](evidence/package-qualification.md);
this working-tree result does not certify a committed/tagged release.
The [governance review packet](governance-review.md) records owner/qualified-review decisions
that tests cannot resolve. No commit, merge, tag, publication, active agent configuration or
user-memory change is part of this local implementation.

## Upgrade and downgrade

Back up a workspace before upgrading. 03 derived snapshots are read and rebuilt from preserved
CAS events, then saved as 04. Older executables cannot read 04: restore the pre-upgrade backup
for downgrade, rather than reinterpreting or deleting data. Unversioned Profile A fc remains
Q15 scale 32,767. Existing stored secrets are not silently scrubbed or user credentials rotated.

`icx context-budget --dir PATH` reports the persisted policy and estimator. For example:

```sh
icx context-budget --dir PATH --context-tokens 32768 --completion-reserve 4096 --external-reserve 4096
```

The one-UTF8-byte-per-token allowance is conservative estimation, identified as
`utf8-bytes-upper-bound-v1`; it is not a downloaded model tokenizer or provider measurement.
Changing the model/window policy can change prompt prefixes and invalidate cached history.
