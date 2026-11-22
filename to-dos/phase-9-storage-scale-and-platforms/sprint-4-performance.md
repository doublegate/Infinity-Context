# Sprint 4 — Performance

**Phase:** Phase 9 — Storage, Scale and Platforms  
**Release:** v1.7.0  
**Sprint Goal:** Close the open performance qualification tickets, add runtime CPU dispatch and
bit-sliced majority counters behind exact-bit oracles, and remeasure and publish every affected
number.  
**Estimated Duration:** 1.5 weeks

## Research gate

Re-check before any ticket starts:

- **External docs and versions:** `std::arch` AVX-512 intrinsics stability and
  `is_x86_feature_detected!("avx512vpopcntdq")` in the current stable Rust; NEON popcount on
  aarch64; any crate considered for dispatch (Context7).
- **Installed agent behaviour:** not applicable beyond the hook latency budget recorded in Phase 8.
- **Project evidence:** the current state of T-120-024 and T-120-025 in
  `to-dos/codex-audit-remediation.md`; the HDC scan and bundling kernels in `icx-hdc`; the strict
  performance ceilings and the hosts they were measured on; whether the workspace
  `unsafe_code = "forbid"` rule permits the intrinsics path (it must stay forbidden).

### Research notes

_None recorded yet._

## Tickets

### T-94-001 — Close T-120-024 (release candidate qualification)

**Description:** Finish the remaining release-candidate qualification items: strict performance,
governance and exact-commit CI.

**Research:** Re-read the ticket's open items and confirm which still apply after Phases 6 to 9.

**Acceptance Criteria:**

- [ ] Every open item is closed with evidence or explicitly re-scoped with the owner.
- [ ] `to-dos/codex-audit-remediation.md` records the closure.

**Dependencies:** None  
**Reference:** `to-dos/codex-audit-remediation.md:53`, `:83`  
**Estimated Complexity:** M

---

### T-94-002 — Close T-120-025 (measured HDC kernel optimization)

**Description:** Complete the measured HDC kernel and codegen optimization with the exact-bit
oracle, a matched A/B proof and unchanged strict performance ceilings.

**Research:** Confirm the A/B method (same host, pinned clocks, repeated runs) and the oracle
coverage still required.

**Acceptance Criteria:**

- [ ] The exact-bit oracle passes for every kernel variant.
- [ ] The matched A/B result is measured and published in `docs/numbers.md` with its manifest.
- [ ] Strict performance ceilings still pass.

**Dependencies:** None  
**Reference:** `to-dos/codex-audit-remediation.md:54`, `:84`  
**Estimated Complexity:** M

---

### T-94-003 — Runtime CPU dispatch with AVX-512 VPOPCNTDQ

**Description:** Select the Hamming-distance kernel at run time: AVX-512 VPOPCNTDQ where
available, AVX2 or the portable kernel otherwise, and NEON on aarch64, without `unsafe` in the
workspace.

**Research:** Confirm whether safe dispatch is possible under `unsafe_code = "forbid"` (for
example via safe `std::arch` wrappers, a vetted dependency, or autovectorized code with
`target_feature` multiversioning). Hypothesis: VPOPCNTDQ lowers the 10k-node scan time on capable
hosts. Measurement: scan benchmark per kernel on a capable host. If neither safe dispatch nor a
gain is possible, close the ticket with the finding.

**Acceptance Criteria:**

- [ ] Every kernel matches the scalar reference bit for bit (oracle test on each CI platform).
- [ ] The workspace still has zero `unsafe` code.
- [ ] Scan time per kernel and host is measured and published in `docs/numbers.md`.

**Dependencies:** T-94-002  
**Reference:** `ref-docs/novel-methods/non-human-neural-codecs-and-holographic-context.md:81-82`  
**Estimated Complexity:** L

---

### T-94-004 — Bit-sliced majority counters

**Description:** Replace per-bit counters in hypervector bundling with bit-sliced counters, so
majority over many vectors uses word-wide operations.

**Research:** Hypothesis: bit-sliced counters cut bundling time for large bundles. Measurement:
bundling benchmark across bundle sizes. A rejected hypothesis closes the ticket.

**Acceptance Criteria:**

- [ ] Bundled output is bit-identical to the current implementation, including tie-breaking
  (property test).
- [ ] Bundling time across bundle sizes is measured and published in `docs/numbers.md`.

**Dependencies:** None  
**Reference:** plan Phase 9 Sprint 4; `docs/hyperdimensional-and-fractal-index.md`  
**Estimated Complexity:** M

---

### T-94-005 — Remeasure and publish the numbers

**Description:** Rerun `icx benchmark` and the storage, bus and kernel benchmarks on every
platform job and the reference host, and regenerate `docs/numbers.md`.

**Research:** Confirm host descriptions and that the generator (T-83-005) covers every new row.

**Acceptance Criteria:**

- [ ] All rows are regenerated from fresh runs with manifests.
- [ ] Documents that cite changed rows link to them; no stale copies remain.

**Dependencies:** T-94-001, T-94-002, T-94-003, T-94-004, T-83-005  
**Reference:** `docs/numbers.md`; plan Phase 9 Sprint 4  
**Estimated Complexity:** S

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Line coverage is `>= 95%` (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Affected docs and `CHANGELOG.md` `[Unreleased]` are updated.
- [ ] Upstream re-scan for Phase 10 Sprint 1 is done and its tickets are adjusted.
