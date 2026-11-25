# Sprint 2 — Evaluation

**Phase:** Phase 7 — Retrieval and Compaction Quality

**Release:** v1.5.0

**Sprint Goal:** Measure retrieval on real sessions, decide the default ranking from that
measurement, and evaluate optional local embeddings against the same set.

**Estimated Duration:** 2 weeks

## Research gate

Re-check before the sprint starts:

- **External docs and versions:** `ort` (ONNX Runtime) and `candle` via Context7, including
  their `unsafe` and native-library footprint; small code-aware embedding models and their
  licences.
- **Installed agent behaviour:** the session log formats of Claude Code, Codex and agy on the
  installed versions, for extracting queries and the artifacts that answered them.
- **Project evidence:** `docs/numbers.md` lines 8-13 (27-row synthetic corpus, recall@1/@5);
  `docs/releases/v1.2.0/evidence/retrieval.csv`; Sprint 1 rankers.

### Research notes

_None recorded yet._

### Research notes (2026-11-24, first real-session run)

- **Owner decision:** use this project's sessions; widen to all projects only if that measurably
  improves the evaluation. Queries and judgements stay in gitignored `eval/private/`; only
  aggregate scores are committed.
- **Judgements:** explicit `recall` is too rare to judge (6 calls, 5 recall-then-deref pairs in 43
  transcripts), and injected recall lines were followed by a deref of their handle 6 times out of
  173, so neither source works. Instead each case is a `deref`/`slice` of a handle the agent got
  from a stub (handles first seen in a recall line are excluded, to avoid favouring `legacy`).
- **Two query sources** (side-agent review): the agent's note just before the deref, and the
  user prompt that started the turn. Automatic recall searches with the prompt, so the prompt set
  decides a default; both are scored.
- **Size and split:** 733 note cases (18 transcripts) and 110 prompt cases (6 transcripts). One
  session holds almost all of them, so the split is chronological (older half train), not by
  session. This is the measured reason to widen to other projects' sessions.
- **First scores (held-out, recall@10):** notes legacy 1.4%, bm25 9.5%, rrf 3.5%; prompts legacy
  1.8%, bm25 10.9%, rrf 9.1%. BM25 leads; RRF with the HDC list is worse than BM25 alone.
- **Time leak, fixed:** each query now ranks only artifacts that existed when it was asked, and a
  newest-first baseline is scored. It wins outright (recall@1 83–88%): agents dereference the stub
  they just received. Only 7 of about 2,100 cases across six projects had an answer with 20 or
  more newer artifacts, so widening to all projects did not yield a long-range set. Decision:
  ADR-0010.
- **Prompt timing, corrected (review of #98):** prompt cases are now timed at the prompt and
  keyed per turn. 311 of 314 then needed outputs produced later in the turn, so automatic recall
  had an existing answer to surface in only 3.

## Tickets

### T-72-001 — Real-session retrieval evaluation set

**Description:** Build an evaluation set from real agent sessions: queries the agent made (or
needed), and the artifacts that answered them. Every session is consented, and all text is
redacted before it is stored.

**Research:** Decide the judgement method (recorded `deref` after `recall`, or manual labels)
and check how many queries are needed for a usable train and held-out split.

**Acceptance Criteria:**

- [x] Consent and redaction procedure documented; gitleaks clean over the set.
- [x] Set size, sources and split recorded; held-out split never used for tuning.
- [x] If raw text cannot be published, only derived queries and judgements are committed, and
  the private part is documented as such.

**Dependencies:** None

**Reference:** `docs/numbers.md` lines 8-13 (synthetic corpus limits); plan Phase 7 Sprint 2

**Estimated Complexity:** L

---

### T-72-002 — recall@k and MRR evaluation harness

**Description:** Add a harness that scores any ranker on a query set by recall@1, recall@5,
recall@10 and MRR, and writes a result file with commit, seed and set size.

**Research:** None beyond agreeing the metric definitions with `docs/testing-strategy.md`.

**Acceptance Criteria:**

- [x] Harness runs on the synthetic set and on the T-72-001 set.
- [x] Output is machine-readable and feeds `docs/numbers.md`.
- [x] Deterministic for a fixed seed (test).

**Dependencies:** None

**Reference:** `docs/numbers.md` lines 8-13; `docs/testing-strategy.md`

**Estimated Complexity:** M

---

### T-72-003 — Optional local embeddings, off by default

**Description:** Add an optional embedding ranker (ONNX or Candle) behind a Cargo feature and a
config switch, both off by default, and fuse it into RRF when enabled.

**Research:** Hypothesis: a small local embedding model improves recall@5 on the real-session
held-out split over BM25 plus HDC with RRF. Measured with T-72-002. If rejected, the feature is
not merged, and the finding is recorded.

**Acceptance Criteria:**

- [ ] Feature off: build, tests and binaries unchanged.
- [ ] Model licence, size and source recorded; no download without explicit user action.
- [ ] recall@k, MRR, ingest time and memory measured and published in `docs/numbers.md`.

**Result (2026-11-25): closed without a feature.** The hypothesis (a gain over BM25 plus HDC on
the held-out real set) cannot be measured: real-session answers are almost always just produced
(7 of about 2,100 cases had 20 or more newer artifacts), so no long-range set exists to test it on.
Per the sprint rule nothing is merged; revisit with a long-range set (ADR-0010).

**Dependencies:** T-71-002, T-72-001, T-72-002

**Reference:** Plan Phase 7 Sprint 2; `ref-docs/research-report.md`

**Estimated Complexity:** L

---

### T-72-004 — Default ranking decision

**Description:** Using the held-out split, decide the default for `recall.ranking` and for
embeddings, and record the decision with the numbers.

**Research:** Re-run every ranker on the final set at the release commit.

**Acceptance Criteria:**

- [x] Decision recorded as an ADR with the measured results.
- [x] A ranking becomes the default only if it beats `legacy` on the held-out split and does
  not lower recall@1 on the synthetic set.
- [x] `docs/numbers.md` updated with the final table.

**Dependencies:** T-71-002, T-71-003, T-72-001, T-72-002

**Reference:** Plan Phase 7 Sprints 1 and 2

**Estimated Complexity:** S

## Sprint exit

- [ ] Gates green: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and
  coverage `>= 95%`.
- [ ] Docs and CHANGELOG updated for any user-visible change.
- [ ] Upstream re-scan done for Sprint 3 (compaction hooks in Claude Code, Codex and Hermes),
  and its tickets adjusted.
