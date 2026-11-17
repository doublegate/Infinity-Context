# Changelog

All notable changes to **Infinity-Context (`icx`)** are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html), and is distributed under the **GNU General Public License v3.0 or later (`GPL-3.0-or-later`)** ([`LICENSE`](LICENSE)).

## [Unreleased]

### Added

- **Automated Antigravity CLI (`agy`) Pull Request Review & CI/CD Hardening (`.github/`, `scripts/`):**
  - Self-hosted Antigravity PR review workflow ([`.github/workflows/antigravity-review.yml`](.github/workflows/antigravity-review.yml)), review rubric ([`.github/agy-review.md`](.github/agy-review.md)), deterministic parser and deduplicating GitHub comment publisher ([`scripts/agy-review.sh`](scripts/agy-review.sh)), and 16-case offline regression harness ([`scripts/agy-review-selftest.sh`](scripts/agy-review-selftest.sh)).
  - Automated Contributor License Agreement (`CLA.md`) verification workflow ([`.github/workflows/cla.yml`](.github/workflows/cla.yml)) and commit-author / co-author, PR-description, and PR-comment signature verifier with offline selftest suite ([`.github/scripts/cla-check.sh`](.github/scripts/cla-check.sh)).
  - Expanded CI pipeline ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) adding `cargo-llvm-cov` LCOV workspace coverage reporting with `codecov/codecov-action@v5` and checksum-pinned `gitleaks` `v8.30.1` secret scanning alongside `check` and `agy` self-tests.
  - Repository code ownership ([`.github/CODEOWNERS`](.github/CODEOWNERS)) and automated Cargo + GitHub Actions dependency updates ([`.github/dependabot.yml`](.github/dependabot.yml)).

### Changed

- **Five-Plane Patent-Pending Notice & README Badge Matrix ([`PATENTS.md`](PATENTS.md), [`README.md`](README.md)):**
  - Reconciled [`PATENTS.md`](PATENTS.md) to explicitly enumerate the five pending invention families across all five architectural planes of **Infinity-Context (`icx`)** (`AHTL` & Braille-256 symbolic codec, Virtual Context MMU & 4-tier `BLAKE3` Merkle-DAG with subtask context-folding, 10,240-bit `HDC` + Delta-Rule bundler + Contractive IFS `FCGA` archive, Titans surprise-gated bi-temporal `PPR` + 4-zone prefix-cache epoch controller, and lock-free `/dev/shm` seqlock telepathy ring bus + multi-harness hook fabric + polymorphic `icx` MCP pager).
  - Optimized the [`README.md`](README.md) hero badge block into four balanced, width-verified rows covering CI, REUSE 3.3, CLA, Codecov, Antigravity (`agy`) PR review, Gitleaks, licensing, 5 pending patent families, verified performance benchmarks, and supported agent harnesses.

## [0.1.0] - 2026-11-16

### Added

- **Licensing, Patent-Pending IP Protection & REUSE 3.3 Governance:**
  - Multi-tier IP protection framework ([`LICENSING.md`](LICENSING.md), [`PATENTS.md`](PATENTS.md), [`COMMERCIAL.md`](COMMERCIAL.md), [`CLA.md`](CLA.md), [`REUSE.toml`](REUSE.toml), [`LICENSES/`](LICENSES/)) verified at 100% REUSE 3.3 compliance (`uvx reuse lint`): `GPL-3.0-or-later` (`LICENSE` / `COPYING`) for core workspace code, `GPL-3.0-or-later AND AGPL-3.0-or-later` for networked proxy/MCP crates (`icx-proxy`, `icx-adapters`), `CC-BY-NC-ND-4.0 AND LicenseRef-InfinityContext-Commercial` for specifications, mathematical derivations, and artwork, plus patent-pending and commercial reservation in `PATENTS.md` and `COMMERCIAL.md`.
- **Research & Forensics Corpus (`ref-docs/`):**
  - Initial project concept (`ref-docs/initial-concept.md`) and 38-source Deep-Research Technical Report (`ref-docs/research-report.md`).
  - Downloaded 22 primary arXiv research PDFs into `ref-docs/papers/` with full mathematical synthesis (`ref-docs/papers/INDEX.md`).
  - Reverse-Engineer-Anything (`rea`) binary and SQLite forensic ledger covering Claude Code `2.1.326`, OpenAI Codex `0.169.0`, Google Antigravity CLI `agy`, Hermes Agent, and `ai-memory` (`ref-docs/agent-internals/rea-binary-and-sqlite-forensics.md`).
  - Cross-agent hook and wire interception matrix (`ref-docs/agent-internals/cross-agent-hook-and-wire-matrix.md`), industry prior-art comparison (`ref-docs/prior-art/github-and-industry-landscape.md`), and mathematical derivations of `AHTL`, 10,240-bit `HDC/HRR`, Contractive IFS `FCGA`, and Titans Surprise Gating (`ref-docs/novel-methods/non-human-neural-codecs-and-holographic-context.md`).
- **Implementation Specifications (`docs/`):**
  - 13 implementation-grade specification documents (`architecture.md`, `alien-holographic-codec.md`, `virtual-memory-and-dag-engine.md`, `hyperdimensional-and-fractal-index.md`, `agent-adapters-and-interception.md`, `wire-protocol-and-shm-bus.md`, `data-model.md`, `interfaces.md`, `performance.md`, `security-and-threat-model.md`, `testing-strategy.md`, `build-and-tooling.md`, `glossary.md`).
  - 4 Michael Nygard Architecture Decision Records in `docs/adr/` (`ADR-0001` through `ADR-0004`).
  - Normative Mermaid diagram sources in `docs/diagrams/` and dark/light vector SVGs in `docs/images/`.
- **Hierarchical Roadmap & Sprint Plans (`to-dos/`):**
  - 5-Phase, 14-Sprint hierarchical roadmap (`to-dos/ROADMAP.md`), Phase 0 precursory setup checklist (`to-dos/phase-0-precursory-setup/overview.md`), and ticket specifications `T-11-001` through `T-52-002`.
- **8-Crate Rust Workspace (`crates/`):**
  - `icx-core`: `IcxHandle` (`#icx:<16-hex>`), Gear-hash `FastCdcChunker`, `zstd`-compressed `BLAKE3` `CasStore`, 4-level `MerkleContextDag` (`LCM`), and `CacheEpochController`.
  - `icx-hdc`: 10,240-bit `Hypervector` (`[u64; 160]`), Infini-attention Delta-Rule `HdcBundler`, Contractive IFS `FractalCoord`, `TitansSurpriseGate`, and `HdcAssociativeIndex`.
  - `icx-codec`: `AhtlPacket` non-human BPE-aligned symbolic codec, Unicode Braille-256 bijection (`U+2800..U+28FF`), `StructuralSkeletonizer`, and pre-hash `SecretRedactor`.
  - `icx-store`: `BiTemporalGraph` (`[t_valid, t_invalid)` x `[t_created, t_expired)`) and HippoRAG `PersonalizedPageRank` (`PPR`).
  - `icx-bus`: Lock-free POSIX `/dev/shm/icx-fabric-v1.shm` `SeqlockTelemetryBus` (`512 x 2,048-byte` `#[repr(C, align(64))]` slots).
  - `icx-adapters`: `InfinityContextMmu`, `ClaudeCodeAdapter`, `CodexAdapter`, `AntigravityAdapter` (strict exit code `0`), `HermesAdapter`, and Single-Tool Polymorphic `McpPagerServer`.
  - `icx-proxy`: `WireProxyTransformer` for Anthropic 4-slot `cache_control: {"type": "ephemeral"}` and OpenAI 1,024-token prefix cache alignment.
  - `icx-cli`: `icx` binary supporting `hook`, `mcp-serve`, `proxy-transform`, `encode-ahtl`, `decode-ahtl`, `status`, `doctor`, and `benchmark`.
- **Branding, Icons & Relationship Graphs (`images/`, `assets/`):**
  - Graphviz relationship diagrams (`images/infinity-context-architecture.{dot,png,svg}` and `images/infinity-context-crate-graph.{dot,png,svg}`).
  - Multi-resolution icon packs (`assets/icons/`), 3:1 wide banners (`assets/branding/icx-banner-{1500x500,1200x400,900x300}.png`), full 16:9 hero banner (`images/InfinityContext_Full-Logo.png`), web manifest (`assets/web/`), Linux `.desktop` launcher (`assets/linux/`), and `1280x640` GitHub Social Preview card (`assets/branding/icx-social-preview-1280x640.{png,svg}`).
