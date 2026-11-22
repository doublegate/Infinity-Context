# Sprint 2 — Timeline and Repo Map

**Phase:** Phase 11 — Govern and Distribute  
**Release:** v1.9.0  
**Sprint Goal:** Show what happened around a recall hit through `op:"timeline"`, and give agents a
token-budgeted tree-sitter repo map ranked by the existing PPR code.  
**Estimated Duration:** 1.5 weeks

## Research gate

Before any ticket starts:

- Re-read `crates/icx-store` (`BiTemporalGraph`, `PersonalizedPageRank`) to confirm which edges
  and timestamps a timeline query can rely on.
- Check the current `tree-sitter` crate and grammar crate versions via Context7, and the license
  of each grammar to be bundled.
- Re-read Aider's repo-map write-up and source for the ranking and budget method.
- Evaluate the `repo-mapper` crate (maintenance, license, API, output quality) before writing one.
- Re-run the Phase 7 retrieval evaluation set and record recall@k and MRR as the baseline.

### Research notes

_None recorded yet._

## Tickets

### T-11-2-001 — `op:"timeline"` over the bi-temporal graph

**Description:** Add a `timeline` op to the MCP pager that returns the events before and after a
hit (tool calls, folds, recalls) in valid-time order, bounded by a count and a token budget, and
filtered through the policy layer.

**Research:** Confirm that valid-time and transaction-time are both recorded for every edge type
the timeline needs, and that ordering is stable for events with equal timestamps.

**Acceptance Criteria:**

- [ ] Returns a bounded, ordered window around an `#icx:` handle or a recall hit.
- [ ] Denied paths and `<private>` content never appear (tested against T-11-1-002).
- [ ] Documented in `docs/wire-protocol-and-shm-bus.md` (pager ops) and `docs/interfaces.md`.

**Dependencies:** T-11-1-002  
**Reference:** Plan Phase 11 Sprint 2; `docs/hyperdimensional-and-fractal-index.md` (bi-temporal
graph)  
**Estimated Complexity:** M

---

### T-11-2-002 — Evaluate `repo-mapper` against a native implementation

**Description:** Decide whether to depend on `repo-mapper` or build the map on `tree-sitter`
directly.

**Research:** Hypothesis: `repo-mapper` produces symbol maps of usable quality for Rust, Python,
TypeScript and Go at an acceptable build-time and binary-size cost. Confirming measurement: map
quality on three repositories of this workspace, build time and binary size delta. Rejection
closes this ticket with the finding and T-11-2-003 builds natively.

**Acceptance Criteria:**

- [ ] Decision recorded with the measurements in the sprint Research notes.
- [ ] Binary-size and build-time deltas measured and published in `docs/numbers.md`.

**Dependencies:** None  
**Reference:** Plan Phase 11 Sprint 2; `repo-mapper` crate; Aider repo map  
**Estimated Complexity:** S

---

### T-11-2-003 — Token-budgeted tree-sitter repo map ranked by PPR

**Description:** Build a repo map of definitions and references, ranked by the existing
`PersonalizedPageRank` personalised by recent recall hits (the Aider pattern), and trimmed to a
token budget. Exposed through the pager and optionally injected at session start.

**Research:** Hypothesis: PPR personalised by recall hits ranks the files an agent next touches
higher than unpersonalised PageRank. Confirming measurement: hit rate of the top-k map entries
against files actually read in the next turns of recorded sessions. Rejection ships the map
unpersonalised and records the finding.

**Acceptance Criteria:**

- [ ] Map stays within the configured token budget (tested).
- [ ] Ranking comparison measured and published in `docs/numbers.md`.
- [ ] Denied paths from the policy file are excluded.
- [ ] Optional feature; default builds unchanged when off.

**Dependencies:** T-11-2-002, T-11-1-002  
**Reference:** Plan Phase 11 Sprint 2; Aider repo map; `crates/icx-store` PPR  
**Estimated Complexity:** L

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Coverage >= 95% (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Docs and `CHANGELOG.md` (`[Unreleased]`) updated in the same change.
- [ ] Upstream re-scan for Sprint 3 done, its tickets adjusted, and findings recorded in its
  Research notes.
