# Sprint 1 — 10,240-Bit HDC/HRR Engine & Infini-Attention Delta Bundler (COMPLETED)

**Phase:** Phase 3 — Hyperdimensional, Fractal & Graph Index  
**Sprint Goal:** Implement the 64-byte-aligned 10,240-bit `Hypervector` algebra, Infini-attention Delta-Rule bundler, and two-stage spectral/Bloom associative index in `crates/icx-hdc`.  
**Estimated Duration:** 1 week

## Tickets

### T-31-001 — Implement 10,240-Bit `Hypervector` (`[u64; 160]`) & BLAKE3 XOF Seeding (COMPLETED)

**Description:** Implement `Hypervector::from_seed`, `from_seed_framed`, `from_u64_seed`, `bind` (`^`), `unbind`, `bind_role_filler`, `permute`, `inverse_permute`, `encode_sequence`, `popcount`, `hamming_distance_simd`, `similarity`, `cosine_bipolar`, `sub_hypervector_64`, and `sketch_u32` in `crates/icx-hdc/src/hv.rs`.

**Acceptance Criteria:**

- [x] Unrelated seeds produce quasi-orthogonal hypervectors with `similarity < 0.05` and `|cosine_bipolar| < 0.05`.
- [x] `A.bind(&B).bind(&B) == A` (`(A ^ B) ^ B == A`) and `A.permute(k).inverse_permute(k) == A` hold for all hypervectors and shift indices.
- [x] `permute(k)` preserves Hamming distance between any pair of hypervectors, and `encode_sequence` is non-commutative.

**Dependencies:** T-11-002  
**Reference:** `docs/hyperdimensional-and-fractal-index.md` §2, `docs/ahtl/04-hyperdimensional-10240-bit-lattice.md`  
**Estimated Complexity:** M

---

### T-31-002 — Implement Infini-Attention Delta-Rule Majority Bundler (`HdcBundler`) & Two-Stage Associative Index (COMPLETED)

**Description:** Implement `HdcBundler` in `crates/icx-hdc/src/hv.rs` weighting incoming hypervectors by their novelty relative to the current memory readout (`arXiv:2404.07143`) with seen-digest deduplication and coordinate-parity tie-breaking so repeated boilerplate cannot saturate the bundle, and implement `AssociativeMemoryIndex` with two-stage 16-byte L2-cache `EntrySummary` pre-screening in `crates/icx-hdc/src/lib.rs`.

**Acceptance Criteria:**

- [x] Bundling 100 identical boilerplate vectors alongside 1 novel anomaly vector via `HdcBundler::add` (in either order) allows the anomaly vector to be retrieved above the similarity threshold (`> 0.25`).
- [x] `HdcAssociativeIndex` implements `AssociativeMemoryIndex` and executes two-stage spectral/Bloom + 10,240-bit POPCNT associative recall.

**Dependencies:** T-31-001  
**Reference:** `docs/hyperdimensional-and-fractal-index.md` §2.2; <https://arxiv.org/abs/2404.07143> (PDF not redistributed: arXiv non-exclusive license)  
**Estimated Complexity:** M
