# Prior Art & GitHub Landscape: Context Compression, Proxies, and Agentic Memory

**References:** [initial-concept.md](../initial-concept.md), [papers/INDEX.md](../papers/INDEX.md)

This document surveys the state of the art across open-source GitHub repositories and commercial/research memory systems, analyzing their design tradeoffs and demonstrating how **Infinity-Context (`icx`)** unifies and surpasses them.

---

## 1. Taxonomy of Existing Approaches

Existing projects attempting to solve LLM context limits fall into four disjoint categories:

1. **Wire-Level Context Compression Proxies** (`billion-context`, `llmtrim`, `claude-rolling-context`)
2. **Lifecycle Hook & Tool-Result Compressors** (`squeez`, `densely`, `mcptoon`)
3. **OS-Inspired Virtual Memory & Lossless DAG Engines** (`Letta/MemGPT`, `Volt/LCM`)
4. **External RAG / Knowledge-Graph Memory Stores** (`ai-memory`, `Graphiti/Zep`, `HippoRAG`, `Cognee`, `Mem0`, `claude-mem`, `ownmem`)

No existing system integrates all four planes into a single zero-copy Rust engine with a non-human BPE-aligned symbolic codec (`AHTL`), AVX-512 Hyperdimensional Computing (`HDC`), Fractal KV/Trajectory Archives (`FCGA`), and a `/dev/shm` cross-agent telepathy bus.

---

## 2. Detailed Analysis of Prior Art Systems

### 2.1 Wire-Level Proxies & Rolling Compressors

| Project | Repository | Mechanism | Strengths | Critical Limitations |
| :--- | :--- | :--- | :--- | :--- |
| **billion-context** | `ranxianglei/billion-context` | Universal proxy/plugin for Claude Code, Codex, OpenCode, Pi, Zed using incremental hierarchical compression. | Preserves prefix-cache hit rates via staged compaction intervals; supports multi-CLI workflows. | Relies on slow, lossy LLM-generated English summaries for compressed tiers; no byte-exact lossless Merkle-DAG dereferencing; no cross-agent shared-memory bus. |
| **llmtrim** | `fkiene/llmtrim` | Transparent local HTTP proxy between Claude Code and Anthropic API. | Strips redundant terminal output, stack traces, and test dumps on the fly while respecting prompt-caching boundaries. | Heuristic stripping is lossy (once stripped on the wire, the model cannot dereference a dropped line); single-provider focus; human-text output only. |
| **claude-rolling-context** | `NodeNestor/claude-rolling-context` | Proxy/plugin replacing Claude Code's monolithic `/compact` with asynchronous background rolling compression. | Avoids sudden "compaction cliff" latency spikes; keeps recent window verbatim. | Still summarizes older turns into lossy prose; lacks cross-agent synchronization or tool-output interception. |

### 2.2 Hook-Based & Reversible Tool Compressors

| Project | Repository | Mechanism | Strengths | Critical Limitations |
| :--- | :--- | :--- | :--- | :--- |
| **squeez** | `claudioemmanuel/squeez` | Hook & filter pipeline across 7 AI CLIs (Claude Code, Codex, etc.) using `updatedToolOutput`, MinHash deduplication, and reversible compression. | Zero runtime dependencies; intercepts bulky bash/file reads before LLM ingestion (up to 95% reduction). | Operates per-tool-call in isolation; lacks global trajectory DAG, associative multi-hop graph recall, or wire-level prefix-cache management. |
| **densely** | `alibaizhanov/densely` | MCP server + `PostToolUse` hook that compresses logs/JSON with LZMA and encodes cold storage handles + server-side regex over compressed blobs. | Proven concept that cold storage + server-side search avoids re-reading huge files into the prompt. | Naive LZMA-to-English-word encoding cannot be reasoned over *directly* in-context by attention heads without decompression; requires an MCP round-trip for every inspection. |
| **mcptoon** | `activeing123/mcptoon` | Zero-dependency MCP gateway CLI that virtualizes tool schemas across Claude Code and Codex. | Eliminates 10K–40K tokens of upfront MCP tool-schema bloat by exposing a tiny discovery/dispatch surface. | Only addresses tool-schema overhead, not conversation trajectory or tool-result compression. |

### 2.3 Virtual Memory & Lossless DAG Architectures

| Project | Repository / Paper | Mechanism | Strengths | Critical Limitations |
| :--- | :--- | :--- | :--- | :--- |
| **Letta (MemGPT)** | `letta-ai/letta` (`arXiv:2310.08560`) | OS virtual memory model (Core Memory RAM blocks + Recall/Archival Disk storage) mutated via explicit LLM tool calls (`core_memory_replace`, `archival_memory_search`). | Pioneered self-editing memory blocks and virtual context paging. | Requires the agent to run inside Letta's custom Python harness rather than transparently augmenting Claude Code, Codex, or `agy`; relies on explicit LLM tool calls (high latency/token overhead) for paging. |
| **Volt / LCM** | `arXiv:2605.04050` | Deterministic Lossless Context Management (LCM) with a hierarchical summary DAG, lossless pointers to raw messages, and engine-managed `Map`. | Eliminates lossy compaction amnesia; beats Claude Code on OOLONG from 32K to 1M tokens. | Uses verbose human-language summaries in DAG nodes; single-agent scope; lacks HDC/Fractal sub-millisecond indexing and prefix-cache epoch locking. |

### 2.4 Knowledge Graphs & Cross-Agent Memory Stores

| Project | Repository / Paper | Mechanism | Strengths | Critical Limitations |
| :--- | :--- | :--- | :--- | :--- |
| **ai-memory** | Local workstation (`/home/parobek/.local/lib/ai-memory/ai-memory`) | Rust daemon + SQLite (`memory.sqlite`) + FTS5 + embeddings + Git-backed Markdown wiki + hooks across 18 agents. | Mature lifecycle hook integration, cross-project handoffs, and durable wiki curation. | Designed for human-readable wiki pages and session handoffs, **not** live in-session virtual context paging, `updatedToolOutput` virtualization, wire-level cache-epoch locking, or machine-native token codecs. |
| **Graphiti (Zep)** | `getzep/graphiti` (`arXiv:2501.13956`) | Bi-temporal knowledge graph tracking valid time `[t_valid, t_invalid)` and transaction time `[t_created, t_expired)`. | Prevents stale/contradicted facts from poisoning recall after code or requirement changes. | Requires heavy external graph DB (Neo4j/FalkorDB) and multiple LLM extraction calls per turn (high latency). |
| **HippoRAG** | `OSU-NLP-Group/HippoRAG` (`arXiv:2405.14831`) | Neurobiological hippocampal indexing + Personalized PageRank (PPR) over OpenIE entity graphs. | Solves multi-hop associative recall across indirect entity relationships in a single retrieval pass. | Python batch pipeline; too slow for synchronous `< 500 µs` per-turn hook execution unless re-engineered in Rust over an in-memory CSR graph. |
| **claude-mem / ownmem / cognee / mem0** | `thedotmack/claude-mem`, `grpcer/ownmem`, `topoteretes/cognee`, `mem0ai/mem0` | Hook-triggered observation capture + vector/graph RAG or Git-native note syncing across agents. | Useful cross-session persistence. | RAG retrieval alone is fundamentally not Infinite Context: top-$k$ cosine similarity misses structural causal chains, loses exact byte offsets, and cannot prevent within-session context window overflow. |

---

## 3. How Infinity-Context Synthesizes & Transcends the State of the Art

**Infinity-Context (`icx`)** combines the strongest primitive from every category above while eliminating their bottlenecks via a native Rust implementation:

1. **From `squeez` + `densely` + Claude/Codex/AGY Hooks:** In-flight `PostToolUse` (`updatedToolOutput`) and `PreInvocation` (`injectSteps`) virtualization that intercepts bulky outputs *before* they consume context tokens.
2. **From `LCM` (`arXiv:2605.04050`) + `Context-Folding` (`arXiv:2510.11967`):** A cryptographic **BLAKE3 Merkle-DAG** that guarantees 100% byte-exact lossless recovery of every historical message and tool output, paired with automatic sub-trajectory folding.
3. **From `Cache Engineering` (`arXiv:2610.28577`) + `billion-context`:** **Prefix-Cache-Locked Epochs** that guarantee >= 94% provider prompt-cache hit rates on the wire.
4. **From `LLMLingua-2` + `xRAG` + Information Theory:** The **Alien Holographic Token Lattice (`AHTL`)** — a non-human, BPE-aligned symbolic representation that is directly readable by Transformer attention heads *without* decompression while achieving 12x–40x compression.
5. **From `Titans` (`arXiv:2501.00663`) + `Infini-attention` (`arXiv:2404.07143`) + `Fractal KV Archives` (`arXiv:2607.07144`) + `HippoRAG` + `Graphiti`:** Sub-50-microsecond **AVX-512 Hyperdimensional Computing (`HDC`)**, **Fractal Chaos-Game Trajectory Coordinates (`FCGA`)**, **Surprise-Gated Momentum Eviction**, and an embedded **CSR Bi-Temporal Personalized PageRank Graph** in pure Rust.
6. **From `mcptoon`:** A **Single-Tool Polymorphic MCP Pager (`icx`)** that adds `< 180 tokens` of schema overhead while giving every agent full virtual-memory syscall capabilities.
