# Phase 7 (`v1.5.0`) Implementation Plan — Retrieval and Compaction Quality

## Goal Description

Phase 7 moves retrieval from "100% on a synthetic identifier corpus" to "measured on real
sessions", and moves compaction from "seal the epoch" to "hand the agent what it needs to
resume". It also fixes the two known redaction weaknesses: stores written before a rule change
are never re-checked, and the line-based rule over-redacts. Each sprint lands as its own pull
request; the release is cut at the phase close.

## Ordered Approach

1. **Sprint 1 — Ranking.** Add a BM25 plus trigram index and fuse it with HDC and exact postings
   by reciprocal rank fusion (RRF). Raise the prescreen to at least 1,024 bits with a candidate
   cap that scales with the corpus. Fix the fractal-suffix false positives (ICX-AUD-052). New
   ranking ships behind a config switch, default off, until Sprint 2 measures it.
2. **Sprint 2 — Evaluation.** Build a retrieval evaluation set from real sessions, redacted and
   consented, scored by recall@k and MRR. Measure Sprint 1 against it and decide defaults. Add
   optional local embeddings (ONNX or Candle), off by default, enabled only on a measured gain.
3. **Sprint 3 — Compaction.** Inject a resume packet (handles plus open tasks, within a byte
   budget) via `PreCompact`/`PostCompact` for Claude Code and Codex and via Hermes
   `select_context`. Add agent-directed, typed eviction through `fold`, following the T-64-008
   decision.
4. **Sprint 4 — Redaction.** Ship `icx rescan` (dry-run default). Write the structural redactor
   ADR, then implement it with property tests and a field corpus. Close the mutation gaps in
   `redact.rs`.

Sprint 1 and Sprint 2 overlap: the evaluation harness (T-72-002) can start in Sprint 1 on the
synthetic corpus, so Sprint 1 changes are measured as soon as the real-session set lands.

## Research Rule

Every sprint file opens with a research gate, and every ticket has a Research line. Research-
backed tickets (ranking, embeddings, eviction policy) state a hypothesis and the measurement on
the evaluation set that confirms or rejects it. A rejected hypothesis closes the ticket with the
finding, and the feature does not ship on by default.

## Risks

- **Privacy of the evaluation set.** Real sessions contain user code and possibly secrets.
  Mitigation: explicit consent per session, redaction before anything is stored, gitleaks on
  the set, and only derived queries and judgements committed if raw text cannot be published.
- **Overfitting to a small set.** A few dozen queries can make any change look good.
  Mitigation: a held-out split, the 27-row synthetic set kept as a regression check, and
  results reported with the set size.
- **Latency regressions.** BM25, trigram and embeddings add work per query and per ingest.
  Mitigation: measure query and ingest time before and after, and publish both in
  `docs/numbers.md`; embeddings stay off by default.
- **Resume packets cost tokens and cache.** Injected context can break the prompt-cache prefix.
  Mitigation: inject only at compaction, where the cache is rebuilt anyway, and within a byte
  budget.
- **Redaction regressions.** A new redactor can miss secrets the old one caught. Mitigation:
  the new redactor must catch every case the old one caught in the existing tests and corpus,
  plus the property tests, before it replaces it.
- **Rescan writes.** Re-redacting stores rewrites user data. Mitigation: dry-run by default,
  explicit confirmation, and a backup or new CAS objects rather than in-place edits.

## Verification

- recall@k and MRR on the real-session set and the synthetic set, measured and published in
  `docs/numbers.md` with set size, seed and commit.
- Query and ingest latency before and after each ranking change, measured and published in
  `docs/numbers.md`.
- Property tests for the redactor and the resume-packet budget; mutation score for `redact.rs`
  re-run and recorded.
- Gates: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, and
  `cargo llvm-cov --workspace --all-targets --fail-under-lines 95`.
- `docs/hyperdimensional-and-fractal-index.md`, `docs/alien-holographic-codec.md` (redaction)
  and `docs/security-and-threat-model.md` updated in the same change as the code.
