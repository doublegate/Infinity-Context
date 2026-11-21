# Infinity-Context adversarial audit

**Subject:** v1.1.0 at `a996d59f1eb468e7c386747e15f4c7e58d08410e`.
**Branch:** `codex-audit`, created from `ci/release-workflow`.
**Audit date:** 2026-11-20 America/New_York / 2026-11-21 UTC.

This report suite implements the approved audit plan. It contains new findings, source checks,
reproducible isolated experiments, primary-source research, prior-finding reassessment and
remediation recommendations. It makes **no production fixes**. The starting `AGENTS.md`
modification and prior audit/reference material are preserved. No commit, push, merge, release,
paid provider call, live-agent configuration or user-memory-store change was made.

Start with [the executive assessment](00-executive-summary.md), then
[the consolidated register](10-findings-register.md) and [remediation roadmap](11-remediation-roadmap.md).

| Report | Purpose |
| --- | --- |
| [00 — Executive assessment](00-executive-summary.md) | Principal risks, strengths, readiness and validation limits |
| [01 — AHTL conformance](01-ahtl-conformance.md) | Complete PDF provenance/page inventory, normative matrix, codec/redaction findings |
| [02 — Mathematics and retrieval](02-mathematics-and-retrieval.md) | Independent derivations, index snapshots, graph oracle and27dataset scaling matrix |
| [03 — Integrity, persistence, workspace and daemon](03-integrity-persistence-security.md) | Exact bytes, ownership/seals, transactions, crash and failed-flush evidence |
| [04 — Bus and concurrency](04-bus-concurrency.md) | Sequence/loss invariants, corruption/reset behavior, real separate-process propagation |
| [05 — Integrations and proxy](05-integrations-proxy.md) | Claude/Codex/agy/Hermes/MCP contracts, HTTP/SSE controls and rejected curl lead |
| [06 — Validation and performance](06-validation-performance.md) | Exact gates, coverage, fuzz/mutation outcomes, portable/native repetitions |
| [07 — Supply chain and governance](07-supply-chain-governance.md) | Dependencies, workflows, release controls, licenses and qualified legal questions |
| [08 — Claims and documentation](08-documentation-and-claim-traceability.md) | Material-claim matrix, current-vs-historical distinctions, Unicode table check |
| [09 — Context budget and enhancements](09-context-budget-and-enhancements.md) | Unbounded epoch-window probe and measured/source-based optimization proposals |
| [10 — Findings register](10-findings-register.md) / [CSV](findings.csv) | Stable IDs with categories, severity and links to complete evidence |
| [11 — Remediation roadmap](11-remediation-roadmap.md) | Priority, dependencies, effort, compatibility and acceptance checks |
| [12 — Research ledger](12-research-ledger.md) | Primary-source versions/access dates,22arXiv identity checks and research gaps |
| [13 — Prior reassessment](13-prior-audit-reassessment.md) | Every one of51actual old finding rows, preserving scoped judgments |
| [14 — Coverage ledger](14-coverage-ledger.md) | All519tracked paths accounted for; review depth and explicit exclusions |
| [15 — Rejected leads/procedures](15-rejected-leads-and-procedures.md) | Counterevidence and reproducibility entry points |

## Evidence and method

[Baseline](evidence/baseline.json), [per-path inventory](evidence/inventory.csv),
[claim leads](evidence/documentary-claim-leads.csv), command/exit ledgers and sanitized harnesses
live in [evidence/](evidence/). Mutable builds, mutations, synthetic keys/workspaces/databases and
child-process experiments use `/tmp/Infinity-Context/`. Exceptions and environment failures are
recorded in validation procedures. The reference PDF itself was not modified or duplicated.

Findings require a reachable source path or a reproducible contract counterexample and record
prerequisites, impact, confidence, counterevidence, recommendations and acceptance criteria.
Security conclusions state the relevant trust boundary; robustness/library defects are not
automatically treated as remote exploits. Reserved IDs and rejected leads do not inflate counts.
Independent root rechecks reproduced the CLI YAML leak, varied10k recall result, daemon ACK/loss
and failed-final-flush behavior. A countertest rejected the original curl-retention allegation.

Closeout checks are retained in [verification](evidence/closeout-verification.json) and
[REUSE results](evidence/closeout-reuse.log). The original `AGENTS.md` EOF whitespace warning
is preserved with its starting diff. No audited input was changed to make a check pass.

## Scope of assurance

The **path inventory is complete**, while review depth varies. Large historical fixtures,
generated assets and external archives have targeted/sampled dispositions rather than blanket
semantic approval. The58-page PDF was completely extracted, with selected equations visually
checked. Material claims are grouped and adjudicated; the826extracted candidate lines are not
826independent proofs. Full mutation-space execution, formal concurrency validation, every
crash point, all external citations/licenses, unsupported platforms and live model/provider
behavior are explicit gaps. See report14 before interpreting this as exhaustive certification.

Baseline correctness gates and adversarial findings answer different questions. Passing tests,
95% line coverage, doctor or REUSE does not certify product claims, secret recall, strong test
oracles, live integration or redistribution rights. Strict benchmark failures and fuzz crashes
are reported without production remediation because fixes were outside the approved scope.
