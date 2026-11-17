# Sprint 2 — AST, Test-Log & Diff Structural Skeletonizers

**Phase:** Phase 2 — Alien Holographic Token Lattice (`AHTL`)  
**Sprint Goal:** Implement single-pass structural skeletonizers for source code, build/test outputs, and unified diffs in `crates/icx-codec/src/skeleton.rs`.  
**Estimated Duration:** 1 week

## Tickets

### T-22-001 — Implement Source Code & Unified Diff Structural Skeletonizer

**Description:** Extract function/method signatures (`fn`, `pub fn`, `def`, `func`, `class`, `struct`, `trait`, `impl`), line numbers, and diff hunks (`⊕`/`⊖`) while eliding low-surprise function bodies into `#icx:<hash>` slice pointers.

**Acceptance Criteria:**
- [x] Preserves 100% of public/private function names and type declarations.
- [x] Achieves `>= 10x` reduction on multi-hundred-line source files.

**Dependencies:** T-21-002  
**Reference:** `docs/alien-holographic-codec.md` §4  
**Estimated Complexity:** M

---

### T-22-002 — Implement Compiler & Test-Suite Log Skeletonizer (`cargo test`, `pytest`, `tsc`)

**Description:** Collapse passing test lines (`test ... ok`) into aggregate `✓pass=N` counters while extracting 100% of compiler errors (`error[E...]`), panics (`panicked at`), and failure stack traces into `!` anomaly clauses.

**Acceptance Criteria:**
- [x] Compresses a 500-test passing + 1-test failing `cargo test` output by `>= 25x` while keeping the exact panic file, line, column, and message verbatim.

**Dependencies:** T-22-001  
**Reference:** `docs/alien-holographic-codec.md` §4  
**Estimated Complexity:** M
