# Project status

## Executive summary

The v1.2.0 remediation candidate is implemented on `codex-audit`; final qualification is active.
The audit and its evidence are preserved in `docs/codex/`. Implementation is tracked in
[the v1.2.0 plan](to-dos/v1.2.0-remediation.md). This is not a release-ready revision.

## Phase matrix

| Phase | State |
| --- | --- |
| 0 — Plan and acceptance criteria | complete locally |
| 1 — Redaction and integrity | verified regressions and renewed sanitizer campaigns |
| 2 — Retrieval and context budgets | verified synthetic corpora, production10kturns and upgrade |
| 3 — Durability and integrations | regression suites and crash/reopen tests pass |
| 4 — Documentation and governance | current docs/diagrams reconciled; qualified decisions pending |
| 5 — Assurance and release qualification | final gates, measured performance and packages active |

## Feature and capability matrix

CAS exact post-redaction recovery remains supported. New ingestion handles the demonstrated
YAML/adjacent credential cases and UTF8 regression inputs. Exact postings improve the tested
retrieval corpora; automatic bounded icx-managed context retains cold history. Daemon success
follows WAL FULL commit. Codex hooks capture but do not replace PostToolUse output. No general
model-effectiveness, provider-cache or unrestricted full-request bound is claimed.

## Metrics snapshot

| Metric | Source / scope |
| --- | --- |
| Source baseline / candidate | a996d59 / manifests1.2.0, uncommitted working tree |
| Audit findings | 19:1High/10Medium/8Low, `docs/codex/10-findings-register.md` |
| Post-change line coverage | Final run95.1943%:19,571/20,559 lines; all224workspace tests pass including ignored qualification test |
| Frozen retrieval matrix | 27 rows, recall@1/@5=100%; synthetic identifiers, not general model recall |
| Production context budget | >10kturn ingestion test passes; cold artifact exact recovery preserved |
| Mutation samples | 135 executed across8crates;80caught/43missed/7unviable/5timeout; targeted proofs separate |
| Latest release qualification | All four Linux packages pass local/Ubuntu2.35 checks; optimized strict benchmark series still fail |

## Timeline

2026-11-21 UTC: audit completed; user authorized implementation on this branch for v1.2.0.

## Known issues and prioritized remaining work

Audit findings map to verified local fixes or explicit qualification/decision tickets. Final
redactor fuzzing passed859,078executions/901seconds; seeded snapshots passed2,231,269/901seconds.
Performance qualification remains open: both optimized series fail unchanged thresholds,
and unrelated host compilation was observed. Redistribution and patent/CLA questions
require qualified decisions; asset-level ledger and review packet are prepared. A local gate
does not establish successful hosted CI for an uncommitted candidate.

## Next steps

Diagnose remaining strict performance failures, resolve qualified governance decisions, and
obtain separate authorization for a reviewed commit and the hosted release process. Tests,
coverage, bounded mutation rechecks, sanitizer qualification and local packages pass; none
replace successful exact-candidate hosted CI.

## Readiness checklist

- [x] Authorized code fixes and enhancements independently verified within recorded scope
- [x] Compatibility and migration tests pass
- [ ] Full quality/security/performance gates pass
- [ ] Documentation and governance decisions reconciled
- [x] Four Linux variants pass local package qualification; committed-release certification pending
- [ ] Separate instruction received for commits, merge, tag and publication
