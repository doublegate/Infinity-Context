# Sprint 4 — Toolchain Modernization, 100% Zero-`unsafe` Conversion & `>= 95%` Codecov Enforcement

**Phase:** Phase 1 — Foundations  
**Sprint Goal:** Keep `std` across the workspace while eliminating 100% of `unsafe` code (`unsafe_code = "forbid"` enforced permanently across all 8 crates through `v1.0.0`), upgrade the Rust toolchain (`edition = "2024"`, MSRV `1.100`), workspace dependencies, and GitHub Actions (PR #2), and raise workspace Codecov line coverage to `>= 95%` across all crates with an automated CI gate.  
**Estimated Duration:** 1 week

## Tickets

### T-14-001 — Upgrade Rust Toolchain (`2024` Edition, MSRV `1.100`), Cargo Dependencies & GitHub Actions (PR #2)

**Description:** Incorporate and expand Dependabot PR #2 to bump all GitHub Actions (`actions/checkout@v7`, `codecov/codecov-action@v7`), upgrade workspace Rust edition to `2024` and MSRV (`rust-version`) to `1.100`, and bump all Cargo workspace crate constraints (`blake3 = "1.8.7"`, `serde = "1.0.229"`, `serde_json = "1.0.154"`, `thiserror = "2.0.21"`).

**Acceptance Criteria:**
- [x] `Cargo.toml` sets `edition = "2024"` and `rust-version = "1.100"`.
- [x] All GitHub Actions workflows and Cargo workspace dependencies are updated to current compatible releases and PR #2 is merged into `main`.

**Dependencies:** T-13-002  
**Reference:** `Cargo.toml`; `.github/workflows/ci.yml`  
**Estimated Complexity:** S

---

### T-14-002 — Refactor `SeqlockTelemetryBus` (`crates/icx-bus`) to 100% Safe Atomic Word-Packed Slots & Safe `/dev/shm` Positional I/O

**Description:** Eliminate all 11 `unsafe` blocks, raw pointer casts, `UnsafeCell`, `read_volatile`/`write_volatile`, and the `memmap2` dependency in `crates/icx-bus/src/ring.rs`. Represent `FabricHeader` (64 B) and `SeqlockSlot` (4,096 B) using 100% safe `core::sync::atomic::{AtomicU32, AtomicU64}` word arrays (`[AtomicU64; 496]` for `ahtl_words`, `[AtomicU64; 4]` for `hdc_sketch`, `[AtomicU64; 4]` for `fractal_q15_words`, `AtomicU64` for `packed_meta`) and implement cross-process `/dev/shm` persistence using safe positional file I/O (`std::os::unix::fs::FileExt::{read_exact_at, write_all_at}` on Unix with a `Mutex<File>` fallback on non-Unix) and OS-managed advisory file locking (`File::try_lock`).

**Acceptance Criteria:**
- [x] `size_of::<FabricHeader>() == 64`, `align_of::<FabricHeader>() == 64`, `size_of::<SeqlockSlot>() == 4096`, and `align_of::<SeqlockSlot>() == 64` are verified at compile time with zero `unsafe`.
- [x] `memmap2` is removed from `Cargo.toml` and `crates/icx-bus/Cargo.toml`.
- [x] Concurrent 8-writer × 8-reader × 10,000-message stress test and multi-instance file-backed `/dev/shm` bus test pass with zero torn reads (`s1 == s2`).

**Dependencies:** T-14-001  
**Reference:** `docs/wire-protocol-and-shm-bus.md` §2.1–2.2; `docs/security-and-threat-model.md`  
**Estimated Complexity:** L

---

### T-14-003 — Permanent Workspace-Wide `unsafe_code = "forbid"` Enforcement Through `v1.0.0`

**Description:** Set `[lints] workspace = true` in `crates/icx-bus/Cargo.toml` so all 8 crates inherit `unsafe_code = "forbid"`, and add a dedicated `Zero-unsafe policy gate` step to `.github/workflows/ci.yml` that fails CI if any `Cargo.toml` or `.rs` file weakens `unsafe_code = "forbid"` or introduces an `unsafe` block, function, trait, or impl anywhere in the repository.

**Acceptance Criteria:**
- [x] All 8 crates in `crates/*/Cargo.toml` set `[lints] workspace = true` and inherit `unsafe_code = "forbid"`.
- [x] `.github/workflows/ci.yml` enforces the zero-`unsafe` policy gate on every push and pull request.
- [x] `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `.github/copilot-instructions.md`, and `docs/` record the permanent rule (`std` allowed, zero `unsafe` code through `v1.0.0` and beyond).

**Dependencies:** T-14-002  
**Reference:** `Cargo.toml`; `.github/workflows/ci.yml`; `docs/build-and-tooling.md`  
**Estimated Complexity:** M

---

### T-14-004 — Expand Workspace Unit & Integration Tests to `>= 95%` Codecov & Sync Documentation

**Description:** Expand unit and integration test coverage across all 8 workspace crates (`icx-core`, `icx-hdc`, `icx-codec`, `icx-store`, `icx-bus`, `icx-adapters`, `icx-proxy`, `icx-cli`), raise the CI `cargo-llvm-cov` gate in `.github/workflows/ci.yml` and `codecov.yml` to `>= 95%` line coverage, and update `README.md`, `CHANGELOG.md`, `.gitignore`, and all `docs/` files prior to closing out Phase 1.

**Acceptance Criteria:**
- [x] `cargo llvm-cov --workspace --all-targets --lcov --output-path lcov.info --fail-under-lines 95` passes with `>= 95%` workspace and per-crate line coverage.
- [x] `README.md`, `CHANGELOG.md`, `.gitignore`, `codecov.yml`, and `docs/` reflect all Phase 1 Sprint 1–4 deliverables and invariants.

**Dependencies:** T-14-003  
**Reference:** `docs/testing-strategy.md`; `.github/workflows/ci.yml`; `codecov.yml`  
**Estimated Complexity:** L
