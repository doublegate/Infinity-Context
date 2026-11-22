# Phase 7 — Retrieval and Compaction Quality (`v1.5.0`)

## Goal

Make recall find the right artifact on real sessions, not only on the synthetic corpus, and
make compaction keep what the agent needs to continue. The phase adds lexical ranking fused
with the existing HDC and exact postings, builds an evaluation set from real sessions, injects
a resume packet at compaction, and replaces the line-based redactor with a structural one.

## Exit Criterion

- [ ] A redacted, consented retrieval evaluation set from real sessions exists, and recall@k
  and MRR for the shipped ranking are measured on it and published in `docs/numbers.md`.
- [ ] Any ranking or embedding change that is on by default has a measured gain on that set;
  changes without a gain stay off or are removed.
- [ ] Claude Code, Codex and Hermes receive a resume packet at compaction, within a byte budget.
- [ ] `icx rescan` reports (dry-run default) or re-redacts existing stores, and the structural
  redactor has replaced "first quote to end of line".
- [ ] Gates green: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and
  coverage `>= 95%`.

## Sprints

- [Sprint 1 — Ranking](sprint-1-ranking.md)
- [Sprint 2 — Evaluation](sprint-2-evaluation.md)
- [Sprint 3 — Compaction](sprint-3-compaction.md)
- [Sprint 4 — Redaction](sprint-4-redaction.md)

Implementation approach, risks and verification:
[phase_7_implementation_plan.md](phase_7_implementation_plan.md).

## Dependencies on Earlier Phases

- **Phase 6 (v1.4.0):** live-run contract fixtures (T-65-001) for the compaction events; the
  `fold` tool decision (T-64-008) that Sprint 3's typed eviction builds on; the Hermes shim
  (T-65-002) that `select_context` extends.
- **Phase 3** HDC index, fractal archive and PPR graph, which the new ranking fuses with.
- **v1.3.x redaction work:** the v1.3.1 rules ("redaction from the first quote") and the v1.3.0
  redactor split decision (#29), which the structural redactor supersedes.
- **`docs/numbers.md`:** the frozen 27-row v1.2.0 synthetic corpus stays as a regression set
  next to the new real-session set.

## Standing Sources to Watch

Re-check these at every sprint start and close, and record changes in the sprint's research
notes:

- Claude Code and Codex changelogs for `PreCompact`/`PostCompact` payloads and compaction
  behaviour; Hermes source for `select_context`.
- MCP specification changelog for tool and resource changes that affect recall output.
- Crate releases via Context7: lexical index and BM25 crates, `ort` (ONNX Runtime), `candle`,
  `proptest`.
- Papers and their code: CWL (arXiv 2606.11213), CAT (arXiv 2512.22087), SWE-MeM
  (arXiv 2606.28434), HippoRAG, LLMLingua-2.
- Comparable tools: context-mode (BM25 and trigram search), rtk, and secret scanners (gitleaks
  rule set) for redaction patterns.
