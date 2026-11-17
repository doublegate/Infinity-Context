# Sprint 2 — Fractal Chaos-Game Archive (`FCGA`) & Titans Surprise Gate

**Phase:** Phase 3 — Hyperdimensional, Fractal & Graph Index  
**Sprint Goal:** Implement the Contractive IFS `FractalTrajectoryArchive` (`arXiv:2607.07144`) and `TitansSurpriseGate` (`arXiv:2501.00663`) in `crates/icx-hdc`.  
**Estimated Duration:** 1 week

## Tickets

### T-32-001 — Implement Contractive Iterated Function System `FractalCoord` & Suffix Matcher

**Description:** Implement `FractalCoord::step(&Hypervector)` and `FractalCoord::shared_suffix_depth(&FractalCoord)` in `crates/icx-hdc/src/fractal.rs`.

**Acceptance Criteria:**
- [x] Two trajectories with different prefixes followed by the exact same $K$-step suffix (`K <= 32`) yield `shared_suffix_depth >= K` in $O(1)$ time.

**Dependencies:** T-31-001  
**Reference:** `docs/hyperdimensional-and-fractal-index.md` §3; `ref-docs/papers/2607.07144-Fractal-KV-Cache-Archives.pdf`  
**Estimated Complexity:** M

---

### T-32-002 — Implement `TitansSurpriseGate` Momentum & Eviction Classifier

**Description:** Implement `TitansSurpriseGate` in `crates/icx-hdc/src/surprise.rs` computing instantaneous associative surprise $g_t$, momentum $S_t = \eta S_{t-1} + (1-\eta)g_t$, and tier assignment (`PinVerbatim`, `FoldAhtlStub`, `ColdBeacon`).

**Acceptance Criteria:**
- [x] High-surprise anomaly turns trigger elevated momentum $S_t$ that persists across the immediately following diagnostic/fix turns before decaying smoothly.

**Dependencies:** T-31-002  
**Reference:** `docs/hyperdimensional-and-fractal-index.md` §4; `ref-docs/papers/2501.00663-Titans-Learning-to-Memorize-at-Test-Time.pdf`  
**Estimated Complexity:** M
