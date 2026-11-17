# Sprint 3 — Bi-Temporal CSR Knowledge Graph & Personalized PageRank (`PPR`) (COMPLETED)

**Phase:** Phase 3 — Hyperdimensional, Fractal & Graph Index  
**Sprint Goal:** Implement `BiTemporalGraph` (`arXiv:2501.13956`), `CsrGraphSnapshot`, and HippoRAG `PersonalizedPageRank` (`arXiv:2405.14831`) in `crates/icx-store`.  
**Status:** COMPLETED (`v0.4.0`)

## Tickets

### T-33-001 — Implement `BiTemporalGraph` with Automatic Functional Edge Invalidation (COMPLETED)

**Description:** Implement `BiTemporalGraph::upsert_edge` in `crates/icx-store/src/graph.rs` tracking `[t_valid, t_invalid)` and `[t_tx_start, t_tx_end)` intervals and invalidating superseded functional edges automatically.

**Acceptance Criteria:**
- [x] Upserting `(fn:foo, test_status, PASS)` at `t=200` automatically marks a prior `(fn:foo, test_status, FAIL)` edge from `t=100` with `t_invalid = Some(200)` and `t_tx_end = Some(200)`, and re-upserting with the same target also supersedes the prior edge so at most one active edge exists per functional `(source_entity, relation)` pair.
- [x] Time-travel queries at `t=150` return `FAIL` while current queries return `PASS`.
- [x] Supports `query_edges_from`, `query_edges_to`, `query_relation`, `invalidate_edge`, and atomic directory persistence (`save_to_dir` / `load_from_dir`).

**Dependencies:** T-11-004  
**Reference:** `docs/hyperdimensional-and-fractal-index.md` §5.1; `ref-docs/papers/2501.13956-Zep-Temporal-Knowledge-Graph.pdf`  
**Estimated Complexity:** M

---

### T-33-002 — Implement HippoRAG Personalized PageRank (`PPR`) Multi-Hop Retrieval (COMPLETED)

**Description:** Implement power-iteration `PersonalizedPageRank` over currently valid edges in `BiTemporalGraph` via a contiguous `CsrGraphSnapshot` (`row_ptr`, `col_idx`, `weights`, `inv_out_weight`, `prov_row_ptr`, `prov_ids`) to retrieve multi-hop transitively connected `ArtifactId` provenance handles.

**Acceptance Criteria:**
- [x] `BiTemporalGraph::build_csr(at_ms)` compiles active edges into a contiguous Compressed Sparse Row (`CSR`) snapshot and runs $O(V + E)$ power iteration with scalar dangling-node redistribution.
- [x] A query seeded on `module_a` retrieves artifacts linked to `module_c` across a 2-hop chain `module_a -> trait_b -> module_c` while ignoring invalidated historical edges.

**Dependencies:** T-33-001  
**Reference:** `docs/hyperdimensional-and-fractal-index.md` §5.2; `ref-docs/papers/2405.14831-HippoRAG.pdf`  
**Estimated Complexity:** M
