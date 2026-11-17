# AGENTS.md — Infinity-Context (`icx`) Agent Guide

## 1. Project Summary

**Infinity-Context (`icx`)** is an 8-crate Rust systems architecture that provides unlimited effective context within and across AI coding and research agents (**Claude Code**, **OpenAI Codex CLI**, **Google Antigravity CLI (`agy`)**, **Hermes Agent**, and MCP/ACP clients). It operates as a Virtual Context Memory Management Unit (MMU) combining a non-human BPE-aligned symbolic codec (**AHTL — Alien Holographic Token Lattice**), a 100% lossless **BLAKE3 Merkle-DAG (`LCM`)**, a sub-50µs **10,240-Bit Hyperdimensional (`HDC`) + Fractal Chaos-Game (`FCGA`) + Bi-Temporal Personalized PageRank (`PPR`)** associative index, a **Prefix-Cache-Locked Epoch Controller (`>= 94%` cache hit rate)**, and a lock-free **`/dev/shm` Seqlock Telepathy Bus**.

---

## 2. Repository Map

- `ref-docs/initial-concept.md` — Initial concept and architectural vision.
- `ref-docs/research-report.md` — Comprehensive deep-research technical report citing 38 primary sources.
- `ref-docs/papers/` — 22 downloaded primary arXiv PDFs (`LCM`, `Cache Engineering`, `Fractal KV Archives`, `Titans`, `Infini-attention`, `RLMs`, `Context-Folding`, `ACE`, `HippoRAG`, `Zep/Graphiti`, `A-MEM`, `LLMLingua-2`, `xRAG`, `Activation Beacon`, `ICAE`, `MemGPT`, `SnapKV`, `PyramidKV`, `DuoAttention`, `Quest`, `RVQ`, `ACM`) + `INDEX.md`.
- `ref-docs/agent-internals/` — Reverse-Engineer-Anything (`rea`) binary & SQLite forensics (`rea-binary-and-sqlite-forensics.md`) and cross-agent hook/wire matrix (`cross-agent-hook-and-wire-matrix.md`).
- `ref-docs/prior-art/` — Comparative analysis of GitHub & industry prior art (`github-and-industry-landscape.md`).
- `ref-docs/novel-methods/` — Mathematical specs of AHTL, 10,240-bit HDC/HRR, Fractal IFS archives, Titans Surprise Gating, and Prefix-Cache-Locked paging (`non-human-neural-codecs-and-holographic-context.md`).
- `docs/` — Implementation-grade specifications:
  - `docs/architecture.md` — System overview, crate dependency DAG, and execution flows.
  - `docs/alien-holographic-codec.md` — AHTL grammar, Braille-256 bijection, structural skeletonizers, and secret redaction.
  - `docs/virtual-memory-and-dag-engine.md` — FastCDC, BLAKE3 CAS, Merkle-DAG (`LCM`), Context-Folding, and Cache-Epoch Controller.
  - `docs/hyperdimensional-and-fractal-index.md` — 10,240-bit HDC, Fractal Chaos-Game Archive, Titans Surprise Gate, and Bi-Temporal PPR Graph.
  - `docs/agent-adapters-and-interception.md` — Claude Code, Codex, Antigravity CLI (`agy`), Hermes, and HTTP/2 Wire Proxy adapters.
  - `docs/wire-protocol-and-shm-bus.md` — `/dev/shm` Seqlock Ring Buffer and Single-Tool Polymorphic MCP Pager.
  - `docs/data-model.md`, `docs/interfaces.md`, `docs/performance.md`, `docs/security-and-threat-model.md`, `docs/testing-strategy.md`, `docs/build-and-tooling.md`, `docs/glossary.md`, and `docs/adr/`.
  - `docs/diagrams/` — Normative Mermaid (`.mmd`) diagram sources.
  - `docs/images/` — Theme-adaptive dark/light SVG and PNG architecture, packet anatomy, and cache-epoch diagrams.
- `images/` — High-resolution logos and Graphviz (`.dot`, `.png`, `.svg`) 5-plane and 8-crate relationship graphs.
- `assets/` — Multi-size icon packs (`icons/`), wide and full banners (`branding/`), `1280x640` GitHub Social Preview card, web favicons/manifest (`web/`), and Linux desktop launcher (`linux/`).
- `scripts/` — Deterministic diagram and platform asset generators (`render_diagrams.py`, `generate_assets_and_graphs.py`).
- `to-dos/ROADMAP.md` — Hierarchical Phase/Sprint/Ticket plan (`Phase 1` through `Phase 5`).
- `crates/` — Rust Cargo workspace:
  - `crates/icx-core` — Core types, FastCDC, BLAKE3 CAS, `MerkleContextDag`, `CacheEpochController`.
  - `crates/icx-hdc` — 10,240-bit `Hypervector`, Infini-attention Delta bundler, `FractalCoord`, `TitansSurpriseGate`.
  - `crates/icx-codec` — `AhtlPacket` codec, Braille-256 sketch, `StructuralSkeletonizer`, `SecretRedactor`.
  - `crates/icx-store` — `BiTemporalGraph` and HippoRAG `PersonalizedPageRank`.
  - `crates/icx-bus` — Lock-free `SeqlockTelemetryBus` (`/dev/shm` + in-memory ring).
  - `crates/icx-adapters` — `InfinityContextMmu`, `ClaudeCodeAdapter`, `CodexAdapter`, `AntigravityAdapter`, `HermesAdapter`, `McpPagerServer`.
  - `crates/icx-proxy` — `WireProxyTransformer` (Anthropic & OpenAI Cache-Epoch wire transformation).
  - `crates/icx-cli` — `icx` binary entry point.

---

## 3. Recommended First Reads

1. `docs/architecture.md` — Understand the 5-plane architecture and 8-crate dependency graph.
2. `docs/alien-holographic-codec.md` & `docs/virtual-memory-and-dag-engine.md` — Understand AHTL packets and lossless `#icx:<hash>` dereferencing.
3. `docs/agent-adapters-and-interception.md` — Understand how Claude Code, Codex, Antigravity CLI, and Hermes hooks are intercepted.
4. `to-dos/ROADMAP.md` — Track active phases, sprints, and ticket IDs (`T-PS-NNN`).

---

## 4. Project Conventions & Quality Gates

- **Language:** Rust (Stable `1.80+`).
- **License & IP Governance:** `GPL-3.0-or-later` / `AGPL-3.0-or-later` for code (`LICENSE`, `COPYING`), `CC-BY-NC-ND-4.0` for specifications/diagrams, and patent-pending + commercial reservation (`LICENSING.md`, `PATENTS.md`, `COMMERCIAL.md`, `CLA.md`), enforced via REUSE 3.3 (`REUSE.toml` + `uvx reuse lint`).
- **Safety:** `unsafe_code = "forbid"` workspace-wide, except inside `crates/icx-bus` where POSIX `mmap` seqlock operations are isolated and documented with `// SAFETY:` invariants.
- **Formatting & Linting:**
  ```bash
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets -- -D warnings
  ```
- **Testing & Benchmarking:**
  ```bash
  cargo test --workspace
  cargo run --release -p icx-cli -- benchmark
  cargo run --release -p icx-cli -- doctor
  ```

---

## 5. Open Questions & Future Enhancements

- Tuning provider-specific `epoch_token_threshold` defaults (`16,384` tokens for Anthropic 4-breakpoint cache vs. `8,192` tokens for OpenAI automatic 1024-token prefix cache).
- Optional CUDA Tensor-Core Residual Vector Quantization (`RVQ`) acceleration on RTX 5080 for dense multimodal activations.
