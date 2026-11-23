# Project status

## Executive summary

v1.4.0 is the current release (2026-11-23), closing Phase 6 (#40 to #44): Gemini CLI, Cursor and
OpenCode adapters, the Hermes context engine and shell-output plugin, `icx install`, a read-only
`icx` MCP tool (ADR-0009), live contract fixtures for Claude Code, Codex, agy and Muse, and a
generated capability matrix (`docs/capability-matrix.md`). Live runs for Hermes, Gemini CLI and
Cursor tool calls are deferred to v1.5.0 (#89); known issue #90 (redactor over-redaction).

v1.3.2 (2026-11-22) fixed #33, #34, #36 and #37 (stubs not found in git
worktrees and after a session changes directory, MCP results stored as lines, CLI and redactor
follow-ups, docs-site hardening) and field problems found in four days of agent sessions. v1.3.1
fixed #28, #29 and #30 on top of v1.3.0, which added the Meta Muse Code adapter.

v1.2.2 (2026-11-21, pull requests #23 and #25) followed up v1.2.0
with the open review suggestions, two post-upgrade regressions (command recall, token-count
over-redaction) and a redactor panic found after v1.2.1 was tagged; v1.2.1 was therefore never
published. Notes: [`docs/releases/v1.2.2/`](docs/releases/v1.2.2/README.md).

v1.2.0 was released on 2026-11-21 from pull request #22 (branch `codex-audit`). It
remediates the independent audit of v1.1.0 in `docs/codex/` (Codex implementation, tracked in
[the v1.2.0 plan](to-dos/codex-audit-remediation.md)) and the defects found by the pre-release review
(tickets T-120-026..030 and the parallel scan). It was qualified by the exact-commit CI run
of that pull request and published as the v1.2.0 GitHub release.

## Phase matrix

| Phase | State |
| --- | --- |
| 0 — Plan and acceptance criteria | complete |
| 1 — Redaction and integrity | verified; review closed header, multi-line, flow, plural and digest gaps |
| 2 — Retrieval and context budgets | verified; review added IDF ranking, bounded candidates and step eviction |
| 3 — Durability and integrations | verified; review fixed daemon lock handling and Codex null replies |
| 4 — Documentation and governance | docs reconciled; 071 resolved from v1.2.0 on by owner decision (earlier tags and history unchanged), 074 open pending counsel |
| 5 — Assurance and release qualification | v1.4.0 qualified on #88, #91 and its release PR; v1.3.2 on its release PR; v1.3.1 on PR #31 (exact-commit CI, Codecov, gitleaks, REUSE); releases verified by checksums and build attestations |

## Feature and capability matrix

CAS exact post-redaction recovery remains supported. New ingestion hides the demonstrated YAML
(block, multi-line, quoted, flow) and plural-key credential cases while keeping public digests.
Exact postings with IDF ranking back HDC recall. The icx-managed window is bounded and evicts
in persisted steps behind one stable omission frame; cold history stays dereferenceable. Daemon
success follows a committed save, survives lock contention and (v1.2.2) transient save
failures without acknowledging them. Recall indexes `:`/`/`-joined tokens by component
(`ICXHDC05`). Token counters, code type annotations and code bindings are not redacted. Since
v1.3.1, a value after a sensitive key that holds a quote is hidden from its first quote to the end
of the line (a call head stays visible), and a complete JSON line is redacted structurally (only
sensitive values are replaced). Codex hooks capture but do not replace PostToolUse output. No general model-effectiveness, provider-cache or unrestricted
full-request bound is claimed.

## Metrics snapshot

| Metric | Value | Source / scope |
| --- | --- | --- |
| Current release | v1.4.0 (v1.3.2 at e32eb3e, v1.3.1 at 10e1f63) | git tags |
| Audit findings | 19: 1 High/10 Medium/8 Low | `docs/codex/10-findings-register.md` |
| Workspace tests | 507 pass, 0 fail (3 ignored: long-horizon qualification, two daemon child scenarios) | `cargo test --workspace`, v1.4.0 |
| Plugin tests | 6 OpenCode (Bun), 24 Hermes stand-in | CI check job, v1.4.0 |
| Line coverage | 95.15% at v1.4.0 (empty `HOME`, as in CI); floor 95% | `cargo llvm-cov` |
| Benchmark | 10/10 quiet-host runs pass all gates | `docs/performance.md`, evidence `benchmark-native-final-series.json` |
| Saturated window prefix reuse | 0.94 (was 0.01) | `crates/icx-core/tests/window_eviction.rs` |
| Frozen retrieval matrix | 27 rows, recall@1/@5 = 100% | synthetic identifiers, not general model recall |
| Mutation samples | 135 executed; 80 caught/43 missed/7 unviable/5 timeout | `docs/releases/v1.2.0/mutation-triage.md` |
| Release packages | 4 Linux variants pass local and Ubuntu 22.04/glibc 2.35 checks | `docs/releases/v1.2.0/evidence/package-qualification.md` |

## Timeline

- 2026-11-21: Codex audit and implementation on `codex-audit`.
- 2026-11-21: pre-release review, fixes, owner decisions (071 links, 074 deferred, curated
  evidence, digest rule, measured parallel 10k ceiling), pull request; v1.2.0 released.
- 2026-11-21: v1.2.1 follow-up (#23) tagged; held unpublished after a redactor panic was found.
- 2026-11-21: v1.2.2 (#25) released: panic fix, issue #24, all v1.2.1 changes.
- 2026-11-21: v1.3.0 released: Meta Muse Code adapter (fifth agent).
- 2026-11-22: v1.3.1 released: redaction from the first quote to the end of the line after sensitive keys (#29), CLI flag values (#28), fail-open replies and Muse fixes (#30).
- 2026-11-22: v1.3.2 released: worktree and moved-session stubs resolve (#37, #34), MCP results stored as lines, 2 KiB stub floor, recall floor, `slice`/`grep` bounds, CLI and redactor follow-ups (#33), docs-site hardening (#36).
- 2026-11-23: v1.4.0 released: Phase 6. Gemini CLI, Cursor and OpenCode adapters; Hermes plugins; `icx install`; read-only `icx` tool and `icx_write`; live fixtures; capability matrix.

## Known issues and remaining work

- ICX-CODEX-074: PATENTS/CLA/COMMERCIAL terms unchanged pending counsel (owner decision).
- Mutation space is sampled, not exhausted; survivors are triaged in `mutation-triage.md`.
- The single-threaded 10k scan is memory-bandwidth bound on shared hosts (reported, ungated).
- Older binaries cannot read upgraded workspaces (v1.1.0 cannot read ICXHDC04, v1.2.0 cannot read
  ICXHDC05; v1.2.x cannot read a workspace once a Muse event is stored); all fail closed. Keep a
  pre-upgrade backup for a downgrade.
- Redaction runs once, at storage: content stored by an earlier version is not re-checked by a
  newer redactor (v1.3.1 release notes).

## Readiness checklist

- [x] Audit and review fixes verified with regression tests
- [x] Compatibility and migration tests pass (v1.1.0 workspace upgrade verified on real data)
- [x] Local quality, security and performance gates pass
- [x] Documentation reconciled; governance decisions recorded (074 open by owner decision)
- [x] Four Linux variants pass local package qualification
- [x] Exact-commit hosted CI green on the pull request
- [x] Merge, tag and publication (owner instruction 2026-11-21)
