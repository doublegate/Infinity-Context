# ADR 0010: Recall ranking stays `legacy` by default; lexical ranking is opt-in

Status: accepted, 2026-11-25 UTC. Phase 7 Sprint 2 (T-72-004), with T-71-002 and T-72-003.

## Context

v1.5.0 adds a lexical index (BM25 over words, trigrams for substrings; T-71-001) and reciprocal
rank fusion (RRF, k = 60) of it with the HDC ranking (T-71-002). The sprint rule is that a ranking
becomes the default only if it beats `legacy` on a held-out real-session set without lowering
recall@1 on the synthetic set, and that an unconfirmed feature is not merged.

`icx eval-recall` (T-72-002) scores rankers by recall@1/5/10 and MRR@10. The real-session set
(T-72-001) is built from Claude Code transcripts. Each case is a `deref` or `slice` of a handle the
agent got from a stub, and its query is either the agent's note before the call or the user's
prompt that started the turn. Each query ranks only the artifacts that existed when it was asked.
Queries and judgements stay private (`eval/private/`); only these aggregates are published.

Measured (2026-11-24, all six projects with at least 40 dereferences, about 2,100 cases):

- **Answers are almost always fresh.** Ranked newest-first, the needed output is first in 83–88%
  of cases and in the top 5 in 99–100%. Only 7 cases had 20 or more newer artifacts between the
  answer and the query, and 2 had 100 or more. Widening from this project to all six did not
  change that.
- **On those fresh-answer cases** (this project, held-out half), recall@10 was: notes, legacy 2.4%,
  BM25 12.5%, RRF 5.2%, newest-first 99.2%; prompts, legacy 3.6%, BM25 10.7%, RRF 7.1%,
  newest-first 100%. They measure fetching what was just produced, which recency answers alone,
  not the long-range recall that automatic recall exists for.
- **Synthetic sets** (bag-of-words known-item queries) put the three rankers within a few points
  of each other (`docs/numbers.md`).
- **Automatic recall** (the injected line) was followed by a dereference of its handle 6 times
  out of 173 in this project's sessions.

## Decision

- **Default stays `legacy`.** No ranking was shown to beat it on long-range recall, because the
  real-session data holds almost no long-range needs to judge.
- **`ICX_RECALL_RANKING=rrf` is opt-in.** It maintains the lexical index and fuses BM25 with the
  HDC hits for the injected recall line. With it off, recall output is byte-identical to v1.4.x
  (tested).
- **Embeddings (T-72-003) are not built.** Their hypothesis, a gain over BM25 plus HDC on the
  held-out real set, cannot be measured on data with no long-range needs; per the sprint rule, the
  finding is recorded and no feature is merged.

## Consequences

- v1.5.0 ships the index, fusion and harness without changing what any user sees by default.
- **Open question for the owner:** whether automatic recall should change at all (a stricter
  threshold, fewer lines, or none) given how rarely its line is used. That needs a measure of
  benefit beyond "was the handle opened" (an agent may use the subject line without opening it),
  which this harness does not yet have.
- **To revisit the default:** build a long-range set, for example from sessions that resume after
  a compaction or a restart, where the needed output is no longer in context. `icx eval-recall
  --min-age <N>` already selects such cases when they exist.
