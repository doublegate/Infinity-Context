# Sprint 2 — Fractal Chaos-Game Archive (`FCGA`) & Titans Surprise Gate (COMPLETED)

**Phase:** Phase 3 — Hyperdimensional, Fractal & Graph Index  
**Sprint Goal:** Implement the Contractive IFS `FractalTrajectoryArchive` (`arXiv:2607.07144`) and `TitansSurpriseGate` (`arXiv:2501.00663`) in `crates/icx-hdc`.  
**Estimated Duration:** 1 week

## Tickets

### T-32-001 — Implement Contractive Iterated Function System `FractalCoord` & Suffix Matcher (COMPLETED)

**Description:** Implement `FractalCoord::step(&Hypervector)`, `FractalCoord::shared_suffix_depth(&FractalCoord)`, `euclidean_distance`, `to_q1_14` / `from_q1_14`, 64-bit Morton Z-order prefix codes (`morton_z64`), `@o1:o2` octal quadrant addressing (`encode_octal_addr` / `decode_octal_addr`), and `FractalTrajectoryArchive` in `crates/icx-hdc/src/fractal.rs`.

**Acceptance Criteria:**
- [x] Two trajectories with different prefixes followed by the exact same $K$-step suffix (`K <= 32`) yield `shared_suffix_depth >= K` in $O(1)$ time and satisfy Theorem 3.2 bounds $d_\infty \le 2^{1-K}$ and $d_2 \le 4 d_\infty \le 2^{3-K}$.
- [x] `FractalTrajectoryArchive` indexes historical trajectory states by 64-bit Morton Z-order prefix (`morton_z64`) and retrieves shared-suffix trajectory matches.

**Dependencies:** T-31-001  
**Reference:** `docs/hyperdimensional-and-fractal-index.md` §3, `docs/ahtl/03-fractal-ifs-chaos-game-coordinates.md`; `ref-docs/papers/2607.07144-Fractal-KV-Cache-Archives.pdf`  
**Estimated Complexity:** M

---

### T-32-002 — Implement `TitansSurpriseGate` Momentum & Eviction Classifier (COMPLETED)

**Description:** Implement `TitansSurpriseGate` in `crates/icx-hdc/src/surprise.rs` computing instantaneous associative surprise $g_t$, momentum $S_t = \eta S_{t-1} + (1-\eta)g_t$, idle forgetting rate $\alpha$, `observe_with_exit_code`, and tier assignment (`PinVerbatim` / `PinHighResolution`, `FoldAhtlStub`, `ColdBeacon`).

**Acceptance Criteria:**
- [x] High-surprise anomaly turns (and non-zero exit codes) trigger elevated momentum $S_t$ that persists across the immediately following diagnostic/fix turns before decaying smoothly into `FoldAhtlStub` and `ColdBeacon`.

**Dependencies:** T-31-002  
**Reference:** `docs/hyperdimensional-and-fractal-index.md` §4, `docs/ahtl/07-transformer-attention-and-bpe-mechanics.md` §4; `ref-docs/papers/2501.00663-Titans-Learning-to-Memorize-at-Test-Time.pdf`  
**Estimated Complexity:** M
