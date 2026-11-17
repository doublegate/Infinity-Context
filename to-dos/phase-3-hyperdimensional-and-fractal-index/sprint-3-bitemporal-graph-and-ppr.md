# Sprint 3 — Bi-Temporal CSR Knowledge Graph & Personalized PageRank (`PPR`)

**Phase:** Phase 3 — Hyperdimensional, Fractal & Graph Index  
**Sprint Goal:** Implement `BiTemporalGraph` (`arXiv:2501.13956`) and HippoRAG `PersonalizedPageRank` (`arXiv:2405.14831`) in `crates/icx-store`.  
**Estimated Duration:** 1 week

## Tickets

### T-33-001 — Implement `BiTemporalGraph` with Automatic Functional Edge Invalidation

**Description:** Implement `BiTemporalGraph::upsert_edge` in `crates/icx-store/src/graph.rs` tracking `[t_valid, t_invalid)` and `[t_tx_start, t_tx_end)` intervals and invalidating superseded functional edges automatically.

**Acceptance Criteria:**
- [x] Upserting `(fn:foo, test_status, PASS)` at `t=200` automatically marks a prior `(fn:foo, test_status, FAIL)` edge from `t=100` with `t_invalid = Some(200)`.
- [x] Time-travel queries at `t=150` return `FAIL` while current queries return `PASS`.

**Dependencies:** T-11-004  
**Reference:** `docs/hyperdimensional-and-fractal-index.md` §5.1; `ref-docs/papers/2501.13956-Zep-Temporal-Knowledge-Graph.pdf`  
**Estimated Complexity:** M

---

### T-33-002 — Implement HippoRAG Personalized PageRank (`PPR`) Multi-Hop Retrieval

**Description:** Implement power-iteration `PersonalizedPageRank` over currently valid edges in `BiTemporalGraph` to retrieve multi-hop transitively connected `ArtifactId` provenance handles.

**Acceptance Criteria:**
- [x] A query seeded on `module_a` retrieves artifacts linked to `module_c` across a 2-hop chain `module_a -> trait_b -> module_c` while ignoring invalidated historical edges.

**Dependencies:** T-33-001  
**Reference:** `docs/hyperdimensional-and-fractal-index.md` §5.2; `ref-docs/papers/2405.14831-HippoRAG.pdf`  
**Estimated Complexity:** M
