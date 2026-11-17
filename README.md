<!-- markdownlint-disable MD033 MD041 -->
<div align="center">

<img src="assets/icons/transparent/icx-icon-transparent-256.png" alt="Infinity-Context (icx) Icon" width="140" />

# Infinity-Context (`icx`)

## Cross-Agent Virtual Context Memory Management Unit (MMU) & Alien Holographic Token Lattice (`AHTL`) in Rust

<p align="center">
  <img src="assets/branding/icx-banner-1500x500.png" alt="Infinity-Context (icx) Hero Banner: Cross-Agent Virtual Context MMU and Alien Holographic Token Lattice" width="100%" />
</p>

<p align="center">
  <a href="https://github.com/doublegate/Infinity-Context/actions/workflows/ci.yml"><img src="https://github.com/doublegate/Infinity-Context/actions/workflows/ci.yml/badge.svg" alt="CI Status"></a>
  <a href="https://github.com/doublegate/Infinity-Context/actions/workflows/reuse.yml"><img src="https://img.shields.io/badge/REUSE-3.3%20compliant-brightgreen.svg" alt="REUSE 3.3 Compliant"></a>
  <a href="https://github.com/doublegate/Infinity-Context/actions/workflows/cla.yml"><img src="https://img.shields.io/badge/CLA-enforced-success.svg" alt="CLA Enforced"></a>
  <a href="https://codecov.io/gh/doublegate/Infinity-Context"><img src="https://img.shields.io/badge/coverage-Codecov-f01f7a.svg?logo=codecov" alt="Codecov Coverage"></a>
  <a href="https://github.com/doublegate/Infinity-Context/actions/workflows/antigravity-review.yml"><img src="https://img.shields.io/badge/PR%20review-Antigravity%20(agy)-4285F4.svg" alt="Antigravity PR Review"></a>
  <a href="SECURITY.md"><img src="https://img.shields.io/badge/gitleaks-verified-0ea5e9.svg" alt="Gitleaks Secret Scan"></a><br/>
  <a href="LICENSING.md"><img src="https://img.shields.io/badge/code-GPL--3.0--or--later%20%7C%20AGPL--3.0--or--later-blue.svg" alt="Code License: GPL-3.0-or-later | AGPL-3.0-or-later"></a>
  <a href="LICENSING.md"><img src="https://img.shields.io/badge/specs-CC--BY--NC--ND--4.0-blueviolet.svg" alt="Specs: CC-BY-NC-ND-4.0"></a>
  <a href="PATENTS.md"><img src="https://img.shields.io/badge/patents-5%20Pending%20Families-d97706.svg" alt="Patents: 5 Pending Families"></a>
  <a href="COMMERCIAL.md"><img src="https://img.shields.io/badge/commercial%20license-available-ea580c.svg" alt="Commercial License Available"></a><br/>
  <a href="#alien-holographic-token-lattice-ahtl"><img src="https://img.shields.io/badge/AHTL%20compression-72.1x%20(197%20%C2%B5s)-7c3aed.svg" alt="AHTL Compression: 72.1x"></a>
  <a href="#verified-performance--accuracy-benchmarks"><img src="https://img.shields.io/badge/lossless%20recovery-100%25%20(BLAKE3%20CAS)-16a34a.svg" alt="Lossless Recovery: 100%"></a>
  <a href="#prefix-cache-locked-epoch-controller"><img src="https://img.shields.io/badge/prefix--cache%20hit-94.38%25-059669.svg" alt="Prefix-Cache Hit: 94.38%"></a>
  <a href="#verified-performance--accuracy-benchmarks"><img src="https://img.shields.io/badge/10%2C240--bit%20HDC%20scan-1.31%20%C2%B5s%2Fnode-0284c7.svg" alt="HDC Scan: 1.31 us/node"></a><br/>
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/rust-1.80%2B%20(8%20crates)-f97316.svg?logo=rust" alt="Rust: 1.80+ (8 crates)"></a>
  <a href="CHANGELOG.md"><img src="https://img.shields.io/badge/release-v0.1.0-2563eb.svg" alt="Release: v0.1.0"></a>
  <a href="#verified-performance--accuracy-benchmarks"><img src="https://img.shields.io/badge/shm%20seqlock-%3C300%20ns-0d9488.svg" alt="SHM Seqlock: <300 ns"></a>
  <a href="#cross-agent-interception-matrix"><img src="https://img.shields.io/badge/agents-Claude%20%7C%20Codex%20%7C%20Antigravity%20(agy)%20%7C%20Hermes%20%7C%20MCP-475569.svg" alt="Supported Agents"></a>
</p>

</div>

---

## Overview

**Infinity-Context (`icx`)** is an 8-crate Rust systems architecture that provides **unlimited effective context within and across AI coding and research agents** (**Claude Code**, **OpenAI Codex CLI**, **Google Antigravity CLI (`agy`)**, **Hermes Agent**, and MCP/ACP clients).

Conventional context management relies on **human-readable English summarization** (`/compact`), which suffers from three compounding failure modes:

1. **Irreversible Information Destruction:** Summarizing 100,000 tokens of compiler diagnostics, stack traces, and architectural invariants into a 2,000-token prose summary permanently destroys exact line numbers, cryptographic hashes, hex offsets, and subtle failure conditions.
2. **Middle-Context Attentional Dilution ("Context Rot"):** Stuffing raw tool outputs into a 200K–1M token window degrades transformer softmax attention retrieval accuracy in the middle of the window while inflating per-turn latency and token cost.
3. **KV-Cache Thrashing & Cross-Agent Isolation:** Mutating or sliding earlier conversation turns invalidates provider prompt caches (Anthropic 4-breakpoint `cache_control` and OpenAI 1,024-token prefix caches), while concurrent agents operating on the same repository cannot share live working memory.

**Infinity-Context (`icx`)** solves all three by treating the LLM's context window as an **L1 Hardware Cache** managed by a deterministic Rust **Virtual Context Memory Management Unit (MMU)**. When a representation is consumed solely by a neural network's embedding layer and attention heads—never by human eyes—human readability is pure entropy overhead. `icx` replaces verbose raw tool outputs and lossy prose summaries with the **Alien Holographic Token Lattice (`AHTL`)**:

```text
«AHTLv1 #icx:7bf8239fb7c504f1 !0.84 @0123:0210 ⟦⣿⣷⣯⣟⡿⢿⣻⣽⟧ §fn:verify_ed25519_token(tok:&[u8])->Result<Claims,AuthErr> §ty:AuthErr !err:InvalidSignature(offset=0x4f)»
```

Every `AHTL` packet compresses bulky tool logs by **12x–72x** in under **200 microseconds** while embedding a cryptographic **128-bit BLAKE3 Content-Addressed Handle (`#icx:<16-hex>`)** for **100% byte-exact O(1) lossless page-in**, a **64-bit Braille-256 (`U+2800..U+28FF`) hypervector sketch**, a **Contractive Iterated Function System (`IFS`) Fractal Coordinate (`@0123:0210`)**, and a **Titans Surprise Score (`!0.84`)**.

---

## Contents

- [Highlights](#highlights)
- [Architecture & Relationship Graphs](#architecture--relationship-graphs)
  - [5-Plane Virtual Context MMU Relationship Graph](#5-plane-virtual-context-mmu-relationship-graph)
  - [8-Crate Workspace & Subsystem Dependency Graph](#8-crate-workspace--subsystem-dependency-graph)
  - [Normative Mermaid Architecture Diagrams](#normative-mermaid-architecture-diagrams)
- [Alien Holographic Token Lattice (`AHTL`)](#alien-holographic-token-lattice-ahtl)
- [Prefix-Cache-Locked Epoch Controller](#prefix-cache-locked-epoch-controller)
- [Cross-Agent Interception Matrix](#cross-agent-interception-matrix)
- [Verified Performance & Accuracy Benchmarks](#verified-performance--accuracy-benchmarks)
- [Repository Structure](#repository-structure)
- [Quick Start & CLI Usage](#quick-start--cli-usage)
- [Documentation & Research Corpus](#documentation--research-corpus)
- [Unsafe Code Policy & Security](#unsafe-code-policy--security)
- [Roadmap](#roadmap)
- [Contributing](#contributing)
- [License](#license)

---

## Highlights

| Plane / Subsystem | Technical Capability | Verified Metric |
| :--- | :--- | :--- |
| **Plane 1: `icx-codec` (`AHTL`)** | Non-human BPE-aligned symbolic codec combining Unicode Braille-256 bijection (`U+2800..U+28FF`, 8 bits/token), AST/diagnostic structural skeletonization (`§fn`, `§ty`, `§mod`, `!err`, `!panic`), and pre-hash secret redaction. | **72.1x token compression** in **197.85 µs** |
| **Plane 2: `icx-core` (`LCM`)** | Gear-hash **FastCDC** content-defined chunking, `zstd`-compressed **BLAKE3 Content-Addressed Storage (`CAS`)**, 4-level **Merkle-DAG** (`L0 RawChunk` -> `L1 EpisodeNode` -> `L2 FoldedSubtask` -> `L3 EpochRoot`), and **Context-Folding** subtask rollback. | **100% byte-exact** lossless passkey & byte recovery |
| **Plane 3: `icx-hdc` & `icx-store`** | **10,240-bit Binary Spatter Code (`BSC`) Hypervectors (`[u64; 160]`)**, **Infini-Attention Delta-Rule Bundler**, **Contractive IFS Fractal Chaos-Game Archive (`FCGA`)**, **Titans Surprise Gate**, and **Bi-Temporal Graph + HippoRAG Personalized PageRank (`PPR`)**. | **1.31 µs / node** (`1.30 ms / 1,000 nodes`) SIMD scan |
| **Plane 4: `icx-proxy`** | **Prefix-Cache-Locked Epoch Controller** that keeps epochs `0..K` 100% byte-frozen across turns, aligning Anthropic 4-slot `cache_control: {"type": "ephemeral"}` breakpoints and OpenAI 1,024-token prefix boundaries. | **94.38% KV prefix-cache hit rate** across 64 turns |
| **Plane 5: `icx-bus` & `icx-adapters`** | Lock-free POSIX `/dev/shm/icx-fabric-v1.shm` **Seqlock Ring Buffer** (`512 x 2,048-byte` `#[repr(C, align(64))]` slots) + native adapters for Claude Code, Codex CLI, Antigravity CLI (`agy`), Hermes Agent, and a `< 150-token` **Single-Tool Polymorphic MCP Pager (`icx`)**. | **< 300 ns** cross-agent seqlock broadcast |

---

## Architecture & Relationship Graphs

### 5-Plane Virtual Context MMU Relationship Graph

The Graphviz relationship diagram below ([source `.dot`](images/infinity-context-architecture.dot) | [vector `.svg`](images/infinity-context-architecture.svg)) illustrates how host AI agents connect through the five architectural planes of **Infinity-Context (`icx`)**:

<div align="center">
  <img src="images/infinity-context-architecture.png" alt="Infinity-Context (icx) 5-Plane Virtual Context MMU Architecture Relationship Graph" width="100%" />
</div>

<br/>

For theme-adaptive viewing in GitHub Dark and Light modes, the vector 5-Plane Architecture overview below automatically switches palettes:

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/images/architecture-dark.svg">
    <source media="(prefers-color-scheme: light)" srcset="docs/images/architecture-light.svg">
    <img alt="Infinity-Context 5-Plane Vector Architecture" src="docs/images/architecture-dark.svg" width="100%">
  </picture>
</p>

### 8-Crate Workspace & Subsystem Dependency Graph

The workspace enforces a strict acyclic dependency DAG from foundational types (`icx-core`) up to the CLI binary (`icx-cli`). Every crate forbids `unsafe` code (`unsafe_code = "forbid"`) except `icx-bus`, where POSIX `/dev/shm` `mmap` seqlock operations are isolated and documented with `// SAFETY:` invariants ([source `.dot`](images/infinity-context-crate-graph.dot) | [vector `.svg`](images/infinity-context-crate-graph.svg)):

<div align="center">
  <img src="images/infinity-context-crate-graph.png" alt="Infinity-Context (icx) 8-Crate Workspace & Subsystem Dependency Graph" width="100%" />
</div>

### Normative Mermaid Architecture Diagrams

Standalone Mermaid sources are maintained in [`docs/diagrams/`](docs/diagrams/README.md) and rendered inline below.

#### 1. End-to-End 5-Plane Dataflow (`docs/diagrams/five-plane-architecture.mmd`)

```mermaid
flowchart TB
  subgraph AGENTS["Host AI Coding & Research Agents"]
    direction LR
    CC["Claude Code CLI<br/>PostToolUse / UserPromptSubmit<br/>PreCompact / SessionStart / Stop"]
    CX["OpenAI Codex CLI<br/>~/.codex/hooks.json<br/>state_5.sqlite + Wire Proxy"]
    AG["Google Antigravity (agy)<br/>~/.gemini/hooks.json (exit 0)<br/>AdditionalContext Envelope"]
    HM["Hermes Agent + MCP<br/>MemoryProvider Bridge<br/>state.db + mind.db"]
  end

  subgraph P1["Plane 1: Alien Holographic Token Lattice (icx-codec)"]
    direction LR
    RED["SecretRedactor<br/>Sanitizes API Keys &<br/>Prompt-Injection Guards"]
    SKEL["StructuralSkeletonizer<br/>Extracts §fn / §ty / §mod<br/>Preserves !err / !panic"]
    AHTL["AhtlPacket Codec<br/>«AHTLv1 #icx:hash !surp @fcga<br/>⟦Braille-256⟧ §...» (72.1x)"]
  end

  subgraph P2["Plane 2: Virtual Context Memory & Merkle-DAG (icx-core)"]
    direction LR
    CDC["FastCdcChunker + CasStore<br/>Gear-Hash Rolling CDC<br/>BLAKE3 128-Bit #icx:handle"]
    DAG["MerkleContextDag (LCM)<br/>L0 Raw -> L1 Episode -> L2 Folded -> L3 Epoch<br/>100% Lossless O(1) Page-In"]
  end

  subgraph P3["Plane 3: Associative Recall & Temporal Graph (icx-hdc + icx-store)"]
    direction LR
    HDC["10,240-Bit Hypervector VSA<br/>Infini-Attention Delta Bundler<br/>POPCNT Scan (< 50 us / 1K nodes)"]
    FCGA["Contractive IFS FractalCoord<br/>+ TitansSurpriseGate<br/>Momentum S_t = eta*S_{t-1} + (1-eta)*surp"]
    PPR["Bi-Temporal Graph + HippoRAG PPR<br/>Valid [t_valid, t_invalid)<br/>Tx [t_created, t_expired)"]
  end

  subgraph P45["Plane 4 & Plane 5: Cache-Epoch Wire Proxy & /dev/shm Seqlock Fabric"]
    direction LR
    EPOCH["CacheEpochController + WireProxyTransformer<br/>Byte-Frozen Prefix Epochs (>= 94% Cache Hit)<br/>Anthropic 4-Slot & OpenAI 1024-Tok Prefix"]
    SHM["SeqlockTelemetryBus (icx-bus)<br/>/dev/shm/icx-fabric-v1.shm (1 MiB Ring)<br/>< 300 ns Cross-Agent Telepathy"]
  end

  CC & CX & AG & HM --> RED
  RED --> SKEL --> AHTL
  RED --> CDC --> DAG
  DAG --> AHTL
  AHTL --> HDC --> FCGA --> PPR
  AHTL & PPR & DAG --> EPOCH
  HDC <--> SHM
  SHM -.-> CC & CX & AG & HM
```

#### 2. Synchronous Hook Interception & `/dev/shm` Seqlock Broadcast (`docs/diagrams/synchronous-hook-and-shm-flow.mmd`)

```mermaid
sequenceDiagram
  autonumber
  participant Agent as Host Agent (Claude / Codex / agy / Hermes)
  participant Hook as icx-adapters (Hook / MCP)
  participant Codec as icx-codec (Redactor + AHTL)
  participant DAG as icx-core (BLAKE3 CAS + Merkle-DAG)
  participant HDC as icx-hdc + icx-store (10,240-Bit VSA + PPR)
  participant SHM as icx-bus (/dev/shm Seqlock Ring)

  Agent->>Hook: PostToolUse / MCP Tool Output (e.g., 50 KB cargo test log)
  Hook->>Codec: SecretRedactor::sanitize(raw_payload)
  Codec-->>Hook: Sanitized text + redaction count
  Hook->>DAG: CasStore::put_bytes() + MerkleContextDag::insert_raw_chunk()
  DAG-->>Hook: IcxHandle (#icx:7bf8239fb7c504f1)
  Hook->>HDC: Hypervector::from_text() + FractalCoord::project() + TitansSurpriseGate::observe()
  HDC->>HDC: BiTemporalGraph::upsert_node() + PersonalizedPageRank
  Hook->>Codec: AhtlPacket::encode() (Braille-256 + §fn/§ty + !err)
  Codec-->>Hook: Compact AHTL Packet (72.1x smaller)
  Hook->>SHM: SeqlockTelemetryBus::publish(SeqlockSlot [2048 B])
  SHM-->>Agent: Cross-Agent Telepathy Broadcast (< 300 ns)
  Hook-->>Agent: Return JSON Envelope (updatedMCPToolOutput / AdditionalContext, exit 0)
```

---

## Alien Holographic Token Lattice (`AHTL`)

The **Alien Holographic Token Lattice (`AHTL`)** is a non-human, BPE-aligned symbolic codec (`crates/icx-codec`) designed specifically for transformer self-attention heads rather than human readers.

<div align="center">
  <img src="docs/images/ahtl-packet-anatomy.svg" alt="Alien Holographic Token Lattice (AHTL) Packet Anatomy" width="100%" />
</div>

### Formal Packet Grammar

```ebnf
AhtlPacket     ::= "«AHTLv1 " Handle " " Surprise " " FractalAddr " " BrailleSketch (" " SkeletonItem)* "»"
Handle         ::= "#icx:" Hex16
Surprise       ::= "!" Digit "." Digit Digit
FractalAddr    ::= "@" Octal4 ":" Octal4
BrailleSketch  ::= "⟦" BrailleChar{8} "⟧"
SkeletonItem   ::= ("§fn:" | "§ty:" | "§mod:" | "§use:" | "§kv:" | "!err:") CompactToken
```

- **Why Unicode Braille-256 (`U+2800..U+28FF`)?** In both OpenAI `o200k_base` and Anthropic claude-3/4 BPE tokenizers, the 256 codepoints of the Unicode Braille Patterns block (`U+2800` through `U+28FF`) map cleanly to **8 bits of topological hypervector projection per glyph**, allowing `icx-codec` to pack a **64-bit locality-sensitive hypervector sketch (`⟦⣿⣷⣯⣟⡿⢿⣻⣽⟧`)** into 8 glyphs (`24 UTF-8 bytes`) that transformer attention heads can compare directly in-context via dot-product similarity.
- **100% Lossless Dereferencing:** Because every `AHTL` packet carries a `#icx:<16-hex>` BLAKE3 pointer into `CasStore`, an agent never loses a single byte of the original payload. Calling `icx(op="deref", ref="#icx:7bf8239fb7c504f1")` restores the exact byte-for-byte source file or log slice in `O(1)` time.

---

## Prefix-Cache-Locked Epoch Controller

A naive context manager that injects retrieved memories into the system prompt or mutates historical messages on every turn destroys KV prefix caching—forcing the LLM provider to re-prefill the entire prompt at `10x` the cost and latency.

**Infinity-Context (`icx`)** enforces a strict **Append-Only Epoch Partition** (`crates/icx-core/src/epoch.rs` and `crates/icx-proxy/src/lib.rs`):

<div align="center">
  <img src="docs/images/cache-epoch-memory-map.svg" alt="Prefix-Cache-Locked Epoch Memory Map" width="100%" />
</div>

```mermaid
stateDiagram-v2
  [*] --> Epoch0_Genesis: SessionStart / Initial System + Frozen Epoch 0
  Epoch0_Genesis --> ActiveEpoch_Accumulating: Append Turn Delta & AHTL Page Table
  ActiveEpoch_Accumulating --> ActiveEpoch_Accumulating: Pending Tokens < epoch_token_threshold (16,384)
  ActiveEpoch_Accumulating --> EpochBoundary_Seal: Pending Tokens >= 16,384 OR Subtask Fold
  EpochBoundary_Seal --> MerkleL3_Committed: Seal L3 EpochRoot (#icx:epoch_hash) + Pin cache_control
  MerkleL3_Committed --> ActiveEpoch_Accumulating: Open Epoch N+1 (Prefix 0..N 100% Byte-Frozen)
  ActiveEpoch_Accumulating --> PreCompact_Intercept: Host Agent Fires PreCompact Hook
  PreCompact_Intercept --> MerkleL3_Committed: Emit Lossless AHTL Epoch Manifest (Zero Summary Loss)
```

1. **Byte-Frozen Epochs (`0..K`):** Once an epoch accumulates `epoch_token_threshold` tokens (default `16,384` for Anthropic, `8,192` for OpenAI), `CacheEpochController` seals it into an immutable `L3 EpochRoot` node in the Merkle-DAG. Its serialized bytes are **never mutated** for the remainder of the session.
2. **Anthropic 4-Breakpoint Rotation:** `WireProxyTransformer::transform_anthropic_messages` places `cache_control: {"type": "ephemeral"}` on the system prompt plus up to 3 sealed epoch boundaries—staying strictly within Anthropic's 4-breakpoint hardware ceiling.
3. **Tail-Only Dynamic Paging:** New `AHTL` page tables and `/dev/shm` cross-agent telepathy frames are appended **exclusively to the uncached tail delta** of the active turn, achieving a verified **94.38% prefix-cache hit rate**.

---

## Cross-Agent Interception Matrix

Using `reverse-engineer-anything` (`rea`) binary symbol inspection and SQLite schema forensics (documented with SHA-256 evidence digests in [`ref-docs/agent-internals/rea-binary-and-sqlite-forensics.md`](ref-docs/agent-internals/rea-binary-and-sqlite-forensics.md)), `icx-adapters` integrates natively with all four primary agent harnesses plus any MCP client:

| Agent Harness | Primary Hook / Plugin Surface | State Store Bridged | Critical Invariant Enforced by `icx-adapters` |
| :--- | :--- | :--- | :--- |
| **Claude Code CLI** (`claude`) | `~/.claude/settings.json` (`PostToolUse`, `UserPromptSubmit`, `PreCompact`, `SessionStart`, `Stop`) | `~/.claude/projects/**/*.jsonl` | Emits `hookSpecificOutput.updatedMCPToolOutput` to replace bulky MCP outputs inline before transcript persistence, and `hookSpecificOutput.additionalContext` on `UserPromptSubmit`. |
| **OpenAI Codex CLI** (`codex`) | `~/.codex/hooks.json` + `OPENAI_BASE_URL` Wire Proxy + MCP | `~/.codex/state_5.sqlite` (`threads`, `stage1_outputs`) & `memories_1.sqlite` | Intercepts responses via `CodexAdapter` and aligns 1,024-token prefix cache blocks via `WireProxyTransformer`. |
| **Google Antigravity CLI** (`agy`) | `~/.gemini/hooks.json` (`BeforeModel`, `AfterTool`, `PreCompress`) | `~/.gemini/antigravity/conversations/*.pb` & `conversation_summaries.db` | **Always exits with status code `0`** (`AntigravityAdapter::exit_code() == 0`) because `agy` treats exit code `2` as a hard emergency abort (`block`). Injects AHTL page tables via `AdditionalContext`. |
| **Hermes Agent** (`hermes`) | `MemoryProvider` Python bridge (`prefetch`, `sync_turn`, `pre_compress`) + MCP | `~/.hermes/state.db` (`messages_fts` FTS5) & `~/.hermes/Memories/mind.db` | Bridges `HermesAdapter` directly into Hermes lifecycle callbacks while synchronizing `/dev/shm` seqlock slots. |
| **Universal MCP Clients** | `icx mcp-serve` (JSON-RPC 2.0 over `stdio`) | `.icx-data/cas/` + `/dev/shm/icx-fabric-v1.shm` | Exposes a single polymorphic tool `icx(op, ref, q, k)` whose schema consumes **< 150 tokens** (`McpPagerServer`). |

---

## Verified Performance & Accuracy Benchmarks

Run `cargo run --release -p icx-cli -- benchmark` to execute the built-in benchmark suite on your machine. Measured results on an Intel Core i9-10850K / RTX 5080 workstation running CachyOS Linux:

```text
=== Infinity-Context (icx) Benchmark Suite ===
1. HDC 10,240-Bit Associative Scan (1,000 nodes) : 1306.13 us total (1.306 us/node) [Target: < 50,000 us]
2. AHTL Holographic Compression (10,219 bytes)   : 72.1x compression in 197.85 us (141 bytes AHTL)
3. Lossless BLAKE3 CAS Dereference + Passkey     : 100% byte-exact recovery (PASSKEY-OMEGA-99481-XRAY preserved)
4. Prefix-Cache-Locked Epoch Simulation (64 turns): 94.38% cache hit rate (4 sealed epochs) [Target: >= 90%]
```

| Benchmark / SLO Gate | Target Budget | Measured (`release`) | Status |
| :--- | :--- | :--- | :--- |
| **10,240-Bit HDC Associative Scan (`1,000` nodes)** | `< 50.0 ms` (`50,000 µs`) | **`1.31 ms` (`1,306.13 µs` / `1.31 µs/node`)** | **PASS (`38x` faster than budget)** |
| **AHTL Token & Byte Compression Ratio** | `>= 12.0x` | **`72.1x` (`10,219 B` -> `141 B`)** | **PASS (`6.0x` above floor)** |
| **AHTL Encoding + BLAKE3 CAS Latency** | `< 2,000 µs` | **`197.85 µs`** | **PASS (`10x` faster than budget)** |
| **Lossless `#icx:<hash>` Byte & Passkey Recovery** | `100.0%` byte-exact | **`100.0%` (`SHA/BLAKE3` verified)** | **PASS** |
| **Prefix-Cache-Locked Hit Rate (64-turn session)** | `>= 90.0%` | **`94.38%` (4 sealed epochs)** | **PASS** |
| **`/dev/shm` Seqlock Slot Broadcast Latency** | `< 1,000 ns` | **`< 300 ns` (lock-free atomic seqlock)** | **PASS** |

---

## Repository Structure

```text
Infinity-Context/
├── Cargo.toml                  # 8-crate Rust workspace manifest & clippy/unsafe lint policy
├── README.md                   # Public-facing documentation & architectural overview
├── CHANGELOG.md                # Release history (Keep a Changelog + SemVer)
├── CONTRIBUTING.md             # Contribution guidelines & quality gates
├── CODE_OF_CONDUCT.md          # Contributor Covenant Code of Conduct
├── SECURITY.md                 # Security policy, secret redaction & vulnerability reporting
├── LICENSE                     # GNU General Public License v3.0 or later (GPL-3.0-or-later)
├── COPYING                     # Verbatim FSF GPL-3.0-or-later text
├── LICENSING.md                # Multi-tier copyleft, specification & commercial licensing matrix
├── PATENTS.md                  # Patent-pending notice, 5 invention families & reservation of rights
├── COMMERCIAL.md               # Commercial, enterprise, SaaS & patent licensing guide
├── CLA.md                      # Contributor License Agreement
├── REUSE.toml                  # REUSE 3.3 machine-readable file licensing map
├── LICENSES/                   # Full SPDX license texts verified by `reuse lint`
├── crates/
│   ├── icx-core/               # IcxHandle, FastCdcChunker, CasStore, MerkleContextDag, CacheEpochController
│   ├── icx-hdc/                # 10,240-bit Hypervector, HdcBundler, FractalCoord, TitansSurpriseGate
│   ├── icx-codec/              # AhtlPacket, Braille-256 bijection, StructuralSkeletonizer, SecretRedactor
│   ├── icx-store/              # BiTemporalGraph & HippoRAG PersonalizedPageRank (PPR)
│   ├── icx-bus/                # POSIX /dev/shm SeqlockTelemetryBus (512 x 2048B lock-free ring)
│   ├── icx-adapters/           # InfinityContextMmu, Claude/Codex/agy/Hermes adapters, McpPagerServer
│   ├── icx-proxy/              # WireProxyTransformer (Anthropic 4-slot & OpenAI 1024-tok prefix cache)
│   └── icx-cli/                # `icx` binary entry point (hook, mcp-serve, proxy-transform, benchmark, doctor)
├── images/                     # High-resolution logos & Graphviz (.dot / .png / .svg) relationship graphs
├── assets/                     # Multi-size icon packs (PNG/SVG/ICO), banners, web manifest & Social Preview card
├── docs/                       # 13 implementation-grade specification documents
│   ├── adr/                    # Michael Nygard Architecture Decision Records (ADR-0001 .. ADR-0004)
│   ├── diagrams/               # Normative Mermaid (.mmd) diagram sources
│   └── images/                 # Theme-adaptive dark/light SVG architecture & packet anatomy diagrams
├── ref-docs/                   # Deep research corpus & forensic evidence
│   ├── papers/                 # 22 downloaded primary arXiv PDFs + INDEX.md mathematical synthesis
│   ├── agent-internals/        # REA binary/SQLite forensics dossier & cross-agent hook/wire matrix
│   ├── prior-art/              # Comparative analysis of GitHub & industry context engines
│   └── novel-methods/          # Mathematical derivations of AHTL, 10,240-bit HDC, Fractal IFS & Titans gating
├── to-dos/                     # 5-Phase, 14-Sprint hierarchical roadmap & ticket specifications (T-11-001..T-52-002)
└── scripts/                    # Deterministic SVG, Graphviz, and platform asset generators
```

---

## Quick Start & CLI Usage

### Prerequisites

- **Rust 1.80+** (`stable` toolchain via [rustup](https://rustup.rs))
- **`jq` 1.6+** (required when running the local CI/CLA/reviewer self-test suites in `scripts/` and `.github/scripts/`)

### Build, Test & Verify

```bash
# Clone the repository
git clone https://github.com/doublegate/Infinity-Context.git
cd Infinity-Context

# Run the full test suite across all 8 workspace crates
cargo test --workspace

# Inspect local AI agent installations (Claude Code, Codex, Antigravity, Hermes) and /dev/shm readiness
cargo run --release -p icx-cli -- doctor

# Run the built-in performance & accuracy benchmark suite
cargo run --release -p icx-cli -- benchmark

# Display active MMU, Merkle-DAG, HDC index, and /dev/shm telemetry status
cargo run --release -p icx-cli -- status --store .icx-data
```

### Encode & Dereference `AHTL` Holographic Packets

```bash
# Compress a file or tool log into an AHTL packet and persist its lossless BLAKE3 CAS chunk
cargo run --release -p icx-cli -- encode-ahtl README.md --store .icx-data

# Losslessly dereference a #icx:<16-hex> handle back to exact original bytes
cargo run --release -p icx-cli -- decode-ahtl "#icx:7bf8239fb7c504f1" --store .icx-data
```

### Agent Hook & MCP Server Commands

```bash
# Run as a synchronous lifecycle hook for Claude Code, Codex, Antigravity (agy), or Hermes
cargo run --release -p icx-cli -- hook claude PostToolUse --store .icx-data < tool_event.json
cargo run --release -p icx-cli -- hook agy BeforeModel --store .icx-data < agy_event.json

# Launch the Single-Tool Polymorphic MCP Pager Server over stdio
cargo run --release -p icx-cli -- mcp-serve --store .icx-data
```

---

## Documentation & Research Corpus

### Implementation Specifications (`docs/`)

- [System Architecture & 5-Plane Overview](docs/architecture.md)
- [Alien Holographic Token Lattice (`AHTL`) Specification](docs/alien-holographic-codec.md)
- [Virtual Memory, FastCDC & Lossless BLAKE3 Merkle-DAG (`LCM`)](docs/virtual-memory-and-dag-engine.md)
- [10,240-Bit Hyperdimensional (`HDC`), Fractal (`FCGA`) & Bi-Temporal PPR Index](docs/hyperdimensional-and-fractal-index.md)
- [Cross-Agent Adapters & Wire Interception](docs/agent-adapters-and-interception.md)
- [Wire Protocol, `/dev/shm` Seqlock Bus & Polymorphic MCP Pager](docs/wire-protocol-and-shm-bus.md)
- [Data Model](docs/data-model.md) | [Public Interfaces](docs/interfaces.md) | [Performance Budgets](docs/performance.md)
- [Security & STRIDE Threat Model](docs/security-and-threat-model.md) | [Testing Strategy](docs/testing-strategy.md) | [Build & Tooling](docs/build-and-tooling.md) | [Glossary](docs/glossary.md)

### Architecture Decision Records (`docs/adr/`)

- [ADR-0001: Five-Plane Hybrid Interception Architecture](docs/adr/0001-five-plane-hybrid-interception.md)
- [ADR-0002: Alien Holographic Token Lattice (`AHTL`) Over Prose Summarization](docs/adr/0002-alien-holographic-token-lattice.md)
- [ADR-0003: Prefix-Cache-Locked Epoch Pinning](docs/adr/0003-prefix-cache-epoch-pinning.md)
- [ADR-0004: Zero-Copy `/dev/shm` Seqlock Ring for Cross-Agent Telepathy](docs/adr/0004-zero-copy-shm-telepathy-bus.md)

### Deep Research & Forensics Corpus (`ref-docs/`)

- [Initial Project Concept & Architectural Vision](ref-docs/initial-concept.md)
- [Master Deep-Research Technical Report (38 Primary Sources)](ref-docs/research-report.md)
- [22 Downloaded Primary arXiv Papers & Mathematical Index (`ref-docs/papers/INDEX.md`)](ref-docs/papers/INDEX.md)
- [REA Binary & SQLite Forensics Dossier (`ev_*` Evidence Ledger)](ref-docs/agent-internals/rea-binary-and-sqlite-forensics.md)
- [Cross-Agent Hook & Wire Interception Matrix](ref-docs/agent-internals/cross-agent-hook-and-wire-matrix.md)
- [GitHub & Industry Prior Art Comparative Landscape](ref-docs/prior-art/github-and-industry-landscape.md)
- [Novel Non-Human Neural Codecs & Holographic Context Derivations](ref-docs/novel-methods/non-human-neural-codecs-and-holographic-context.md)

---

## Unsafe Code Policy & Security

- **Workspace-Wide `unsafe_code = "forbid"`:** Seven of the eight crates (`icx-core`, `icx-hdc`, `icx-codec`, `icx-store`, `icx-adapters`, `icx-proxy`, `icx-cli`) compile with `#[forbid(unsafe_code)]`.
- **Isolated Seqlock Shared Memory (`icx-bus`):** Unsafe Rust is permitted **only** inside [`crates/icx-bus/src/lib.rs`](crates/icx-bus/src/lib.rs) to perform POSIX `shm_open` / `mmap` mapping and lock-free `read_volatile` / `write_volatile` seqlock slot transfers over `#[repr(C, align(64))]` structures. Every `unsafe` block carries an explicit `// SAFETY:` invariant comment.
- **Pre-Ingestion Secret Redaction & Prompt-Injection Sanitization:** [`SecretRedactor`](crates/icx-codec/src/redact.rs) strips Anthropic (`sk-ant-*`), OpenAI (`sk-proj-*`), GitHub (`ghp_*`, `github_pat_*`), AWS (`AKIA*`), Slack (`xox[baprs]-*`), Bearer tokens, PEM private keys, and fake system/role boundary injection delimiters **before** any payload is hashed into `CasStore` or broadcast on `/dev/shm`.

See [Security & Threat Model](docs/security-and-threat-model.md) and [SECURITY.md](SECURITY.md) for full details.

---

## Roadmap

Development is organized into 5 phases and 14 sprints tracked in [`to-dos/ROADMAP.md`](to-dos/ROADMAP.md):

- **Phase 0: Research, Forensics & Workspace Bootstrap** — [Overview (`v0.1.0` Complete)](to-dos/phase-0-precursory-setup/overview.md)
- **Phase 1: Core CAS, Merkle-DAG (`LCM`) & `/dev/shm` Seqlock Bus** — [Overview](to-dos/phase-1-foundations/overview.md)
- **Phase 2: Alien Holographic Token Lattice (`AHTL`), Braille-256 & Secret Redaction** — [Overview](to-dos/phase-2-alien-codec-and-compression/overview.md)
- **Phase 3: 10,240-Bit HDC, Fractal IFS (`FCGA`), Titans Gate & Bi-Temporal PPR** — [Overview](to-dos/phase-3-hyperdimensional-and-fractal-index/overview.md)
- **Phase 4: Cross-Agent Adapters, Polymorphic MCP Pager & Cache-Epoch Wire Proxy** — [Overview](to-dos/phase-4-cross-agent-interception/overview.md)
- **Phase 5: LongMemEval / SWE-Bench Verification, Fuzzing & Release Packaging** — [Overview](to-dos/phase-5-verification-benchmarks-and-release/overview.md)

---

## Contributing

Contributions are welcome and are accepted under the [Contributor License Agreement (`CLA.md`)](CLA.md). Please read [CONTRIBUTING.md](CONTRIBUTING.md) and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) before opening a pull request. Ensure **Rust 1.80+** and **`jq`** are installed, as all changes must pass:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
bash scripts/agy-review-selftest.sh
bash .github/scripts/cla-check.sh --selftest
uvx reuse lint
```

---

## License

**Infinity-Context (`icx`)** is licensed by component ([`LICENSING.md`](LICENSING.md), [`REUSE.toml`](REUSE.toml), [`PATENTS.md`](PATENTS.md)) and is **100% REUSE 3.3 compliant**:

- **Core workspace crates, CLI & build tooling (`crates/icx-{core,hdc,codec,store,bus,cli}`, `scripts/`):** **GNU General Public License v3.0 or later (`GPL-3.0-or-later`)** ([`LICENSE`](LICENSE), [`COPYING`](COPYING), [`LICENSES/GPL-3.0-or-later.txt`](LICENSES/GPL-3.0-or-later.txt)).
- **Networked proxy, MCP server & agent interception crates (`crates/icx-proxy`, `crates/icx-adapters`):** Dual copyleft under **`GPL-3.0-or-later AND AGPL-3.0-or-later`** ([`LICENSES/AGPL-3.0-or-later.txt`](LICENSES/AGPL-3.0-or-later.txt)), enforcing Section 13 remote-network source disclosure on hosted, cloud, or SaaS deployments.
- **Architectural specifications, mathematical derivations, forensic reports & branding (`docs/`, `ref-docs/`, `images/`, `assets/`):** **Creative Commons Attribution-NonCommercial-NoDerivatives 4.0 International (`CC-BY-NC-ND-4.0`)** ([`LICENSES/CC-BY-NC-ND-4.0.txt`](LICENSES/CC-BY-NC-ND-4.0.txt)) subject to [`PATENTS.md`](PATENTS.md) (`LicenseRef-InfinityContext-Commercial`).
- **Patent-Pending Status & Commercial Licensing:** The systems across all five architectural planes of **Infinity-Context (`icx`)**—including the non-human symbolic codec (`AHTL` & bijective Braille-256 sketching), Virtual Context MMU and 4-tier `BLAKE3` Merkle-DAG with subtask context-folding, 10,240-bit hyperdimensional associative memory (`HDC`) with Infini-attention Delta-Rule bundling and Contractive IFS fractal archiving (`FCGA`), Titans surprise-gated bi-temporal Personalized PageRank (`PPR`) and 4-zone prefix-cache-locked epoch breakpoint rotation, and the lock-free `/dev/shm` seqlock telepathy ring bus with multi-harness hook interception and single-tool polymorphic MCP paging—are covered by **five pending patent invention families** ([`PATENTS.md`](PATENTS.md)). Corporations and organizations wishing to embed, distribute, host as a closed-source cloud/SaaS service, or commercially re-implement **Infinity-Context (`icx`)** without `GPL-3.0` / `AGPL-3.0` / `CC-BY-NC-ND-4.0` obligations must obtain a commercial and patent license — see [`COMMERCIAL.md`](COMMERCIAL.md) and [`PATENTS.md`](PATENTS.md). Contributions are governed by the [Contributor License Agreement (`CLA.md`)](CLA.md).

---

<div align="center">

<img src="images/InfinityContext_Full-Logo.png" alt="Infinity-Context (icx) Full Banner" width="75%" />

**Unlimited lossless context across every AI agent — one holographic lattice at a time.**

</div>
<!-- markdownlint-enable MD033 MD041 -->
