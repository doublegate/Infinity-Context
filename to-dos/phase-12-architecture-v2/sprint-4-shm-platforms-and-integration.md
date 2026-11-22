# Sprint 4 — `icx-shm` Platforms and Integration

**Phase:** Phase 12 — Architecture Release  
**Release:** v2.0.0  
**Sprint Goal:** Add the macOS `shm_open` backend, detect containers and sandboxes where
`/dev/shm` is missing, small or hidden and fall back with a logged reason, report the active
backend in `icx doctor`, and measure ADR-0007's daemon-socket alternative over the v1.9 JSON-RPC
API side by side.  
**Estimated Duration:** 2 weeks

## Research gate

Before any ticket starts:

- Re-check macOS `shm_open` semantics (name length limit, no `ftruncate` after first sizing,
  `fstat` fields, no `/proc`) and the Phase 9 macOS build and runner.
- Re-check container defaults for `/dev/shm` (Docker 64 MB, Podman, Kubernetes `emptyDir`
  medium `Memory`) and each agent's sandbox (Claude Code, Codex, `agy`) on the installed versions.
- Re-read the v1.9 JSON-RPC event-routing methods (T-11-6-005, T-11-6-006) and the Phase 8
  bus-timing traces.

### Research notes

_None recorded yet._

## Tickets

### T-12-4-001 — macOS backend via `shm_open`

**Description:** Implement the shm backend on macOS with `shm_open`, the same hardening and
header validation, and a macOS liveness check for dead-writer recovery (no `/proc`).

**Research:** Confirm the macOS process start-time API (`proc_pidinfo` or `sysctl`) and whether
it needs additional `unsafe`, which must stay inside `icx-shm`.

**Acceptance Criteria:**

- [ ] Sprint 3 suites pass on the macOS runner (bus suite, stress harness, fuzz target).
- [ ] Owner-only permissions and refusal cases tested on macOS.

**Dependencies:** T-12-3-007, T-93-002  
**Reference:** Plan Phase 12 Sprint 4; ADR-0007 §Consequences ("macOS needs `shm_open`")  
**Estimated Complexity:** L

---

### T-12-4-002 — Container and sandbox detection with logged fallback

**Description:** Detect environments where `/dev/shm` is missing, too small for the mapping, or
not shared with sibling agents (sandboxes with private mounts), and fall back to the file bus,
logging the reason.

**Research:** Hypothesis: comparing the mount ID and device of `/dev/shm` with sibling agents'
view (via the bus file header) detects private mounts. Confirming measurement: tests in Docker
with default and enlarged `/dev/shm`, and in each agent sandbox available locally. Rejection
falls back whenever visibility cannot be proven.

**Acceptance Criteria:**

- [ ] Each detected case logs a distinct reason and uses the file bus (tested in containers).
- [ ] Two processes that cannot see the same mapping never both select shm (tested).

**Dependencies:** T-12-2-003  
**Reference:** ADR-0007 §Consequences ("containers default to a 64 MB `/dev/shm`, sandboxed agents
may not see it")  
**Estimated Complexity:** M

---

### T-12-4-003 — `icx doctor` reports the active backend and why

**Description:** `icx doctor` (and `--json`) reports the compiled-in backends, the configured
`bus.backend`, the active backend, and the reason for the choice or fallback.

**Research:** Confirm the v1.9 `--json` schema can be extended without a breaking change.

**Acceptance Criteria:**

- [ ] Every fallback reason from T-12-2-002, T-12-2-003 and T-12-4-002 is reported (tested).
- [ ] `--json` schema version bumped and documented.

**Dependencies:** T-12-4-002, T-11-6-003  
**Reference:** Plan Phase 12 Sprint 4  
**Estimated Complexity:** S

---

### T-12-4-004 — Socket-routing comparison over the v1.9 JSON-RPC API

**Description:** Measure ADR-0007's daemon-socket alternative (events routed through the
resident daemon over the v1.9 JSON-RPC API) side by side with the shm backend and the file bus,
on the same harness and hardware, and publish all three. This is evidence, not a replacement: the
shm backend is built either way.

**Research:** Hypothesis: the shm backend gives lower cross-process publish-to-visible latency
than both the socket route and the file bus. Confirming measurement: latency distributions from
the shared harness on x86_64 and arm64. A rejected hypothesis is published as measured; it does
not cancel the shm backend.

**Acceptance Criteria:**

- [ ] All three routes measured and published in `docs/numbers.md`, with methodology and
  hardware.
- [ ] Result linked from issue #32 and ADR-0007.

**Dependencies:** T-12-3-002, T-11-6-006  
**Reference:** Issue #32; ADR-0007 (Alternatives considered); `docs/numbers.md`  
**Estimated Complexity:** M

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes, with and without
  `--features shm-bus`.
- [ ] `cargo test --workspace` passes, with and without `--features shm-bus`.
- [ ] Coverage >= 95% (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Docs and `CHANGELOG.md` (`[Unreleased]`) updated in the same change.
- [ ] Upstream re-scan for Sprint 5 done, its tickets adjusted, and findings recorded in its
  Research notes.
