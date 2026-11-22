# Phase 10 (v1.8.0) Implementation Plan — Prevent and Prove

## Goal Description

Deliver v1.8.0 in seven sprints. Sprints 1 to 4 add prevention and accounting, Sprint 5 proves the
effect with a noise-aware paired benchmark, Sprint 6 pays down code-health debt in
behaviour-preserving PRs, and Sprint 7 closes security residuals. Sprints 6 and 7 may run in
parallel with Sprints 1 to 5 where they do not touch the same modules.

## Ordered Approach

1. **Sprint 10.1 — Pre-execution command rewriter (T-10-1-001 to T-10-1-004).**
   - Confirm PreToolUse input-rewrite support per installed agent, study rtk's filter set and
     failure modes, then build the fail-open rewriter, `icx bypass`, the per-command filters with
     golden tests, and the Codex path that extends Phase 6.
2. **Sprint 10.2 — Pre-tool redirect (T-10-2-001, T-10-2-002).**
   - Per-agent opt-in blocking of oversized Read and WebFetch calls with a `slice` / `grep`
     suggestion, and a minimum-saving ratio measured on the text, not the JSON envelope.
3. **Sprint 10.3 — Log and test-output templating (T-10-3-001 to T-10-3-004).**
   - Evaluate `logdrain` against an in-house Drain, add template mining with masks to the
     skeletonizer, add test-failure summaries, and gate the default on a measured reduction over
     the field corpus.
4. **Sprint 10.4 — Savings ledger and analytics (T-10-4-001 to T-10-4-003).**
   - One ledger fed by stubs, rewrites and templates, sharing storage with `icx stats`; then
     `icx gain` and `icx discover`.
5. **Sprint 10.5 — Paired benchmark harness (T-10-5-001 to T-10-5-006).**
   - Harness first, then the hold-equal run manifest, then baseline variance from at least three
     repeated paired runs, then the statistical gate, the cost budget and smoke subset, and finally
     publication.
6. **Sprint 10.6 — Code health (T-10-6-001 to T-10-6-011).**
   - One PR per split with tests unchanged; de-duplication, typed errors and features after the
     splits; fuzzing, property tests and coverage last, measured fresh.
7. **Sprint 10.7 — Security residuals (T-10-7-001 to T-10-7-006).**
   - The rustls upstream client first (it removes header temp files), then streaming redaction,
     optional TLS on remote listeners, bus MACs, the optional OS-keyring key, and the `deny.toml`
     bans policy.

## Risks

- **Rewriting changes command semantics.** A filter that hides an error or changes an exit code is
  worse than no filter. Mitigation: fail-open by construction, exit code always preserved, output
  never longer than the original, golden tests, and `icx bypass`.
- **Agent support differs and changes.** Mitigation: per-agent research gate on the installed
  version, per-agent opt-in, and contract tests that detect a silently ignored rewrite.
- **Benchmark noise mistaken for regression or gain.** Mitigation: the Sprint 5 noise gate
  (baseline variance, paired per-task test, hold-equal list); single-run drops within the spread
  are reported and do not block.
- **Benchmark cost.** About 180 paid agent sessions per paired full run. Mitigation: a per-run
  budget recorded in the manifest, a 15 to 20 task smoke subset for PRs, and full runs only at
  release candidates or when stubbing, rewriting or recall change.
- **Refactors that change behaviour.** Mitigation: one split per PR, tests unchanged, coverage
  measured before and after.
- **Security changes that break existing installs.** Mitigation: new controls are optional where
  the plan says optional, with migration notes.

## Verification

- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` and `cargo llvm-cov ... --fail-under-lines 95` on every sprint PR.
- Golden tests per filter and template; property tests that output is never longer than input and
  exit codes are preserved.
- Ledger totals reconciled against raw traces in tests.
- Benchmark results published only with manifests, the variance baseline and the stated test.
- Nightly fuzz job green; new fuzz targets seeded with corpora.
- Threat model updated for each closed residual.
