# Infinity-Context (`icx`) — Project Roadmap

This is the planning entry point for Infinity-Context (`icx`). Each phase links to its overview,
its implementation plan and its sprint files. Every ticket has a stable ID, which branches, commits
and pull requests cite for the ticket's whole life.

## Status

- **Current release:** [v1.5.1](https://github.com/doublegate/Infinity-Context/releases/tag/v1.5.1)
  (2026-11-25): the redactor follow-ups
  ([#104](https://github.com/doublegate/Infinity-Context/issues/104)) to v1.5.0, which closed
  Phase 7. Durable state lives in [PROJECT-STATUS.md](../PROJECT-STATUS.md), and user-visible
  change in [CHANGELOG.md](../CHANGELOG.md).
- **Next minor release:** v1.6.0,
  [Phase 8](#phase-8--v160-prompt-cache-proxy-and-observability).
- **Tracking:** one GitHub milestone per release, with one issue per sprint
  ([#40](https://github.com/doublegate/Infinity-Context/issues/40) to
  [#81](https://github.com/doublegate/Infinity-Context/issues/81)). Issue
  [#32](https://github.com/doublegate/Infinity-Context/issues/32) (`icx-shm`) is in the v2.0.0
  milestone.
- **Release train:** Phase 6 to Phase 11 ship v1.4.0 to v1.9.0, one minor release per phase.
  Phase 12 ships v2.0.0.
- **History:** Phases 0 to 5 built v0.1.0 to v1.0.0. Their checked boxes are historical records,
  not current validation. The v1.1.0 to v1.3.2 remediation and fix releases were tracked in
  [codex-audit-remediation.md](codex-audit-remediation.md), the CHANGELOG and GitHub issues.

## Planning conventions

These rules apply to every phase from Phase 6 on.

- **Ticket IDs:**
  - Phases 6 to 9 use `T-<phase><sprint>-<nnn>`, as Phases 0 to 5 did (for example `T-61-001`).
  - Phase 10 onward uses `T-<phase>-<sprint>-<nnn>` (for example `T-10-1-001`), so that
    `T-101` cannot mean two things.
  - Housekeeping tickets use `T-H-<nnn>`.
- **Sources:** every ticket names its source: an issue, a document path and line, an audit
  finding ID, a URL or a paper. Nothing in these plans is unsourced.
- **Claims:** acceptance criteria contain no unverified performance numbers. A target reads
  "measured and published in `docs/numbers.md`". Figures that `docs/numbers.md` disowns are not
  reused.
- **Research continues during the work, not only during planning:**
  - Each sprint file opens with a research gate, and each ticket carries a Research line.
  - Before a ticket starts, re-check every external fact it depends on: agent hook
    documentation, the MCP specification, provider APIs, crate versions. Also re-run the
    project's own evidence. Record what changed in the sprint's dated "Research notes".
  - Agent-facing tickets confirm behaviour on the locally installed agent version first, with REA
    where the documentation is silent.
  - A research-backed ticket states its hypothesis and the measurement that settles it. A
    rejected hypothesis closes the ticket with the finding instead of a feature.
  - At each sprint's close, re-scan upstream changes and adjust the next sprint's tickets.
- **Additive by default:** new capability lands off by default, so the default build is unchanged
  until a feature proves out. Breaking format or policy changes wait for v2.0.0.

## Release train

### Phase 6 — v1.4.0: Agent reach and output shrinking

Goal: reach more agents, and shrink output where icx cannot today. This includes:

- an investigation of the installed agents with REA and Ghidra, for integration points the
  documented hooks do not expose;
- Codex output shrinking;
- Gemini CLI, Cursor and OpenCode adapters;
- deeper Claude Code integration, including the plan-mode permission behaviour of the icx tool;
- live contract fixtures for every agent;
- `icx install`.

- [Overview](phase-6-agent-reach-and-output-shrinking/overview.md) ·
  [Implementation plan](phase-6-agent-reach-and-output-shrinking/phase_6_implementation_plan.md) ·
  [v1.4.0 milestone](https://github.com/doublegate/Infinity-Context/milestone/1)
  - [Sprint 1 — Agent Internals Investigation (REA/Ghidra)](phase-6-agent-reach-and-output-shrinking/sprint-1-agent-internals-investigation.md) (7 tickets) · [#40](https://github.com/doublegate/Infinity-Context/issues/40)
  - [Sprint 2 — Codex Output Shrinking](phase-6-agent-reach-and-output-shrinking/sprint-2-codex-output-shrinking.md) (6 tickets) · [#41](https://github.com/doublegate/Infinity-Context/issues/41)
  - [Sprint 3 — New Adapters: Gemini CLI, Cursor and OpenCode](phase-6-agent-reach-and-output-shrinking/sprint-3-new-adapters.md) (4 tickets) · [#42](https://github.com/doublegate/Infinity-Context/issues/42)
  - [Sprint 4 — Claude Code Depth](phase-6-agent-reach-and-output-shrinking/sprint-4-claude-code-depth.md) (8 tickets) · [#43](https://github.com/doublegate/Infinity-Context/issues/43)
  - [Sprint 5 — Install and Verification Tooling](phase-6-agent-reach-and-output-shrinking/sprint-5-install-and-verification-tooling.md) (9 tickets) · [#44](https://github.com/doublegate/Infinity-Context/issues/44)

### Phase 7 — v1.5.0: Retrieval and compaction quality

Goal: better search and compaction. This includes:

- BM25 and trigram ranking fused with HDC, and optional local embeddings;
- an evaluation set drawn from real sessions;
- a resume packet injected around compaction, and agent-directed eviction;
- `icx rescan`, and a structural redactor.

- [Overview](phase-7-retrieval-and-compaction-quality/overview.md) ·
  [Implementation plan](phase-7-retrieval-and-compaction-quality/phase_7_implementation_plan.md) ·
  [v1.5.0 milestone](https://github.com/doublegate/Infinity-Context/milestone/2)
  - [Sprint 1 — Ranking](phase-7-retrieval-and-compaction-quality/sprint-1-ranking.md) (4 tickets) · [#45](https://github.com/doublegate/Infinity-Context/issues/45)
  - [Sprint 2 — Evaluation](phase-7-retrieval-and-compaction-quality/sprint-2-evaluation.md) (4 tickets) · [#46](https://github.com/doublegate/Infinity-Context/issues/46)
  - [Sprint 3 — Compaction](phase-7-retrieval-and-compaction-quality/sprint-3-compaction.md) (4 tickets) · [#47](https://github.com/doublegate/Infinity-Context/issues/47)
  - [Sprint 4 — Redaction](phase-7-retrieval-and-compaction-quality/sprint-4-redaction.md) (4 tickets) · [#48](https://github.com/doublegate/Infinity-Context/issues/48)

### Phase 8 — v1.6.0: Prompt-cache proxy and observability

Goal: proxy support for current provider caching, with the providers' own cache counters as the
only basis for a cache-hit claim. Also:

- an offline fake-provider harness;
- per-hook traces and `icx stats`;
- alignment with MCP 2026-07-28.

- [Overview](phase-8-cache-proxy-and-observability/overview.md) ·
  [Implementation plan](phase-8-cache-proxy-and-observability/phase_8_implementation_plan.md) ·
  [v1.6.0 milestone](https://github.com/doublegate/Infinity-Context/milestone/3)
  - [Sprint 1 — Provider Caching](phase-8-cache-proxy-and-observability/sprint-1-provider-caching.md) (4 tickets) · [#49](https://github.com/doublegate/Infinity-Context/issues/49)
  - [Sprint 2 — Offline Test Harness](phase-8-cache-proxy-and-observability/sprint-2-offline-test-harness.md) (3 tickets) · [#50](https://github.com/doublegate/Infinity-Context/issues/50)
  - [Sprint 3 — Observability](phase-8-cache-proxy-and-observability/sprint-3-observability.md) (5 tickets) · [#51](https://github.com/doublegate/Infinity-Context/issues/51)
  - [Sprint 4 — MCP 2026-07-28 Alignment](phase-8-cache-proxy-and-observability/sprint-4-mcp-2026-07-28-alignment.md) (6 tickets) · [#52](https://github.com/doublegate/Infinity-Context/issues/52)

### Phase 9 — v1.7.0: Storage, scale and platforms

Goal: storage, compatibility and platforms. This includes:

- incremental saves and zstd in the content store, a trigram index for `grep`, less lock
  contention, and `icx rescan --apply` to re-redact a stored workspace (#102);
- downgrade and export paths;
- aarch64 Linux, macOS and Windows builds;
- the open strict benchmarks.

- [Overview](phase-9-storage-scale-and-platforms/overview.md) ·
  [Implementation plan](phase-9-storage-scale-and-platforms/phase_9_implementation_plan.md) ·
  [v1.7.0 milestone](https://github.com/doublegate/Infinity-Context/milestone/4)
  - [Sprint 1 — Storage](phase-9-storage-scale-and-platforms/sprint-1-storage.md) (5 tickets) · [#53](https://github.com/doublegate/Infinity-Context/issues/53)
  - [Sprint 2 — Compatibility](phase-9-storage-scale-and-platforms/sprint-2-compatibility.md) (3 tickets) · [#54](https://github.com/doublegate/Infinity-Context/issues/54)
  - [Sprint 3 — Platforms](phase-9-storage-scale-and-platforms/sprint-3-platforms.md) (6 tickets) · [#55](https://github.com/doublegate/Infinity-Context/issues/55)
  - [Sprint 4 — Performance](phase-9-storage-scale-and-platforms/sprint-4-performance.md) (5 tickets) · [#56](https://github.com/doublegate/Infinity-Context/issues/56)

### Phase 10 — v1.8.0: Prevent and prove

Goal: prevent bulky output before it reaches the agent, and prove the savings. This includes:

- a command rewriter that runs before execution, a redirect before oversized reads, and log
  templating;
- a savings ledger;
- a paired benchmark with a noise-aware release gate;
- code health and the remaining security residuals.

- [Overview](phase-10-prevent-and-prove/overview.md) ·
  [Implementation plan](phase-10-prevent-and-prove/phase_10_implementation_plan.md) ·
  [v1.8.0 milestone](https://github.com/doublegate/Infinity-Context/milestone/5)
  - [Sprint 1 — Pre-Execution Command Rewriter](phase-10-prevent-and-prove/sprint-1-pre-execution-command-rewriter.md) (4 tickets) · [#57](https://github.com/doublegate/Infinity-Context/issues/57)
  - [Sprint 2 — Pre-Tool Redirect](phase-10-prevent-and-prove/sprint-2-pre-tool-redirect.md) (2 tickets) · [#58](https://github.com/doublegate/Infinity-Context/issues/58)
  - [Sprint 3 — Log and Test-Output Templating](phase-10-prevent-and-prove/sprint-3-log-and-test-output-templating.md) (4 tickets) · [#59](https://github.com/doublegate/Infinity-Context/issues/59)
  - [Sprint 4 — Savings Ledger and Analytics](phase-10-prevent-and-prove/sprint-4-savings-ledger-and-analytics.md) (3 tickets) · [#60](https://github.com/doublegate/Infinity-Context/issues/60)
  - [Sprint 5 — Paired Benchmark Harness](phase-10-prevent-and-prove/sprint-5-paired-benchmark-harness.md) (6 tickets) · [#61](https://github.com/doublegate/Infinity-Context/issues/61)
  - [Sprint 6 — Code Health](phase-10-prevent-and-prove/sprint-6-code-health.md) (11 tickets) · [#62](https://github.com/doublegate/Infinity-Context/issues/62)
  - [Sprint 7 — Security Residuals](phase-10-prevent-and-prove/sprint-7-security-residuals.md) (6 tickets) · [#63](https://github.com/doublegate/Infinity-Context/issues/63)

### Phase 11 — v1.9.0: Govern and distribute

Goal: governance, distribution and polish. This includes:

- privacy tags and a per-project policy file;
- a timeline query and a repository map;
- a local viewer;
- packaging for plugins, the MCP Registry and package managers;
- clap, a config file and the daemon API;
- library and supply-chain work, the documentation site, and Markdown export.

- [Overview](phase-11-govern-and-distribute/overview.md) ·
  [Implementation plan](phase-11-govern-and-distribute/phase_11_implementation_plan.md) ·
  [v1.9.0 milestone](https://github.com/doublegate/Infinity-Context/milestone/6)
  - [Sprint 1 — Privacy and Policy](phase-11-govern-and-distribute/sprint-1-privacy-and-policy.md) (5 tickets) · [#64](https://github.com/doublegate/Infinity-Context/issues/64)
  - [Sprint 2 — Timeline and Repo Map](phase-11-govern-and-distribute/sprint-2-timeline-and-repo-map.md) (3 tickets) · [#65](https://github.com/doublegate/Infinity-Context/issues/65)
  - [Sprint 3 — Local Viewer](phase-11-govern-and-distribute/sprint-3-local-viewer.md) (3 tickets) · [#66](https://github.com/doublegate/Infinity-Context/issues/66)
  - [Sprint 4 — Packaging and Distribution](phase-11-govern-and-distribute/sprint-4-packaging-and-distribution.md) (5 tickets) · [#67](https://github.com/doublegate/Infinity-Context/issues/67)
  - [Sprint 5 — Standards and Approvals](phase-11-govern-and-distribute/sprint-5-standards-and-approvals.md) (3 tickets) · [#68](https://github.com/doublegate/Infinity-Context/issues/68)
  - [Sprint 6 — CLI, Config and Daemon API](phase-11-govern-and-distribute/sprint-6-cli-config-and-daemon-api.md) (6 tickets) · [#69](https://github.com/doublegate/Infinity-Context/issues/69)
  - [Sprint 7 — Library and Supply Chain](phase-11-govern-and-distribute/sprint-7-library-and-supply-chain.md) (7 tickets) · [#70](https://github.com/doublegate/Infinity-Context/issues/70)
  - [Sprint 8 — Documentation Site](phase-11-govern-and-distribute/sprint-8-documentation-site.md) (8 tickets) · [#71](https://github.com/doublegate/Infinity-Context/issues/71)
  - [Sprint 9 — Markdown Export and Import](phase-11-govern-and-distribute/sprint-9-markdown-export-and-import.md) (4 tickets) · [#72](https://github.com/doublegate/Infinity-Context/issues/72)

### Phase 12 — v2.0.0: Architecture release

Goal: the architecture changes that need a major release. This includes:

- the opt-in memory-mapped seqlock bus `icx-shm`, built in full, which resolves issue #32 and
  ADR-0007;
- L1 to L4 memory tiers, and an optional neural tier;
- a format break: 256-bit keys, a MAC on stubs, an encrypted raw tier;
- conditional team sync.

- [Overview](phase-12-architecture-v2/overview.md) ·
  [Implementation plan](phase-12-architecture-v2/phase_12_implementation_plan.md) ·
  [v2.0.0 milestone](https://github.com/doublegate/Infinity-Context/milestone/7)
  - [Sprint 1 — `icx-shm` Design Acceptance and Scaffolding](phase-12-architecture-v2/sprint-1-shm-design-and-scaffolding.md) (11 tickets) · [#73](https://github.com/doublegate/Infinity-Context/issues/73)
  - [Sprint 2 — `icx-shm` Core Protocol](phase-12-architecture-v2/sprint-2-shm-core-protocol.md) (8 tickets) · [#74](https://github.com/doublegate/Infinity-Context/issues/74)
  - [Sprint 3 — `icx-shm` Qualification](phase-12-architecture-v2/sprint-3-shm-qualification.md) (8 tickets) · [#75](https://github.com/doublegate/Infinity-Context/issues/75)
  - [Sprint 4 — `icx-shm` Platforms and Integration](phase-12-architecture-v2/sprint-4-shm-platforms-and-integration.md) (4 tickets) · [#76](https://github.com/doublegate/Infinity-Context/issues/76)
  - [Sprint 5 — `icx-shm` Measurement, Documentation, Policy and Resolution](phase-12-architecture-v2/sprint-5-shm-measurement-policy-and-resolution.md) (8 tickets) · [#77](https://github.com/doublegate/Infinity-Context/issues/77)
  - [Sprint 6 — Memory Tiers](phase-12-architecture-v2/sprint-6-memory-tiers.md) (4 tickets) · [#78](https://github.com/doublegate/Infinity-Context/issues/78)
  - [Sprint 7 — Neural Tier (Optional, Off by Default)](phase-12-architecture-v2/sprint-7-neural-tier.md) (4 tickets) · [#79](https://github.com/doublegate/Infinity-Context/issues/79)
  - [Sprint 8 — Format Break](phase-12-architecture-v2/sprint-8-format-break.md) (6 tickets) · [#80](https://github.com/doublegate/Infinity-Context/issues/80)
  - [Sprint 9 — Sync (Conditional) and v2.0.0 Release](phase-12-architecture-v2/sprint-9-sync-conditional.md) (4 tickets) · [#81](https://github.com/doublegate/Infinity-Context/issues/81)

## Cross-cutting tracks

- **Agent internals:** Phase 6 Sprint 1 extends `ref-docs/agent-internals/` with REA and Ghidra
  findings, as new dated files because that corpus is immutable. It is scoped to interoperability
  with the owner's own installs, and each vendor's terms are checked before anything ships.
- **Issue #32 (`icx-shm`):** built in Phase 12. Three earlier phases supply prerequisites:
  - per-hook timing of bus publish and poll (Phase 8);
  - an arm64 CI runner and the macOS build (Phase 9);
  - the daemon JSON-RPC API, used as the socket-routing comparison (Phase 11).
- **Governance:** the patent, CLA and commercial terms (ICX-CODEX-074) stay with counsel. No
  ticket here changes them.

## Housekeeping (any release)

- **T-H-001:** relabel the stale "v1.2.0 candidate" headers
  (`docs/agent-adapters-and-interception.md:5`, `docs/numbers.md:1`).
- **T-H-002:** propose removing the disowned figures from the project summary in `CLAUDE.md`
  (sub-50 µs recall, ≥94% cache hit as a provider metric). That block is managed, so the change is
  proposed rather than made directly.
- **T-H-003:** decide what to do about large binaries in git history (`docs/claude/12-v1.1.0-remediation-status.md`, lines 44 to 46). Decision only.
- **T-H-004:** build the specification PDF in CI.

## History: Phases 0 to 5 (v0.1.0 to v1.0.0, completed)

- **Phase 0, pre-cursory setup, research and REA forensics (v0.1.0):**
  [overview](phase-0-precursory-setup/overview.md)
- **Phase 1, foundations: CAS, Merkle-DAG, bus and zero-`unsafe` enforcement (v0.2.0):**
  [overview](phase-1-foundations/overview.md) ·
  [plan](phase-1-foundations/phase_1_implementation_plan.md)
- **Phase 2, AHTL codec and structural skeletonizers (v0.3.0):**
  [overview](phase-2-alien-codec-and-compression/overview.md) ·
  [plan](phase-2-alien-codec-and-compression/phase_2_implementation_plan.md)
- **Phase 3, HDC, fractal archive and bi-temporal graph (v0.4.0):**
  [overview](phase-3-hyperdimensional-and-fractal-index/overview.md) ·
  [plan](phase-3-hyperdimensional-and-fractal-index/phase_3_implementation_plan.md)
- **Phase 4, cross-agent interception, MCP pager and wire proxy (v0.5.0):**
  [overview](phase-4-cross-agent-interception/overview.md) ·
  [plan](phase-4-cross-agent-interception/phase_4_implementation_plan.md)
- **Phase 5, verification, benchmarks and packaging (v1.0.0):**
  [overview](phase-5-verification-benchmarks-and-release/overview.md) ·
  [plan](phase-5-verification-benchmarks-and-release/phase_5_implementation_plan.md)

## Cross-phase dependencies

- **Phases 6 and 7:** Phase 7's compaction work uses the Phase 6 hook events (PreCompact and
  PostCompact) and the live contract fixtures.
- **Phases 8, 10 and 11:** Phase 8's `icx stats` storage is shared with the Phase 10 savings
  ledger and the Phase 11 viewer.
- **Phase 10 benchmark:** it gates every later phase, using its noise-aware rule.
- **Phases 11 and 12:** Phase 11's Markdown export and policy file underlie the conditional sync
  in Phase 12.
- **Phase 12 bus:** see the issue #32 prerequisites under cross-cutting tracks.
