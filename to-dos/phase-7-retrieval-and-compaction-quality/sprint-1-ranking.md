# Sprint 1 — Ranking

**Phase:** Phase 7 — Retrieval and Compaction Quality

**Release:** v1.5.0

**Sprint Goal:** Add lexical ranking fused with HDC and exact postings, scale the prescreen with
the corpus, and remove the fractal-suffix false positives.

**Estimated Duration:** 2 weeks

## Research gate

Re-check before the sprint starts:

- **External docs and versions:** current BM25 and trigram index crates via Context7 (or an
  in-house index if no crate fits `unsafe_code = "forbid"` and `cargo deny`); the RRF method
  and its usual constant; how context-mode builds its BM25 and trigram search.
- **Installed agent behaviour:** not agent-facing; recall output shape stays as in v1.4.0.
- **Project evidence:** `HdcAssociativeIndex::query_associative` and the stage-1 candidate
  scaling in `crates/icx-hdc/src/lib.rs`; `crates/icx-hdc/src/fractal.rs` line 88; the 27-row
  synthetic corpus in `docs/numbers.md`; field recall misses from the session scan.

### Research notes

**2026-11-23, research gate** (Phase 6 Sprint 5 exit re-scan; crates.io metadata, dependency
trees resolved in a scratch crate, upstream sources):

| Option | Version (date) | License | Footprint | Fit |
| --- | --- | --- | --- | --- |
| SQLite FTS5 in the existing store | bundled SQLite 3.54.1 (`rusqlite` 0.43, `bundled`) | public domain | no new crate | `porter unicode61 remove_diacritics 2` and `trigram` tokenizers both available (checked in a probe test); the index lives in `icx.sqlite3` and updates per insert |
| `tantivy` | 0.27.0 (2026-10-17) | MIT | 118+ dependencies | BM25 and RAM directory, but no trigram tokenizer and no serialise-to-bytes API; heavy |
| `bm25` | 2.3.2 (2025-09-07) | MIT | 4 dependencies without `default_tokenizer` | BM25 only; no trigram, no serde (rebuild on load) |
| `probly-search` | 2.0.1 (2024-07-03) | MIT | 12 | dormant; no trigram |
| `fst` | 0.4.7 (2021-06-06) | Unlicense/MIT | 2 | immutable term dictionary; a building block only |
| `seekstorm` | 5.1.0 (2026-11-15) | Apache-2.0 | 277, with licenses outside the `cargo deny` allowlist | rejected |

- **RRF:** score = Σ 1/(k + rank), k = 60 (Cormack, Clarke and Büttcher, SIGIR 2009). Elastic's
  tuning found k insensitive (about 5% between best and worst) and that a weighted linear
  combination beats RRF only with labelled queries (about 40 sufficed on one set). Plain RRF is
  the default without labels.
- **context-mode** (HEAD `0fe3f5d`, v2.2.2; Elastic-2.0, so ideas only, no code): SQLite FTS5 with
  a porter `unicode61` table and a separate `trigram` table, both ranked by `bm25()`, fused by
  RRF with `RRF_K = 60`; no diversity re-ranking.
- **rust-memex** 0.6.4: weighted RRF (lexical 0.4, vector 0.6, k = 60) and at most 3 results per
  source, a diversity cap worth evaluating.
- **Not verified:** the paper's text (the PDF did not extract; k = 60 is confirmed by secondary
  sources).

**2026-11-24, T-71-003 and T-71-004 results:**

- **T-71-003:** the prescreen was already 1,024 bits with a `max(4 * sqrt(n), n / 64, ...)` cap
  (v1.1.0). It was measured at 1k, 10k and 100k (`docs/numbers.md`, v1.5.0 measurements):
  stage-1 recall is 100% for rare-word queries at every size and 98.5% for common-word queries at
  100k (corrected after the review of #97: 100% for both once query words are sampled without
  replacement). A 2,048-bit prescreen gave identical recall and was never faster. Hypothesis rejected;
  the width is unchanged, and the ticket closes with the measurement.
- **T-71-004:** measured before the fix over 20,000 events: 3 distinct and 66 similar events
  (0.33%) were false one-step suffixes, all from colliding 16-bit corners. None of the nonzero
  suffixes in the measurement was a real repeat. The fractal bonus (15% of the combined score)
  was therefore almost always zero, and when it was not, it rewarded a coincidence. After the
  fix it is nonzero only for exactly repeated event sequences.

**2026-11-24, T-71-001 design (measured on a copy of a real 54 MB store):** contentless tables
(10.2 MB for both, measured with the production code; content copies would add the text again,
and random 64-bit rowids took 38.5 MB, so rows are numbered compactly); trigrams with
`detail=none`, because that mode rejects phrase queries, so substrings are an AND of trigrams and
each candidate is confirmed against the CAS text; built only with `ICX_RECALL_RANKING=rrf`
(additive and off by default until T-72-004 decides); no schema version bump, so v1.4.x can still
open the store.

**Ticket changes:** T-71-001 uses SQLite FTS5 tables in the workspace store instead of a separate
crate or snapshot index. T-71-002 starts from plain RRF at k = 60 and adds per-list weights and a
per-source cap only if the evaluation set (Sprint 2) shows a gain.

## Tickets

### T-71-001 — BM25 plus trigram lexical index

**Description:** Add a lexical index over stored artifacts: BM25 over tokens for word queries,
and trigrams for identifiers, paths and partial strings. Persist it with the workspace snapshot.

**Research:** Hypothesis: lexical ranking finds artifacts that HDC misses for natural-language
and partial-identifier queries. Confirmed by recall@k on the T-72-002 harness, first on the
synthetic set and then on the real-session set.

**Acceptance Criteria:**

- [x] Index kept in SQLite FTS5 tables of the workspace store (a porter `unicode61` table for
  BM25, a `trigram` table for substrings), written when the workspace is saved (each hook saves
  once; unsaved artifacts are not yet searchable); rebuildable from the CAS.
- [x] Tokenizer handles code identifiers (camelCase, snake_case, paths) with tests.
- [x] Ingest and query time measured and published in `docs/numbers.md`.
- [x] Store schema change versioned through a migration and documented. (Versioned by the
  `lexical_index` meta key rather than the schema version, so a downgrade still opens the store;
  `docs/data-model.md` §6.4.)

**Dependencies:** None

**Reference:** `docs/claude/11-enhancements-and-optimizations.md` §B; context-mode comparison
(`ref-docs/prior-art/github-and-industry-landscape.md`)

**Estimated Complexity:** L

---

### T-71-002 — Reciprocal rank fusion of lexical, HDC and exact postings

**Description:** Combine the lexical, HDC and exact-posting rankings with RRF behind a config
switch (`recall.ranking = legacy|rrf`), default `legacy` until T-72-004 decides.

**Research:** Hypothesis: RRF improves MRR over the best single ranker without lowering recall@1
on the synthetic set. Measured on the T-72-002 harness; the RRF constant is chosen on the
training split only.

**Acceptance Criteria:**

- [ ] RRF with k = 60 by default (research gate, 2026-11-23); per-list weights and a per-source
  cap only if Sprint 2's evaluation set shows a gain.
- [ ] Fusion is deterministic for equal scores (stable tie-break by handle).
- [ ] `legacy` output is byte-identical to v1.4.0.
- [ ] recall@k and MRR for each ranker and for RRF measured and published in `docs/numbers.md`.

**Dependencies:** T-71-001, T-72-002

**Reference:** `docs/claude/11-enhancements-and-optimizations.md` §B

**Estimated Complexity:** M

---

### T-71-003 — Prescreen of at least 1,024 bits and a corpus-scaled candidate cap

**Description:** Widen the stage-1 prescreen to at least 1,024 bits and make the candidate cap
grow with the corpus, so large workspaces do not drop the right artifact before the full
10,240-bit comparison.

**Research:** Hypothesis: at 10k artifacts and above, the current prescreen drops correct
candidates that a wider prescreen keeps. Measured as stage-1 recall (correct item in the
candidate set) at 1k, 10k and 100k artifacts, before and after.

**Acceptance Criteria:**

- [x] Prescreen width and cap formula documented in `docs/hyperdimensional-and-fractal-index.md`.
- [x] Stage-1 recall and query time per corpus size measured and published in
  `docs/numbers.md`.
- [x] No change to final ranking when the cap already covers the corpus (test).

**Dependencies:** None

**Reference:** `docs/claude/11-enhancements-and-optimizations.md` §B;
`docs/claude/03-hdc-fractal-math.md`

**Estimated Complexity:** M

---

### T-71-004 — Fix fractal-suffix false positives

**Description:** Fractal suffix depth is inferred from distance, so unrelated artifacts can be
reported as suffix matches. Derive depth from stored structure instead, or verify a candidate
suffix against the content before reporting it.

**Research:** Reproduce the false positive with a failing test first, using the case in the
audit finding.

**Acceptance Criteria:**

- [x] A failing test reproduces the false positive, then passes.
- [x] Suffix matches are verified before they are reported.
- [x] ICX-AUD-052 moved to fixed in `docs/claude/findings.csv` with the commit.

**Dependencies:** None

**Reference:** ICX-AUD-052 (`docs/claude/03-hdc-fractal-math.md` line 79;
`crates/icx-hdc/src/fractal.rs` line 88)

**Estimated Complexity:** M

## Sprint exit

- [ ] Gates green: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and
  coverage `>= 95%`.
- [ ] Docs and CHANGELOG updated for any user-visible change.
- [ ] Upstream re-scan done for Sprint 2 (embedding runtimes, evaluation methods), and its
  tickets adjusted.
