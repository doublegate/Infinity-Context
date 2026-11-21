# Project status

## Executive summary

v1.2.0 is a release candidate submitted for review as a pull request from `codex-audit`. It
remediates the independent audit of v1.1.0 in `docs/codex/` (Codex implementation, tracked in
[the v1.2.0 plan](to-dos/v1.2.0-remediation.md)) and the defects found by the pre-release review
(tickets T-120-026..030 and the parallel scan). The release is qualified by the exact-commit
CI run of that pull request; merge, tag and publication are separate owner steps.

## Phase matrix

| Phase | State |
| --- | --- |
| 0 — Plan and acceptance criteria | complete |
| 1 — Redaction and integrity | verified; review closed header, multi-line, flow, plural and digest gaps |
| 2 — Retrieval and context budgets | verified; review added IDF ranking, bounded candidates and step eviction |
| 3 — Durability and integrations | verified; review fixed daemon lock handling and Codex null replies |
| 4 — Documentation and governance | docs reconciled; 071 resolved from v1.2.0 on by owner decision (earlier tags and history unchanged), 074 open pending counsel |
| 5 — Assurance and release qualification | local gates pass; exact-commit hosted CI on the pull request |

## Feature and capability matrix

CAS exact post-redaction recovery remains supported. New ingestion hides the demonstrated YAML
(block, multi-line, quoted, flow) and plural-key credential cases while keeping public digests.
Exact postings with IDF ranking back HDC recall. The icx-managed window is bounded and evicts
in persisted steps behind one stable omission frame; cold history stays dereferenceable. Daemon
success follows a committed save and survives lock contention. Codex hooks capture but do not
replace PostToolUse output. No general model-effectiveness, provider-cache or unrestricted
full-request bound is claimed.

## Metrics snapshot

| Metric | Value | Source / scope |
| --- | --- | --- |
| Source base / candidate | `main` 2a5f9f5 / manifests 1.2.0 | git |
| Audit findings | 19: 1 High/10 Medium/8 Low | `docs/codex/10-findings-register.md` |
| Workspace tests | 245+ pass, 0 fail (2 ignored: long-horizon qualification, daemon child scenario) | `cargo test --workspace` |
| Line coverage | 95.25% (21,035 lines) before the final scan change; floor 95% | `cargo llvm-cov` |
| Benchmark | 10/10 quiet-host runs pass all gates | `docs/performance.md`, evidence `benchmark-native-final-series.json` |
| Saturated window prefix reuse | 0.94 (was 0.01) | `crates/icx-core/tests/window_eviction_v120.rs` |
| Frozen retrieval matrix | 27 rows, recall@1/@5 = 100% | synthetic identifiers, not general model recall |
| Mutation samples | 135 executed; 80 caught/43 missed/7 unviable/5 timeout | `docs/releases/v1.2.0/mutation-triage.md` |
| Release packages | 4 Linux variants pass local and Ubuntu 22.04/glibc 2.35 checks | `docs/releases/v1.2.0/evidence/package-qualification.md` |

## Timeline

- 2026-11-21: Codex audit and implementation on `codex-audit`.
- 2026-11-21: pre-release review, fixes, owner decisions (071 links, 074 deferred, curated
  evidence, digest rule, measured parallel 10k ceiling), pull request.

## Known issues and remaining work

- ICX-CODEX-074: PATENTS/CLA/COMMERCIAL terms unchanged pending counsel (owner decision).
- Mutation space is sampled, not exhausted; survivors are triaged in `mutation-triage.md`.
- The single-threaded 10k scan is memory-bandwidth bound on shared hosts (reported, ungated).
- An older v1.1.0 binary cannot read a workspace upgraded to ICXHDC04 (it fails closed).

## Readiness checklist

- [x] Audit and review fixes verified with regression tests
- [x] Compatibility and migration tests pass (v1.1.0 workspace upgrade verified on real data)
- [x] Local quality, security and performance gates pass
- [x] Documentation reconciled; governance decisions recorded (074 open by owner decision)
- [x] Four Linux variants pass local package qualification
- [ ] Exact-commit hosted CI green on the pull request
- [ ] Merge, tag and publication (separate owner instruction)
