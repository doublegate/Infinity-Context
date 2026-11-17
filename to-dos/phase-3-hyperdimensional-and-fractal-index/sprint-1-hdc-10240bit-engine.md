# Sprint 1 — 10,240-Bit HDC/HRR Engine & Infini-Attention Delta Bundler

**Phase:** Phase 3 — Hyperdimensional, Fractal & Graph Index  
**Sprint Goal:** Implement the 64-byte-aligned 10,240-bit `Hypervector` algebra and Infini-attention Delta-Rule bundler in `crates/icx-hdc`.  
**Estimated Duration:** 1 week

## Tickets

### T-31-001 — Implement 10,240-Bit `Hypervector` (`[u64; 160]`) & BLAKE3 XOF Seeding

**Description:** Implement `Hypervector::from_seed`, `bind` (`^`), `permute`, `hamming_distance`, `similarity`, and `sketch_u32` in `crates/icx-hdc/src/hv.rs`.

**Acceptance Criteria:**
- [x] Unrelated seeds produce quasi-orthogonal hypervectors with `similarity < 0.05`.
- [x] `A.bind(&B).bind(&B) == A` holds for all hypervectors.
- [x] `permute(k)` preserves Hamming distance between any pair of hypervectors.

**Dependencies:** T-11-002  
**Reference:** `docs/hyperdimensional-and-fractal-index.md` §2  
**Estimated Complexity:** M

---

### T-31-002 — Implement Infini-Attention Delta-Rule Majority Bundler (`HdcBundler`)

**Description:** Implement `HdcBundler` in `crates/icx-hdc/src/hv.rs` weighting incoming hypervectors by their novelty relative to the current memory readout (`arXiv:2404.07143`) so repeated boilerplate cannot saturate the bundle.

**Acceptance Criteria:**
- [x] Bundling 100 identical boilerplate vectors alongside 1 novel anomaly vector still allows the anomaly vector to be retrieved above the similarity threshold.

**Dependencies:** T-31-001  
**Reference:** `docs/hyperdimensional-and-fractal-index.md` §2.2; `ref-docs/papers/2404.07143-Infini-Attention.pdf`  
**Estimated Complexity:** M
