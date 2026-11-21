# v1.2.0 implementation and qualification

Branch: `codex-audit`. Manifest version:1.2.0. **Development candidate; not published or yet
release-qualified.** Audit evidence against a996d59 remains unchanged in `docs/codex/`.
The [implementation tracker](../../../to-dos/v1.2.0-remediation.md) and
[project status](../../../PROJECT-STATUS.md) distinguish completed work from open gates.

## Implemented behavior

- YAML scalar coverage, stable redaction and hostile Profile A framing regressions.
- Exact lexical postings augment approximate retrieval; snapshots04 persist postings and read03.
- Finite/range/cache consistency validation, checked bus sequences and overflow-safe chunk sizes.
- Automatic bounded icx-managed prompt/window and structured recall frames; unsent bus stays queued.
- Daemon saves before success; SQLite WAL FULL. Failed saves return errors.
- Strict MCP envelope/argument/canonical-tool contracts. Codex capture does not promise replacement.
- Exact-SHA CI/REUSE release gates, independent measured1k/10k scans, JSON build/fixture provenance.
- Current compatibility decisions in [ADR0006](../../adr/0006-v1-2-0-context-and-compatibility.md).

## Evidence and pending qualification

The27frozen corpus rows produce100% recall@1/@5 for their synthetic unique identifiers.
The actual >10kturn ingestion/budget test passes; old content still dereferences exactly.
Neither is a live model-reasoning or whole-provider-request guarantee. Account external user
input/tool schemas and the model's completion reserve when configuring `icx context-budget`.

Three initial parser fuzz campaigns completed900+seconds with normal sanitizer shutdown.
After the retained counterexamples were fixed, final redactor qualification passed859,078
executions/901seconds and seeded03/04 snapshot qualification passed2,231,269/901seconds.
Logs retain failures instead of relabeling them. Initial mutation samples covered all
eight crates; full generated mutation space is not exhausted. Post-snapshot targeted proofs
remain separate from sample counts. Original and optimized portable/native strict benchmark
series fail unchanged ceilings. Host compiler contention was observed; further diagnosis and
performance qualification remain open.

The [qualification ledger](evidence/qualification-summary.json) records24PASS,2FAIL and4WARN
checks. **Release readiness remains false.** The failures are strict performance qualification;
open warnings cover governance and the exact-commit publication process.
The latest full workspace run passes224tests with0ignored, including long-horizon qualification;
line coverage is19,571/20,559=95.1943%. Strict Clippy passes after test-formatting
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
checksums, deterministic archives and Ubuntu22.04/glibc2.35 smoke tests. The initial negatives
remain recorded. See [local package qualification](evidence/package-qualification.md);
this working-tree result does not certify a committed/tagged release.
The [governance review packet](governance-review.md) records owner/qualified-review decisions
that tests cannot resolve. No commit, merge, tag, publication, active agent configuration or
user-memory change is part of this local implementation.

## Upgrade and downgrade

Back up a workspace before upgrading.03derived snapshots are read and rebuilt from preserved
CAS events, then saved as04. Older executables cannot read04: restore the pre-upgrade backup
for downgrade, rather than reinterpreting or deleting data. Unversioned Profile A fc remains
Q15scale32,767. Existing stored secrets are not silently scrubbed or user credentials rotated.

`icx context-budget --dir PATH` reports the persisted policy and estimator. For example:

```sh
icx context-budget --dir PATH --context-tokens 32768 --completion-reserve 4096 --external-reserve 4096
```

The one-UTF8-byte-per-token allowance is conservative estimation, identified as
`utf8-bytes-upper-bound-v1`; it is not a downloaded model tokenizer or provider measurement.
Changing the model/window policy can change prompt prefixes and invalidate cached history.
