# Phase 9 (v1.7.0) Implementation Plan — Storage, Scale and Platforms

## Goal Description

Deliver v1.7.0 in four sprint pull requests. Storage changes go first because the compatibility
work must cover the formats they introduce, and platform builds must exist before the final
performance remeasurement so it can include aarch64.

## Ordered Approach

1. **Sprint 9.1 — Storage (T-91-001 to T-91-004).**
   - Measure save time and lock wait on a field-sized workspace first, then add incremental saves
     of derived state, zstd compression in the CAS, the line/trigram index for `grep_cold`, and
     lock-contention reduction for parallel agents.
2. **Sprint 9.2 — Compatibility (T-92-001 to T-92-003).**
   - `icx export --format <v>` and documented downgrade paths, then detection of a newer snapshot
     format by an already-running MCP server, which tells the user to reconnect.
3. **Sprint 9.3 — Platforms (T-93-001 to T-93-006).**
   - aarch64 Linux first (closest to the current code), then macOS with a `shm_open` / file-bus
     fallback, then Windows; then release-matrix and `doctor` updates and the off-Linux agent
     registry and daemon socket; the `ubuntu-24.04-arm` runner lands with the aarch64 build.
4. **Sprint 9.4 — Performance (T-94-001 to T-94-005).**
   - Close T-120-024 and T-120-025, add runtime CPU dispatch with AVX-512 VPOPCNTDQ and bit-sliced
     majority counters behind exact-bit oracles, then remeasure and publish.

## Risks

- **Format changes break older readers.** Mitigation: every new on-disk format bumps its magic,
  fails closed in older binaries, and has an export path (T-92-001).
- **Compression or indexing cost exceeds its saving.** Mitigation: each is measured on the field
  corpus before it becomes the default; a negative result closes the ticket with the finding.
- **Platform differences in shared memory and sockets.** Mitigation: a file-bus fallback, explicit
  `doctor` checks, and per-platform CI rather than cross-compilation alone.
- **SIMD kernels diverging from the scalar path.** Mitigation: exact-bit oracle tests on every
  dispatch target, and runtime dispatch falls back to the scalar kernel.
- **CI cost and flakiness on new runners.** Mitigation: path filters and a required-check set that
  keeps doc-only changes green.

## Verification

- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` and `cargo llvm-cov ... --fail-under-lines 95` on every sprint PR and on
  every platform job where the tool is supported.
- Round-trip tests: export to each supported older format, read with that release's reader, and
  compare.
- Exact-bit oracle tests for every SIMD kernel against the scalar reference.
- Every changed figure measured and published in `docs/numbers.md` through the generated table.
