# Sprint 2.2: Multi-Language AST & Log Structural Skeletonizers

**Phase:** [Phase 2 — Alien Holographic Token Lattice (`AHTL`) & Structural Skeletonizers](overview.md)  
**Target Crate:** `crates/icx-codec`  
**Specifications:** [docs/alien-holographic-codec.md](../../docs/alien-holographic-codec.md) §4, [docs/ahtl/05-structural-ast-skeletonization-and-redaction.md](../../docs/ahtl/05-structural-ast-skeletonization-and-redaction.md)

---

## Tickets

### `T-22-001` — Source Code & Unified Diff Structural Skeletonizer
- **File:** `crates/icx-codec/src/skeleton.rs`
- **Tasks:**
  - [x] Extract top-level function, struct, enum, trait, class, type, and interface signatures across Rust, Python, TypeScript/JS, Go, and C/C++ (`extract_symbol_signature`), folding contiguous imports (`imports:L<a>-<b>(N)`), extracting module headers (`mod@L<n>`), and computing block line spans (`L<start>-<end>`).
  - [x] Extract `PASSKEY`, `INVARIANT`, `SAFETY:`, `TODO:`, and `FIXME:` annotations as verified pins (`✓pin@L<n>`).
  - [x] Implement unified `git diff` skeletonization (`skeletonize_diff`) summarizing modified file paths (`file:<path>`), hunk line ranges (`hunk:<coords>`), `+added/-removed` counts, and added/removed symbol signatures (`⊕sym` / `⊖sym`).
- **Acceptance Criteria:**
  - [x] `cargo test -p icx-codec skeleton` passes across source, diff, and plain-text inputs.

### `T-22-002` — Compiler & Test-Suite Log Skeletonizer (`12x–40x` Compression)
- **File:** `crates/icx-codec/src/skeleton.rs`
- **Tasks:**
  - [x] Detect test/build logs (`cargo test`, `Compiling`, `error[E`, `panicked at`, `pytest`, `tsc`/`vitest`, `go test`) automatically.
  - [x] Collapse passing tests (`test ... ok`, `PASSED`, `✓`, `--- PASS:`) into a single `✓pass:<count>` clause while pinning 100% of `FAILED` tests (`!FAIL@L<n>`), compiler errors, assertion messages, and panic stack locations (`!ERR@L<n>`).
- **Acceptance Criteria:**
  - [x] `compresses_large_cargo_test_log_by_over_25x_while_preserving_panic` and `compresses_pytest_tsc_vitest_and_go_test_logs_with_100_percent_failure_recall` achieve `>= 15x–25x` compression while preserving 100% of failure diagnostics.
