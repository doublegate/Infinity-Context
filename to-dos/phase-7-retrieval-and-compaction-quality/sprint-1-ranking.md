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

- [ ] Index kept in SQLite FTS5 tables of the workspace store (a porter `unicode61` table for
  BM25, a `trigram` table for substrings), written at ingest; rebuildable from the CAS.
- [ ] Tokenizer handles code identifiers (camelCase, snake_case, paths) with tests.
- [ ] Ingest and query time measured and published in `docs/numbers.md`.
- [ ] Store schema change versioned through a migration and documented.

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

- [ ] Prescreen width and cap formula documented in `docs/hyperdimensional-and-fractal-index.md`.
- [ ] Stage-1 recall and query time per corpus size measured and published in
  `docs/numbers.md`.
- [ ] No change to final ranking when the cap already covers the corpus (test).

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

- [ ] A failing test reproduces the false positive, then passes.
- [ ] Suffix matches are verified before they are reported.
- [ ] ICX-AUD-052 moved to fixed in `docs/claude/findings.csv` with the commit.

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
