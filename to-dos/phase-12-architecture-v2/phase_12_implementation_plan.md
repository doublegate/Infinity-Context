# Phase 12 (v2.0.0) Implementation Plan — Architecture Release

## Goal Description

Deliver v2.0.0 in nine sprints. The first five build and resolve `icx-shm` (issue #32,
ADR-0007), the opt-in memory-mapped seqlock bus; the last four add memory tiers, an optional
neural tier, a deliberate format break and a conditional sync decision. The major version is
required because the published unsafe-code policy changes and because the storage format breaks
one way.

## Ordered Approach

1. **Sprint 1 — Design acceptance and scaffolding.** Run the research gate (memmap2, seqlock
   memory-model literature, RFC 3301, existing shm crates), accept ADR-0007 or supersede it,
   write the layout spec, refactor `ring.rs` behind a `BusBackend` trait with no behaviour
   change, create `crates/icx-shm` with its own lints table, add the CI lint gate, and add the
   `shm-bus` feature and `bus.backend` config key.
2. **Sprint 2 — Core protocol.** Hardened mapping, `posix_fallocate` pre-allocation, CAS ticket
   slot claims, atomic-word seqlock write and read with Release/Acquire ordering per Boehm 2012,
   dead-writer recovery via PID plus start time, and loss reporting at parity with the file bus.
3. **Sprint 3 — Qualification (release gate).** `loom` models, the multi-process stress harness
   on x86_64 and the arm64 runner (not QEMU), TSan and Miri within their scope, a fuzz target,
   the full bus suite against the shm backend, and the feature-off identity check.
4. **Sprint 4 — Platforms and integration.** macOS `shm_open`, container and sandbox detection
   with logged fallback, `icx doctor` backend reporting, and the daemon-socket comparison over the
   v1.9 JSON-RPC API.
5. **Sprint 5 — Measurement, documentation, policy and resolution.** Publish measurements, update
   the policy wording (README Security, `AGENTS.md` as a proposed diff only, `CLAUDE.md` project
   block, CI gate), rewrite the README bus note beside the patent summary, write release and
   upgrade notes, mark ADR-0007 implemented, close issue #32 with links, and record the decision
   on `auto` preferring shm.
6. **Sprint 6 — Memory tiers.** L1–L4 tiering with automatic page-in on a recall fault.
7. **Sprint 7 — Neural tier.** Optional, off by default, and gated on beating the BM25+RRF
   baseline on the evaluation set and the paired benchmark.
8. **Sprint 8 — Format break.** ADR-0009, 256-bit internal keys, a keyed stub MAC, an optional
   encrypted raw tier, versioned policy file and savings ledger, and a one-way migration tool.
9. **Sprint 9 — Sync (conditional) and release.** A privacy ADR decides; otherwise the sprint
   records the decision not to build. Then the v2.0.0 release ceremony.

## Risks

- **Torn reads from a wrong ordering.** A single missing fence produces torn reads that tests
  rarely catch. Mitigation: every data word is an atomic, the orderings follow Boehm 2012 and
  are modelled in `loom`, and the multi-process stress harness with checksummed payloads runs on
  weak-memory arm64 hardware, not QEMU user mode.
- **SIGBUS on a full tmpfs.** A write to a sparse page of a full `/dev/shm` delivers SIGBUS
  rather than an error. Mitigation: reserve the whole mapping with `posix_fallocate` at creation
  and fall back to the file bus when the reservation fails.
- **Policy creep.** `unsafe` could spread beyond `icx-shm`. Mitigation: `icx-shm` has its own
  lints table, and a CI script fails if any other crate drops `[lints] workspace = true` or the
  `forbid`.
- **Default-build drift.** The refactor or feature plumbing could change default behaviour.
  Mitigation: the `BusBackend` refactor is behaviour-preserving with the existing suite as the
  oracle, and the feature-off identity check compares artifacts with v1.9.
- **Dead writers.** A writer killed mid-write leaves a slot odd forever. Mitigation: recovery past
  a deadline after checking PID plus start time through `/proc`, tested with SIGKILL mid-write.
- **Environments without shm.** Containers default to a 64 MB `/dev/shm`, sandboxes may hide it,
  and macOS needs `shm_open`. Mitigation: detection with a logged reason and a first-class file
  bus fallback.
- **Unmeasured claims.** ADR-0007 anticipates sub-microsecond visibility; that is a hypothesis.
  Mitigation: no figure is published before it is measured, with methodology and hardware.
- **Neural tier cost without gain.** Mitigation: the tier ships only if it beats the BM25+RRF
  baseline; otherwise the finding is recorded and the ticket closes.
- **Irreversible migration.** The format break is one way. Mitigation: ADR-0009, a backup step
  in the migration tool, dry-run by default, and tests on real v1.9 stores.

## Verification

- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, and `cargo llvm-cov` at `--fail-under-lines 95` on every sprint PR,
  with and without `--features shm-bus`.
- Sprint 3 qualification gate: `loom`, multi-process stress on x86_64 and arm64, TSan, Miri
  within scope, fuzzing, and the bus suite against the shm backend.
- Feature-off identity: default-feature artifacts and test results match v1.9.
- The Phase 10 paired benchmark shows no significant regression, judged by its noise-aware rule.
- Every figure is measured and published in `docs/numbers.md` with methodology and hardware.
- Issue #32 closed with PR and release links; ADR-0007 status `implemented`.
