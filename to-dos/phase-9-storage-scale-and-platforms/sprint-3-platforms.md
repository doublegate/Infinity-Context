# Sprint 3 — Platforms

**Phase:** Phase 9 — Storage, Scale and Platforms  
**Release:** v1.7.0  
**Sprint Goal:** Build, test and release `icx` on aarch64 Linux, macOS and Windows, with platform
equivalents for the bus, agent registry and daemon socket, and stand up the `ubuntu-24.04-arm`
runner needed for the v2.0 `icx-shm` stress runs.  
**Estimated Duration:** 2 weeks

## Research gate

Re-check before any ticket starts:

- **External docs and versions:** GitHub-hosted runner availability and labels
  (`ubuntu-24.04-arm`, current `macos-*` and `windows-*` images); `rustix` support for `shm_open`
  and Unix sockets on macOS; Windows named pipes or AF_UNIX support and the crate to use
  (Context7); Rust target tiers.
- **Installed agent behaviour:** which agents run on macOS and Windows and where they keep hook and
  MCP configuration there, so `icx install` and `doctor` look in the right places.
- **Project evidence:** Linux-only code paths (`/dev/shm`, `0o600` modes, Unix socket daemon,
  agent registry paths); the release workflow matrix; issue #32's requirements for weak-memory
  stress runs.

### Research notes

_None recorded yet._

## Tickets

### T-93-001 — aarch64 Linux build

**Description:** Build, test and release `aarch64-unknown-linux-gnu` binaries.

**Research:** Confirm which dependencies have architecture-specific code and whether the seqlock
relies on x86 memory ordering anywhere.

**Acceptance Criteria:**

- [ ] `cargo test --workspace` passes natively on aarch64 Linux in CI.
- [ ] Release artifacts for aarch64 Linux are built and checksummed by the release workflow.

**Dependencies:** T-93-006  
**Reference:** plan Phase 9 Sprint 3  
**Estimated Complexity:** M

---

### T-93-002 — macOS build with `shm_open` / file-bus fallback

**Description:** Build and test on macOS (aarch64 and x86_64). The bus uses POSIX `shm_open` where
it works and a file-backed bus fallback otherwise.

**Research:** Confirm `shm_open` size and naming limits on macOS. Hypothesis: `shm_open` gives
publish/poll latency close to Linux `/dev/shm`. Measurement: the Phase 8 bus benchmark on macOS
with both backends.

**Acceptance Criteria:**

- [ ] The full test suite passes on macOS in CI.
- [ ] Both bus backends pass the bus conformance tests.
- [ ] Bus latency per backend is measured and published in `docs/numbers.md`.

**Dependencies:** T-83-002  
**Reference:** plan Phase 9 Sprint 3; `docs/wire-protocol-and-shm-bus.md`  
**Estimated Complexity:** L

---

### T-93-003 — Windows build

**Description:** Build and test on `x86_64-pc-windows-msvc`, with the file-bus fallback, Windows
file permissions in place of `0o600`, and a named-pipe or AF_UNIX daemon transport.

**Research:** Confirm how to restrict file access to the current user on Windows and which daemon
transport the chosen crates support.

**Acceptance Criteria:**

- [ ] The full test suite passes on Windows in CI.
- [ ] Workspace files are readable only by the current user (test).
- [ ] The threat model documents the Windows differences.

**Dependencies:** T-93-002  
**Reference:** plan Phase 9 Sprint 3; `docs/security-and-threat-model.md`  
**Estimated Complexity:** XL

---

### T-93-004 — Release matrix and `doctor` updates

**Description:** Add every new target to the release workflow (archives, checksums) and teach
`icx doctor` the platform-specific checks (bus backend, permissions, daemon transport).

**Research:** Review the current release workflow and `doctor` checks for Linux-only assumptions.

**Acceptance Criteria:**

- [ ] The release workflow produces checksummed artifacts for all targets and smoke-tests each.
- [ ] `icx doctor` reports the platform, bus backend and daemon transport, and passes on each CI
  platform.
- [ ] The README platform matrix is updated.

**Dependencies:** T-93-001, T-93-002, T-93-003  
**Reference:** plan Phase 9 Sprint 3; `docs/build-and-tooling.md`  
**Estimated Complexity:** M

---

### T-93-005 — Agent registry and daemon socket equivalents off Linux

**Description:** Provide the agent registry and daemon socket on macOS and Windows at
platform-appropriate paths, with the same access rules as on Linux.

**Research:** Confirm per-platform runtime directories and where each installed agent keeps its
configuration on macOS and Windows.

**Acceptance Criteria:**

- [ ] Registry and daemon tests pass on all three platforms.
- [ ] Paths and permissions per platform are documented in the deployment guide.

**Dependencies:** T-93-002, T-93-003  
**Reference:** plan Phase 9 Sprint 3  
**Estimated Complexity:** M

---

### T-93-006 — `ubuntu-24.04-arm` CI runner

**Description:** Add a CI job on the GitHub-hosted `ubuntu-24.04-arm` runner. Besides the aarch64
build, it is a prerequisite for the v2.0 `icx-shm` weak-memory stress runs (#32), which need a
weakly ordered architecture.

**Research:** Confirm runner availability for this repository and its limits; read #32 for the
stress-run requirements this runner must meet.

**Acceptance Criteria:**

- [ ] A required CI job runs build and tests on `ubuntu-24.04-arm`.
- [ ] A placeholder stress job is wired and documented for #32.
- [ ] #32 is updated with the runner details.

**Dependencies:** None  
**Reference:** issue #32; plan Phase 9 Sprint 3  
**Estimated Complexity:** S

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes on every platform job.
- [ ] Line coverage is `>= 95%` (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Affected docs and `CHANGELOG.md` `[Unreleased]` are updated.
- [ ] Upstream re-scan for Sprint 4 is done and its tickets are adjusted.
