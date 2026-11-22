# Sprint 7 — Neural Tier (Optional, Off by Default)

**Phase:** Phase 12 — Architecture Release  
**Release:** v2.0.0  
**Sprint Goal:** Evaluate and, only if it wins, ship an optional neural tier: an ONNX or Candle
encoder and GPU RVQ/ICAE compression on idle turns, competing against the v1.5 BM25+RRF baseline
on both the evaluation set and the paired benchmark.  
**Estimated Duration:** 2.5 weeks

## Research gate

Before any ticket starts:

- Re-read `ref-docs/research-report.md` (around :165), `CLAUDE.md` §5 (CUDA RVQ on the local
  GPU), and the RVQ and ICAE papers in `ref-docs/papers/`.
- Check current ONNX Runtime Rust bindings and Candle releases via Context7, and their CUDA
  requirements.
- Re-run the Phase 7 evaluation set and the Phase 10 paired benchmark for the BM25+RRF baseline.
- Use `agent-workstation resource gpu` for any local GPU run.

### Research notes

_None recorded yet._

## Tickets

### T-12-7-001 — Hypothesis and gate: neural recall beats BM25+RRF

**Description:** State the hypothesis and the gate before building: a neural encoder improves
recall@k and MRR over BM25+RRF on the Phase 7 evaluation set and shows no regression on the
paired benchmark.

**Research:** Hypothesis as stated. Confirming measurement: a prototype encoder on the evaluation
set. Rejection closes this sprint with the finding recorded, and T-12-7-002 to T-12-7-004 close
unbuilt.

**Acceptance Criteria:**

- [ ] Prototype result measured and published in `docs/numbers.md`.
- [ ] Go or no-go decision recorded in the Research notes.

**Dependencies:** T-71-002, T-72-001  
**Reference:** Plan Phase 12 Sprint 7; Phase 12 overview ("Changes to the v2.0 plan")  
**Estimated Complexity:** M

---

### T-12-7-002 — ONNX or Candle encoder behind a default-off feature

**Description:** Integrate the chosen encoder behind a `neural` feature, off by default, fused
into the existing RRF ranking.

**Research:** Compare ONNX Runtime and Candle on build size, CPU-only fallback and licensing.

**Acceptance Criteria:**

- [ ] Default build unchanged with the feature off (`cargo tree`, tests).
- [ ] CPU fallback works without a GPU (tested).

**Dependencies:** T-12-7-001  
**Reference:** Plan Phase 12 Sprint 7; research-report :165  
**Estimated Complexity:** L

---

### T-12-7-003 — GPU RVQ/ICAE compression on idle turns

**Description:** Compress dense representations with RVQ (and evaluate ICAE) on the GPU during
idle turns only, never on the hook path.

**Research:** Hypothesis: idle-turn compression reduces stored size without hurting recall.
Confirming measurement: size and recall@k with and without compression. Rejection drops
compression and keeps the encoder.

**Acceptance Criteria:**

- [ ] Hook-path latency unaffected: measured and published in `docs/numbers.md`.
- [ ] Size and recall effects measured and published in `docs/numbers.md`.

**Dependencies:** T-12-7-002  
**Reference:** Plan Phase 12 Sprint 7; `CLAUDE.md` §5; RVQ and ICAE papers in `ref-docs/papers/`  
**Estimated Complexity:** XL

---

### T-12-7-004 — Head-to-head on the evaluation set and the paired benchmark

**Description:** Run the neural tier against the BM25+RRF baseline on both the Phase 7
evaluation set and the Phase 10 paired benchmark, and ship it only if it wins on the first and
does not regress on the second.

**Research:** Apply the Phase 10 noise-aware rule.

**Acceptance Criteria:**

- [ ] Both comparisons measured and published in `docs/numbers.md`.
- [ ] Ship or drop decision recorded in `CHANGELOG.md` and the Research notes.

**Dependencies:** T-12-7-002, T-10-5-004  
**Reference:** Phase 12 overview ("The neural tier competes against the v1.5 BM25+RRF baseline")  
**Estimated Complexity:** M

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Coverage >= 95% (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Docs and `CHANGELOG.md` (`[Unreleased]`) updated in the same change.
- [ ] Upstream re-scan for Sprint 8 done, its tickets adjusted, and findings recorded in its
  Research notes.
