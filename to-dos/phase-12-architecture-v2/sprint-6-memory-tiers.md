# Sprint 6 — Memory Tiers

**Phase:** Phase 12 — Architecture Release  
**Release:** v2.0.0  
**Sprint Goal:** Organise stored context into L1–L4 tiers with automatic page-in on a recall
fault, following novel-methods §4, without a significant regression on the Phase 10 paired
benchmark.  
**Estimated Duration:** 2 weeks

## Research gate

Before any ticket starts:

- Re-read `ref-docs/novel-methods/non-human-neural-codecs-and-holographic-context.md` §4 and the
  MemGPT and Titans papers in `ref-docs/papers/` for tier and paging designs.
- Re-measure recall latency per store layer (in-memory index, CAS on disk, Phase 9 storage) and
  record it.
- Re-run the Phase 7 evaluation set and the Phase 10 paired benchmark as the baseline.

### Research notes

_None recorded yet._

## Tickets

### T-12-6-001 — Define L1–L4 tiers and their residency rules

**Description:** Specify the tiers (for example L1 active window, L2 hot index, L3 local CAS,
L4 cold or archived store), what each holds, and the promotion and demotion rules, in
`docs/virtual-memory-and-dag-engine.md`.

**Research:** Hypothesis: four tiers map onto the existing structures without a new store.
Confirming evidence: a mapping table from tiers to current code. Rejection records which tier
needs new storage.

**Acceptance Criteria:**

- [ ] Tier spec committed with residency, promotion and demotion rules.

**Dependencies:** None  
**Reference:** Plan Phase 12 Sprint 6; novel-methods §4; MemGPT  
**Estimated Complexity:** M

---

### T-12-6-002 — Automatic page-in on a recall fault

**Description:** When recall or deref misses in a fast tier, page the artifact in from the lower
tier transparently and record the fault in the trace.

**Research:** Hypothesis: page-in on fault improves recall@k on the evaluation set at bounded
latency. Confirming measurement: evaluation set and fault-latency distribution. Rejection keeps
explicit deref only and records the finding.

**Acceptance Criteria:**

- [ ] Faults are paged in and traced (tested).
- [ ] Recall@k change and fault latency measured and published in `docs/numbers.md`.

**Dependencies:** T-12-6-001, T-72-001  
**Reference:** Plan Phase 12 Sprint 6; novel-methods §4  
**Estimated Complexity:** L

---

### T-12-6-003 — Tier eviction within budgets

**Description:** Evict from upper tiers under token and memory budgets, honouring policy retention
and pins.

**Research:** Compare LRU with surprise-weighted eviction (Titans gate) on recorded sessions.

**Acceptance Criteria:**

- [ ] Budgets enforced (tested); pinned and policy-retained items never evicted below their tier.
- [ ] Eviction comparison measured and published in `docs/numbers.md`.

**Dependencies:** T-12-6-001, T-11-1-002  
**Reference:** Plan Phase 12 Sprint 6; `crates/icx-hdc` (`TitansSurpriseGate`)  
**Estimated Complexity:** M

---

### T-12-6-004 — Paired-benchmark regression check for tiers

**Description:** Run the Phase 10 paired benchmark with tiers enabled.

**Research:** Apply the Phase 10 noise-aware rule.

**Acceptance Criteria:**

- [ ] No significant regression under the noise-aware rule; result published in
  `docs/numbers.md`.

**Dependencies:** T-12-6-002, T-12-6-003, T-10-5-004  
**Reference:** Phase 12 overview ("Changes to the v2.0 plan")  
**Estimated Complexity:** S

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Coverage >= 95% (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Docs and `CHANGELOG.md` (`[Unreleased]`) updated in the same change.
- [ ] Upstream re-scan for Sprint 7 done, its tickets adjusted, and findings recorded in its
  Research notes.
