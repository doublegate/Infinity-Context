# Sprint 1 — Workspace, FastCDC & BLAKE3 CAS

**Phase:** Phase 1 — Foundations  
**Sprint Goal:** Deliver the compiling 8-crate workspace skeleton and the deterministic FastCDC + BLAKE3 Content-Addressed Store (`crates/icx-core`).  
**Estimated Duration:** 1 week

## Tickets

### T-11-001 — Initialize 8-Crate Rust Workspace, Lints & CI Workflow

**Description:** Create root `Cargo.toml`, `rust-toolchain.toml`, `.gitignore`, `.editorconfig`, `.github/workflows/ci.yml`, and the 8 workspace crates (`icx-core`, `icx-hdc`, `icx-codec`, `icx-store`, `icx-bus`, `icx-adapters`, `icx-proxy`, `icx-cli`).

**Acceptance Criteria:**
- [x] `cargo check --workspace --all-targets` succeeds with zero warnings.
- [x] Workspace lints enforce `unsafe_code = "forbid"` (except `icx-bus` where `mmap` is encapsulated).
- [x] Documentation updated in `docs/build-and-tooling.md`.

**Dependencies:** none  
**Reference:** `docs/build-and-tooling.md` §1–3  
**Estimated Complexity:** M

---

### T-11-002 — Implement `ArtifactId`, `Blake3Digest` & Core Domain Identifiers

**Description:** Define the 64-bit short handle `ArtifactId` (`#icx:<16-hex>`), full 256-bit `Blake3Digest`, `AgentKind`, and typed `IcxError` hierarchy in `crates/icx-core`.

**Acceptance Criteria:**
- [x] `ArtifactId::from_digest` deterministically extracts the big-endian leading 64 bits of a `Blake3Digest`.
- [x] `Display` and `FromStr` round-trip `#icx:0123456789abcdef` cleanly.
- [x] Unit tests added covering parsing, formatting, and malformed handle rejection.

**Dependencies:** T-11-001  
**Reference:** `docs/data-model.md` §1  
**Estimated Complexity:** S

---

### T-11-003 — Implement Gear-Hash Content-Defined Chunker (`FastCDC`)

**Description:** Implement deterministic rolling gear-hash Content-Defined Chunking (`min=2048`, `avg=8192`, `max=65536` bytes) in `crates/icx-core/src/chunker.rs` so shared prefixes/suffixes across large tool outputs deduplicate automatically.

**Acceptance Criteria:**
- [x] Chunk boundaries depend exclusively on byte content and respect `min`/`max` bounds.
- [x] Concatenating emitted chunks reproduces the exact input byte slice.
- [x] Unit tests verify mid-file insertion stability (chunks after a localized edit realign).

**Dependencies:** T-11-002  
**Reference:** `docs/virtual-memory-and-dag-engine.md` §2.1  
**Estimated Complexity:** M

---

### T-11-004 — Implement BLAKE3 Content-Addressed Store (`CasStore`) with Integrity Verification

**Description:** Implement `CasStore` in `crates/icx-core/src/cas.rs` supporting `put_bytes`, `deref_bytes`, `deref_lines`, and `grep_cold`, with mandatory BLAKE3 digest verification on every read.

**Acceptance Criteria:**
- [x] `put_bytes` chunks payloads, hashes each chunk and the composite artifact with `BLAKE3`, and indexes line byte offsets.
- [x] `deref_bytes` verifies `blake3::hash(&reconstructed) == full_digest`, returning `IcxError::IntegrityViolation` if any byte is mutated.
- [x] `deref_lines(id, start, end)` returns exact 1-based inclusive line slices.
- [x] `grep_cold` finds substring/pattern matches across stored artifacts with exact line numbers.

**Dependencies:** T-11-003  
**Reference:** `docs/virtual-memory-and-dag-engine.md` §2.2; `docs/interfaces.md` §1  
**Estimated Complexity:** M
