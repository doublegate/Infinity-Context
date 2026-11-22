# Phase 12 — Architecture Release (v2.0.0)

## Goal

Ship the v2.0.0 architecture release. Sprints 1–5 build `icx-shm`, the opt-in memory-mapped
seqlock bus designed in ADR-0007, in full, and resolve
[issue #32](https://github.com/doublegate/Infinity-Context/issues/32): the feature is developed,
qualified, measured, documented and released, not only evaluated. It lands in a major release
because it changes the published unsafe-code policy from "no `unsafe` anywhere" to "`unsafe`
confined to the opt-in `icx-shm` crate". It stays **off by default**: with the feature off,
builds, tests, benchmarks and binaries are identical to v1.9.

Sprints 6–9 add L1–L4 memory tiers with page-in on recall fault, an optional neural tier, a
format break (256-bit keys, keyed stub MAC, optional encrypted raw tier, ADR-0009 migration), and
a conditional sync decision.

## Changes to the v2.0 Plan from the v1.8 and v1.9 Research

- No v2.0 change may cause a significant regression on the Phase 10 paired benchmark, judged by
  that sprint's noise-aware rule, not a single run.
- Team sync builds on the v1.9 Markdown export and the policy file, rather than a new protocol.
- The neural tier competes against the v1.5 BM25+RRF baseline on both the evaluation set and the
  paired benchmark.
- The format break also versions the policy file and the savings ledger.

Ticket IDs for Phase 10 onward use `T-<phase>-<sprint>-<nnn>` (for example `T-12-3-004`).

## Exit Criterion

- [ ] `icx-shm` ships behind the `shm-bus` feature, passes the Sprint 3 qualification gate on
  x86_64 and on the arm64 runner, and falls back cleanly to the file bus wherever shm is
  unavailable.
- [ ] With the feature off, builds, tests, benchmarks and binaries are identical to v1.9
  (verified).
- [ ] `icx-shm` is the only crate that allows `unsafe`, every block carries `// SAFETY:`, and CI
  fails if any other crate drops the workspace lints.
- [ ] Cross-process latency against the file bus, and the daemon-socket comparison, are measured
  and published in `docs/numbers.md`.
- [ ] ADR-0007 is marked implemented, the policy wording is updated everywhere, and issue #32 is
  closed with links to the PRs and the release.
- [ ] L1–L4 tiers with page-in on recall fault ship; the neural tier ships only if it beats the
  BM25+RRF baseline, otherwise its finding is recorded.
- [ ] ADR-0009 documents the format break, and the one-way migration tool is tested on real v1.9
  stores.
- [ ] The sync decision is recorded, and sync ships only if a privacy ADR approves it.
- [ ] No significant regression on the Phase 10 paired benchmark.

## Sprints

- [Sprint 1 — `icx-shm` Design Acceptance and Scaffolding](sprint-1-shm-design-and-scaffolding.md)
- [Sprint 2 — `icx-shm` Core Protocol](sprint-2-shm-core-protocol.md)
- [Sprint 3 — `icx-shm` Qualification](sprint-3-shm-qualification.md)
- [Sprint 4 — `icx-shm` Platforms and Integration](sprint-4-shm-platforms-and-integration.md)
- [Sprint 5 — `icx-shm` Measurement, Documentation, Policy and Resolution](sprint-5-shm-measurement-policy-and-resolution.md)
- [Sprint 6 — Memory Tiers](sprint-6-memory-tiers.md)
- [Sprint 7 — Neural Tier](sprint-7-neural-tier.md)
- [Sprint 8 — Format Break](sprint-8-format-break.md)
- [Sprint 9 — Sync (Conditional) and v2.0.0 Release](sprint-9-sync-conditional.md)

## Dependencies on Earlier Phases

The `icx-shm` prerequisites are tickets in earlier phases:

- **Phase 8 (v1.6.0), Sprint 3 — Observability:** per-hook traces that time bus publish and poll,
  the baseline the gain is measured against.
- **Phase 9 (v1.7.0), Sprint 3 — Platforms:** the `ubuntu-24.04-arm` CI runner (weak-memory
  hardware for the stress runs) and the macOS build (`shm_open` backend).
- **Phase 11 (v1.9.0), Sprint 6 — Daemon API:** `T-11-6-005` and `T-11-6-006`, the JSON-RPC
  socket-routing alternative to compare against; and `T-11-6-002`, the config file that carries
  `bus.backend`.

Other dependencies:

- **Phase 7 (v1.5.0), Sprints 1–2:** the BM25+RRF baseline and the retrieval evaluation set that
  gate the neural tier.
- **Phase 10 (v1.8.0), Sprint 4:** the savings ledger the format break versions.
- **Phase 10 (v1.8.0), Sprint 5:** the paired benchmark and its noise-aware rule.
- **Phase 11 (v1.9.0), Sprints 1 and 9:** the policy file and the Markdown export that sync
  builds on.
- **v1.3.2:** `proc_info` in `crates/icx-adapters/src/workspace.rs`, reused for dead-writer
  liveness.

## Standing Sources to Watch

- `memmap2`, `loom`, `libc`/`rustix` and `nix` releases (Context7 for APIs).
- Rust RFC 3301 (`AtomicPerByte`, atomic memcpy) and the Rust memory-model and unsafe-code
  guidelines discussions.
- Linux `seqcount_t` and `Documentation/memory-barriers.txt` changes; crossbeam's `SeqLock`.
- Miri and ThreadSanitizer support notes (what can and cannot be checked under mmap).
- GitHub Actions arm64 runner availability and image changes (`ubuntu-24.04-arm`) and macOS
  runner images.
- Docker and container-runtime defaults for `/dev/shm` size and visibility; agent sandbox
  documentation (Claude Code, Codex, `agy`).
- ONNX Runtime and Candle releases, and RVQ/ICAE compression papers.
- Agent changelogs, the MCP specification changelog, provider caching docs, and comparable
  memory tools.

## Reference Docs

- [docs/adr/0007-opt-in-icx-shm-bus.md](../../docs/adr/0007-opt-in-icx-shm-bus.md)
- [docs/adr/0004-zero-copy-shm-telepathy-bus.md](../../docs/adr/0004-zero-copy-shm-telepathy-bus.md)
- [docs/wire-protocol-and-shm-bus.md](../../docs/wire-protocol-and-shm-bus.md)
- [crates/icx-bus/src/ring.rs](../../crates/icx-bus/src/ring.rs)
- [Phase 12 implementation plan](phase_12_implementation_plan.md)
