# Phase 10 — Prevent and Prove

**Release:** v1.8.0

## Goal

Prevent bulky output before it reaches the agent (pre-execution command rewriting, pre-tool
redirects for oversized reads, log and test-output templating), account for every saving in one
ledger, and prove the savings with a paired, noise-aware benchmark that peers can compare against.
Alongside, pay down the code-health debt found in the codebase-gap pass and close the remaining
security residuals from the threat model.

Sources are the second external research pass (2026-11-22) and the codebase-gap pass
(2026-11-22).

## Exit Criterion

- [ ] The PreToolUse command rewriter and the pre-tool redirect are shipped fail-open, with golden
  tests per filter, and their savings are recorded in the ledger.
- [ ] `icx gain` and `icx discover` report measured savings and missed savings from session logs.
- [ ] `icx bench --harness terminal-bench` has a recorded baseline variance from at least three
  repeated paired runs, and the release gate blocks only on a statistically significant pass-rate
  drop; results and manifests are published in `docs/numbers.md`.
- [ ] The module splits, adapter de-duplication, typed errors, Cargo features, fuzz targets with a
  nightly job, and property tests are merged with behaviour unchanged.
- [ ] Threat-model residuals #3, #6, #7, #8 and #9 are closed or reduced, and `deny.toml` has an
  explicit `[bans]` policy.

## Sprints

- [Sprint 1 — Pre-execution command rewriter](sprint-1-pre-execution-command-rewriter.md)
- [Sprint 2 — Pre-tool redirect](sprint-2-pre-tool-redirect.md)
- [Sprint 3 — Log and test-output templating](sprint-3-log-and-test-output-templating.md)
- [Sprint 4 — Savings ledger and analytics](sprint-4-savings-ledger-and-analytics.md)
- [Sprint 5 — Paired benchmark harness](sprint-5-paired-benchmark-harness.md)
- [Sprint 6 — Code health](sprint-6-code-health.md)
- [Sprint 7 — Security residuals](sprint-7-security-residuals.md)

## Dependencies on Earlier Phases

- Phase 6 Sprint 2 (Codex output shrinking): Sprint 1 extends it to the cases where post-hoc
  replacement is impossible.
- Phase 6 Sprints 3 to 5: per-agent PreToolUse support and contract fixtures for the rewriter and
  redirect.
- Phase 7 Sprint 4 (structural redactor): the `redact.rs` split in Sprint 6 is coordinated with it.
- Phase 8 Sprint 3 (`icx stats`, traces, generated numbers table): the savings ledger shares its
  storage, and the benchmark publishes through the generated table.
- Phase 9 Sprint 1 (storage): the ledger uses the incremental save path.

## Standing Sources to Watch

- rtk (command-rewriting filter set and failure modes) and context-mode (pre-tool redirect
  pattern) repositories and release notes.
- Terminal-Bench 2.0 harness repository, its repeat-run guidance and leaderboard fields.
- Claude Code, Codex CLI, Antigravity CLI, Gemini CLI, Cursor, OpenCode, Hermes and Muse
  changelogs, especially PreToolUse input-rewrite support.
- MCP specification changelog.
- `logdrain` and Drain3 releases; RustSec advisories; `cargo-fuzz`, `proptest`, `rustls` and
  `keyring` crate releases (Context7).

## Reference Docs

- [docs/agent-adapters-and-interception.md](../../docs/agent-adapters-and-interception.md)
- [docs/alien-holographic-codec.md](../../docs/alien-holographic-codec.md)
- [docs/security-and-threat-model.md](../../docs/security-and-threat-model.md)
- [docs/testing-strategy.md](../../docs/testing-strategy.md)
- [docs/numbers.md](../../docs/numbers.md)
