# Sprint 2 — Lossless Merkle-DAG (`LCM`) & Cache-Epoch Controller

**Phase:** Phase 1 — Foundations  
**Sprint Goal:** Implement the hierarchical `MerkleContextDag` (`L0` leaves -> `L1` turns/tools -> `L2` folded sub-trajectory branches -> `L3` sealed Cache Epochs) and `CacheEpochController`.  
**Estimated Duration:** 1 week

## Tickets

### T-12-001 — Implement `MerkleContextDag` Node Hierarchy & Cryptographic Root Computation

**Description:** Build `MerkleContextDag` in `crates/icx-core/src/dag.rs` supporting `LeafChunk`, `ToolArtifactNode`, `TurnNode`, `SubTrajectoryBranchNode`, and `CacheEpochNode`.

**Acceptance Criteria:**
- [x] Every parent node computes its `Blake3Digest` over the ordered digests of its children plus its metadata header.
- [x] Every `L1`, `L2`, and `L3` node retains lossless child pointers down to `L0` chunks.
- [x] Unit tests verify Merkle root determinism and child traversal.

**Dependencies:** T-11-004  
**Reference:** `docs/virtual-memory-and-dag-engine.md` §3; `ref-docs/papers/2605.04050-LCM-Lossless-Context-Management.pdf`  
**Estimated Complexity:** M

---

### T-12-002 — Implement Sub-Trajectory Branching & Context-Folding (`branch` / `fold`)

**Description:** Implement `begin_branch` and `fold_branch` on `MerkleContextDag` (`arXiv:2510.11967`) so multi-step debugging/compile loops collapse into a single `L2` `SubTrajectoryBranchNode` once resolved while retaining full `L1` history in CAS.

**Acceptance Criteria:**
- [x] Intermediate turns inside an active branch are absorbed into a single `⟦Φ:...⟧` commit node upon `fold_branch`.
- [x] Original intermediate turns remain 100% dereferenceable via `#icx:<branch_id>`.
- [x] Unit tests verify token reduction and lossless branch expansion.

**Dependencies:** T-12-001  
**Reference:** `docs/virtual-memory-and-dag-engine.md` §3; `ref-docs/papers/2510.11967-Context-Folding.pdf`  
**Estimated Complexity:** M

---

### T-12-003 — Implement `CacheEpochController` for Prefix-Cache-Locked Virtual Paging

**Description:** Implement `CacheEpochController` in `crates/icx-core/src/epoch.rs` partitioning context into Zone 0 (Pinned Head), Zone 1 (Sealed Cache Epochs `E_0..E_{k-1}`), Zone 2 (Active Sliding Window), and Zone 3 (Ephemeral Tail Slot).

**Acceptance Criteria:**
- [x] Sealed epochs in Zone 1 never mutate across subsequent turns (`assert_eq!` on serialized Zone 1 prefix bytes across 100 turns).
- [x] Rollover triggers deterministically when Zone 2 exceeds `epoch_token_threshold` while protecting the last `protect_last_n` turns verbatim.
- [x] Simulated prefix-cache hit rate across a 200-turn trajectory is `>= 94%`.

**Dependencies:** T-12-001, T-12-002  
**Reference:** `docs/virtual-memory-and-dag-engine.md` §4; `ref-docs/papers/2610.28577-Cache-Engineering-for-Agents.pdf`  
**Estimated Complexity:** L
