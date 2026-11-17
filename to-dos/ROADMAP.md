# Infinity-Context (`icx`) — Project Roadmap

This is the master planning entry point for **Infinity-Context (`icx`)**. Each phase below links to its phase overview and constituent sprint files. Every ticket has a portable ID (`T-<Phase><Sprint>-<NNN>`) referenced in commits, tests, and documentation.

## Status

- **Current Phase:** Phase 0 Complete (Pre-Cursory Setup, Deep Research, REA Forensics, `ai-memory` Onboarding & 8-Crate Workspace Bootstrap All Green) — **Ready to start Phase 1 Full Production Expansion**
- **Active Workspace:** 8 Rust crates (`icx-core`, `icx-hdc`, `icx-codec`, `icx-store`, `icx-bus`, `icx-adapters`, `icx-proxy`, `icx-cli`)

---

## Phases Overview

### Phase 0 — Pre-Cursory Setup, Deep Research, REA Forensics & Workspace Bootstrap (COMPLETED)
**Goal:** Conduct 38-source deep research, download 22 arXiv PDFs (`ref-docs/papers/`), reverse-engineer local agent binaries & SQLite databases with REA (`ref-docs/agent-internals/`), derive full technical specifications (`docs/`), configure `ai-memory` (`default/doublegate-infinity-context`), and bootstrap the compiling/tested 8-crate Rust workspace.  
**Exit Criterion:** All research, specifications, ADRs, TODOs, `ai-memory` durable pages, and workspace bootstrap gates (`cargo fmt`, `cargo clippy`, `cargo test`, `icx benchmark`, `icx doctor`) pass with zero warnings.  

- [Phase 0 Overview](phase-0-precursory-setup/overview.md)

---

### Phase 1 — Foundations, BLAKE3 CAS, LCM Merkle-DAG & `/dev/shm` Bus
**Goal:** Establish the 8-crate Rust Cargo workspace, FastCDC + BLAKE3 Content-Addressed Store, Lossless Context Management (`LCM`) Merkle-DAG, Cache-Epoch Prefix Controller, and lock-free `/dev/shm` Seqlock Ring Buffer.  
**Exit Criterion:** `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` pass with 100% lossless byte/line round-trip recovery and `< 1 µs` seqlock ring latency.  
**Estimated Duration:** 3 weeks  

- [Phase 1 Overview](phase-1-foundations/overview.md)
  - [Sprint 1 — Workspace, FastCDC & BLAKE3 CAS](phase-1-foundations/sprint-1-workspace-and-cas.md)
  - [Sprint 2 — Lossless Merkle-DAG (`LCM`) & Cache-Epoch Controller](phase-1-foundations/sprint-2-merkle-dag-and-epochs.md)
  - [Sprint 3 — Lock-Free `/dev/shm` Seqlock Telepathy Bus](phase-1-foundations/sprint-3-shm-seqlock-bus.md)

---

### Phase 2 — Alien Holographic Token Lattice (`AHTL`) & Structural Skeletonizers
**Goal:** Implement the non-human BPE-aligned `AHTL` codec, Braille-256 topological sketcher, domain-specific AST/log/diff skeletonizers, and pre-ingestion secret redaction gate.  
**Exit Criterion:** Achieves `12x–40x` compression ratio on real agent transcripts/logs with 100% identifier/line preservation, zero delimiter-injection vulnerabilities, and 100% secret redaction coverage.  
**Estimated Duration:** 3 weeks  

- [Phase 2 Overview](phase-2-alien-codec-and-compression/overview.md)
  - [Sprint 1 — AHTL Grammar, Parser & Braille-256 Bijection](phase-2-alien-codec-and-compression/sprint-1-ahtl-grammar-and-braille.md)
  - [Sprint 2 — AST, Test-Log & Diff Structural Skeletonizers](phase-2-alien-codec-and-compression/sprint-2-structural-skeletonizers.md)
  - [Sprint 3 — Pre-Ingestion Secret Redactor & Delimiter Escaping](phase-2-alien-codec-and-compression/sprint-3-secret-redaction-and-safety.md)

---

### Phase 3 — Hyperdimensional (`HDC`), Fractal (`FCGA`) & Bi-Temporal Graph Index
**Goal:** Build the sub-50µs 10,240-bit HDC associative memory engine, Contractive IFS Fractal Trajectory Archive, Titans Surprise-Gated Momentum tracker, and Bi-Temporal CSR Graph with HippoRAG Personalized PageRank.  
**Exit Criterion:** Associative recall queries complete in `< 50 µs` over 10,000 nodes; Fractal suffix matching identifies shared trajectory suffixes in $O(1)$ time; Bi-Temporal functional edges auto-invalidate stale facts.  
**Estimated Duration:** 3 weeks  

- [Phase 3 Overview](phase-3-hyperdimensional-and-fractal-index/overview.md)
  - [Sprint 1 — 10,240-Bit HDC/HRR Engine & Infini-Attention Delta Bundler](phase-3-hyperdimensional-and-fractal-index/sprint-1-hdc-10240bit-engine.md)
  - [Sprint 2 — Fractal Chaos-Game Archive (`FCGA`) & Titans Surprise Gate](phase-3-hyperdimensional-and-fractal-index/sprint-2-fractal-ifs-and-surprise-gate.md)
  - [Sprint 3 — Bi-Temporal CSR Knowledge Graph & Personalized PageRank (`PPR`)](phase-3-hyperdimensional-and-fractal-index/sprint-3-bitemporal-graph-and-ppr.md)

---

### Phase 4 — Cross-Agent Low-Level Interception, MCP Pager & Wire Proxy
**Goal:** Wire native lifecycle hook adapters for Claude Code (`2.1.326+`), OpenAI Codex (`0.169.0+`), Antigravity CLI (`agy`), and Hermes Agent, plus the Single-Tool Polymorphic MCP Pager (`icx`), SQLite WAL tailer, and HTTP/2 Cache-Epoch Reverse Proxy.  
**Exit Criterion:** All four agent adapters pass wire-schema conformance tests; `PostToolUse` rewrites bulky tool outputs into AHTL stubs; cross-agent telepathy propagates across harnesses in `< 100 µs`.  
**Estimated Duration:** 3 weeks  

- [Phase 4 Overview](phase-4-cross-agent-interception/overview.md)
  - [Sprint 1 — Claude Code & OpenAI Codex Hook & WAL Adapters](phase-4-cross-agent-interception/sprint-1-claude-and-codex-adapters.md)
  - [Sprint 2 — Antigravity CLI (`agy`) & Hermes ContextEngine Adapters](phase-4-cross-agent-interception/sprint-2-agy-and-hermes-adapters.md)
  - [Sprint 3 — Single-Tool Polymorphic MCP Pager & Cache-Epoch Wire Proxy](phase-4-cross-agent-interception/sprint-3-mcp-pager-and-wire-proxy.md)

---

### Phase 5 — Long-Horizon Verification, Benchmarks & Production Packaging
**Goal:** Verify Infinity-Context on multi-million-token Needle-in-a-Haystack / OOLONG simulations, validate `>= 94%` prompt prefix-cache hit ratios, and ship the `icx` CLI (`doctor`, `benchmark`, `status`, systemd user service).  
**Exit Criterion:** 100% pass rate on multi-million-token passkey retrieval and multi-agent chaos tests; `icx benchmark` and `icx doctor` report all SLOs green.  
**Estimated Duration:** 2 weeks  

- [Phase 5 Overview](phase-5-verification-benchmarks-and-release/overview.md)
  - [Sprint 1 — Multi-Million-Token Needle-in-a-Haystack & Prefix-Cache Simulator](phase-5-verification-benchmarks-and-release/sprint-1-nih-and-prefix-cache-sim.md)
  - [Sprint 2 — CLI Tooling, `icx doctor`, `icx benchmark` & Release Packaging](phase-5-verification-benchmarks-and-release/sprint-2-cli-doctor-and-packaging.md)

---

## Cross-Phase Dependencies

- Phase 2 (`icx-codec`) depends on Phase 1 Sprint 1 (`ArtifactId`, `Blake3Digest` in `icx-core`) and Phase 3 Sprint 1 (`Hypervector` sketch in `icx-hdc`).
- Phase 4 (`icx-adapters`, `icx-proxy`) integrates Phase 1 (`icx-core`, `icx-bus`), Phase 2 (`icx-codec`), and Phase 3 (`icx-hdc`, `icx-store`).
- Phase 5 validates the composed 8-crate workspace end to end.
