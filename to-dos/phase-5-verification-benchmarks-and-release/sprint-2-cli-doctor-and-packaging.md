# Sprint 2 — CLI Tooling, `icx doctor`, `icx benchmark` & Release Packaging

**Phase:** Phase 5 — Verification, Benchmarks & Release  
**Sprint Goal:** Deliver the complete `icx` binary (`crates/icx-cli`) with `init`, `hook`, `mcp-serve`, `proxy`, `encode-ahtl`, `decode-ahtl`, `deref`, `status`, `doctor`, and `benchmark` subcommands, plus repository documentation (`README.md`, `AGENTS.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `SECURITY.md`).  
**Estimated Duration:** 1 week

## Tickets

### T-52-001 — Implement `icx` CLI Subcommands (`crates/icx-cli`)

**Description:** Implement all `icx` subcommands (`init`, `hook`, `mcp-serve`, `proxy`, `encode-ahtl`, `decode-ahtl`, `deref`, `status`, `doctor`, `benchmark`) and CLI integration tests.

**Acceptance Criteria:**

- [x] `icx benchmark` executes CAS, AHTL, HDC, Fractal, PPR, Seqlock, and Prefix-Cache benchmarks and prints a verified SLO scorecard.
- [x] `icx doctor` inspects local agent installations (`claude`, `codex`, `agy`, `hermes`) and verifies hook/MCP readiness.

**Dependencies:** T-51-001, T-51-002  
**Reference:** `docs/interfaces.md` §5  
**Estimated Complexity:** M

---

### T-52-002 — Assemble Root Project Documentation, `AGENTS.md` & Quality Gates

**Description:** Populate `README.md`, `AGENTS.md` (with `CLAUDE.md` and `GEMINI.md` symlinks), `CHANGELOG.md`, `CONTRIBUTING.md`, `SECURITY.md`, `LICENSE` (GNU General Public License v3.0), and verify all quality gates (`cargo fmt`, `cargo clippy`, `cargo test`, `cargo doc`).

**Acceptance Criteria:**

- [x] `cargo fmt --all -- --check` passes.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [x] `cargo test --workspace` passes 100% of unit and integration tests.
- [x] `cargo doc --workspace --no-deps` builds with zero warnings.

**Dependencies:** T-52-001  
**Reference:** `docs/build-and-tooling.md` §2  
**Estimated Complexity:** M
