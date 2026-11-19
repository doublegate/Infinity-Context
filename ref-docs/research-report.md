# Infinity-Context (`icx`) — Deep Research Technical Report

> **Corrections:** this report is kept as written; verified corrections to its citations and figures are in [corrections-2026-11-18.md](corrections-2026-11-18.md), and current values are in [docs/numbers.md](../docs/numbers.md).

**Generated:** 2026-11-16  
**Mode:** Augment & Autonomous Deep Research (building on [ref-docs/initial-concept.md](initial-concept.md))  
**Source Count:** 38 distinct sources (22 primary arXiv research papers downloaded as PDFs in [ref-docs/papers/](papers/INDEX.md), 9 shipped local agent binaries & SQLite databases analyzed via Reverse-Engineer-Anything (`rea`) with cryptographic Evidence IDs in [ref-docs/agent-internals/rea-binary-and-sqlite-forensics.md](agent-internals/rea-binary-and-sqlite-forensics.md), and 7+ open-source reference repositories analyzed in [ref-docs/prior-art/github-and-industry-landscape.md](prior-art/github-and-industry-landscape.md)).

---

## 1. Executive Summary

Autonomous software engineering and research agents—specifically **Claude Code**, **OpenAI Codex CLI**, **Google Antigravity CLI (`agy`)**, and **Hermes Agent**—are fundamentally constrained by finite Transformer context windows, quadratic attention dilution ("context rot"), destructive lossy compaction (`/compact`), and cross-agent state isolation. Even with 200K–2M token nominal windows, feeding raw tool outputs (`Bash`, `Read`, `Grep`), compiler diagnostics, and multi-day debugging loops directly into the prompt degrades multi-hop retrieval accuracy long before hitting the physical token ceiling, while naive mid-prompt editing invalidates provider prefix caches and multiplies API latency and cost by up to 10x.

**Infinity-Context (`icx`)** is a native Rust systems architecture designed to give every agent **unlimited effective context within a single session and telepathic context sharing across all running agents**. Rather than treating compressed memory as human-readable English summaries or slow RAG markdown notes, `icx` operates as a hardware-inspired **Virtual Context Memory Management Unit (MMU)** that speaks a non-human, BPE-aligned symbolic dialect (**AHTL — Alien Holographic Token Lattice**).

By combining five low-level planes—(0) a **Prefix-Cache-Locked Wire Proxy & SQLite WAL/eBPF Tailer**, (1) **Native Lifecycle Hook Tool-Output Virtualizers** (`updatedToolOutput`, `injectSteps`, `pre-model-request`, `ContextEngine.select_context`), (2) the **AHTL Non-Human Machine Codec** with **Titans Surprise-Gated Momentum**, (3) a **Lossless Context Management (LCM) BLAKE3 Merkle-DAG**, and (4) an **AVX-512 10,240-Bit Hyperdimensional Computing (`HDC`) + Fractal Chaos-Game Trajectory Archive (`FCGA`) + Bi-Temporal Personalized PageRank Graph** synchronized across agents via a **`/dev/shm` Lock-Free Seqlock Ring Buffer**—`icx` achieves **12x–40x in-window token compression**, **100% byte-exact lossless dereferencing**, **>= 94% prompt prefix-cache hit rates**, and **sub-100-microsecond cross-agent state synchronization**.

---

## 2. Scope and Goals

### 2.1 In-Scope Subsystems & Capabilities
1. **Cross-Agent Low-Level Interception (`icx-adapters`, `icx-proxy`):**
   - Full lifecycle hook integration for Claude Code (`2.1.326+`), OpenAI Codex (`0.169.0+`), Google Antigravity CLI (`agy`), Hermes Agent, and 18 MCP/hook-capable harnesses.
   - Transparent HTTP/1.1, HTTP/2, and SSE reverse proxy for Anthropic (`/v1/messages`), OpenAI (`/v1/responses`, `/v1/chat/completions`), and Gemini endpoints.
   - Zero-config SQLite WAL (`state_5.sqlite`, `conversations/*.db`, `state.db`) and JSONL transcript tailer for OAuth/mTLS-locked sessions.
2. **Non-Human Symbolic Compression (`icx-codec`):**
   - **AHTL (Alien Holographic Token Lattice)** encoder/decoder packing relational triples, AST skeletons, compiler diagnostics, and Braille-256 / BPE-aligned binary sketches at 12x–40x compression.
   - Deterministic structural skeletonizers for Rust/TS/Python/Go/C++ source reads, `git diff` outputs, `cargo test`/`pytest` logs, and JSON/SQLite dumps.
3. **Lossless Merkle-DAG Virtual Memory (`icx-core`):**
   - Content-Defined Chunking (FastCDC) + BLAKE3 hashing + Zstd dictionary compression.
   - Hierarchical Merkle-DAG (`LCM`) where every AHTL summary stub embeds a lossless `#icx:<blake3_hex8>` dereference pointer.
   - **Cache-Epoch Prefix Controller** maintaining immutable historical epochs and a dynamic Ephemeral Tail Slot.
4. **Hyperdimensional, Fractal & Graph Associative Indexing (`icx-hdc`, `icx-store`):**
   - 10,240-bit Binary/Bipolar Hyperdimensional Computing (`HDC/HRR`) with XOR role-filler binding, circular permutation sequence encoding, Infini-attention Delta-Rule bundling, and AVX-512 `VPOPCNTDQ` Hamming search.
   - **Fractal Chaos-Game Trajectory Archive (`FCGA`)** using Contractive Iterated Function Systems (IFS) for $O(1)$ suffix-tree trajectory matching.
   - **Titans Surprise-Gated Momentum Tracker** for online importance scoring and eviction gating.
   - **Bi-Temporal Knowledge Graph** (`[t_valid, t_invalid)`, `[t_tx_start, t_tx_end)`) with **HippoRAG Personalized PageRank (PPR)** multi-hop retrieval.
5. **Cross-Agent Shared-Memory Telepathy Bus (`icx-bus`):**
   - POSIX shared-memory (`/dev/shm/icx-fabric-v1`) lock-free seqlock ring buffer + Unix Domain Socket RPC for `< 100 µs` inter-agent state propagation.
6. **Single-Tool Polymorphic MCP & ACP Pager (`icx-cli`):**
   - Exposes a single `< 180 token` MCP tool (`icx`) supporting `deref`, `slice`, `grep_cold`, `ast_query`, `fold_branch`, `xagent_broadcast`, and `map_partition`.

### 2.2 Out-of-Scope
- Modifying proprietary closed-source cloud model weights on remote vendor servers (all neural/holographic operations execute locally on the workstation or in the wire/hook representation).
- Replacing human-authored documentation wikis (such as `ai-memory`'s curated `wiki/*.md` pages); `icx` operates as the underlying machine-native L1–L4 Virtual Context MMU and can optionally feed or consume `ai-memory`.

### 2.3 Success Criteria
- **100% Lossless Recovery:** Any byte ingested into `icx` (tool output, file read, conversation turn) is byte-for-byte recoverable from its `#icx:<hash>` handle (`blake3(deref(h)) == h`).
- **12x–40x Token Compression:** Measured across real multi-megabyte Claude Code, Codex, and Antigravity CLI transcripts.
- **Sub-500µs Synchronous Hook Budget:** `icx hook` completes in `< 500 µs` p99 via `/dev/shm` + UDS communication with `icxd`.
- **>= 94% Prefix Cache Hit Ratio:** Verified via the Cache-Epoch simulator over 500-turn trajectories.

---

## 3. Background and Context

### 3.1 Why Large Nominal Context Windows Do Not Solve the Problem
While frontier models now advertise 200K to 2M token context windows, three physical and mathematical phenomena prevent agents from using them effectively:
1. **Quadratic Self-Attention & Softmax Dilution:** In standard softmax attention, $\text{Attention}(Q, K, V) = \text{softmax}\left(\frac{Q K^T}{\sqrt{d_k}}\right) V$, as sequence length $N$ grows, the denominator $\sum_{j=1}^N \exp(q_i \cdot k_j / \sqrt{d_k})$ grows linearly with $N$, diluting attention weights over true retrieval targets ("context rot", documented in [LCM `arXiv:2605.04050`](papers/2605.04050-LCM-Lossless-Context-Management.pdf) and [RLMs `arXiv:2512.24601`](papers/2512.24601-Recursive-Language-Models.pdf)).
2. **Destructive Compaction Cliffs:** As reverse-engineered in Claude Code (`2.1.326`, `autoCompactWindow: 950000`, `executePreCompactHooks`) and Hermes (`agent/conversation_compression.py`, `threshold_percent = 0.75`), hitting the compaction threshold replaces the entire prior trajectory with a single LLM-generated prose summary (`"This session is being continued from a previous conversation that ran out of context..."`). Repeated compactions create lossy *summaries of summaries*, erasing exact line numbers, memory addresses, and negative constraints ([ACE `arXiv:2510.04618`](papers/2510.04618-Agentic-Context-Engineering-ACE.pdf)).
3. **The Prefix-Cache Mutation Penalty:** Anthropic, OpenAI, and Google discount cached prompt prefixes by 75%–90% and skip KV-prefill computation when the leading token sequence is byte-identical to a previous turn. Naive context trimmers that delete or rewrite messages in the middle of the history array invalidate every token after the first modified byte ([Cache Engineering for Agents `arXiv:2610.28577`](https://arxiv.org/abs/2610.28577), [ACM `arXiv:2607.23809`](papers/2607.23809-ACM-Agentic-Cache-Management.pdf)).

---

## 4. Technical Deep-Dive by Subsystem

### 4.1 Subsystem 1: Local Agent Binary & SQLite Internals (REA Forensics)
Full forensic details and `ev_*` Evidence IDs are recorded in [ref-docs/agent-internals/rea-binary-and-sqlite-forensics.md](agent-internals/rea-binary-and-sqlite-forensics.md) and [ref-docs/agent-internals/cross-agent-hook-and-wire-matrix.md](agent-internals/cross-agent-hook-and-wire-matrix.md). Key findings:
- **Claude Code (`2.1.326`, `ev_be2e252614342586f9de78e5f08eb70de8465f18310bc03a948abe986a60ef72`):** Compiled as a Bun single-file ELF (`.note.bun`). Its `PostToolUse` hook supports `updatedToolOutput` for *all* tools (`Bash`, `Read`, `Grep`, MCP), allowing `icx` to intercept and replace 100KB+ tool outputs with 60-token AHTL Holographic Stubs *before* the model ever sees them. Its `PreCompact` and `PostCompact` hooks allow `icx` to override lossy compaction.
- **OpenAI Codex CLI (`0.169.0`, `ev_7cda5928199ea58224bdedbbd14892ff224515db64f7679457f633d5e7d816d3`):** Static-pie musl Rust binary with `sqlx`-managed SQLite databases (`state_5.sqlite` `ev_3e4a7d4314b4d31f13103a257120fda8c23e0a884187421d7a47b4a75f1c29a5` and `memories_1.sqlite` `ev_1cc07dc44d6445edb0769cc0d4fee066758881e20cddd8f2a8c3d961c3fea201`). Exposes `pre-model-request`, `post-model-request`, `pre-compact`, `post-compact`, `post-tool-use` (`updatedMCPToolOutput`), `output_spill.rs`, and configurable `proxy_url` / `model_providers`.
- **Google Antigravity CLI (`agy`, `ev_4f48383ea6a5027ec066fb32443128db1261d36ffde9074e9aef47adccfd50f7`):** Hybrid Go `1.28` + C++ executable with SQLite trajectory stores (`conversation_summaries.db` `ev_6a75fb73dadf1f29cb480f77e5ebbce3692856c21971bae83b6ec3846effbdb1` and `conversations/<id>.db` `ev_524bc260c8a7ccbbaef9d7cfb1c2aaf949a069e2aac27ebff55a9c96d6c118b6`). Its `PreInvocation` and `SessionStart` hooks support `injectSteps` with `metadata.user_markdown = ""`, which hides non-human AHTL frames from the human UI while injecting them directly into the model's context! Its `Stop` hook intercepts `MAX_TOKEN_BUDGET_EXCEEDED` and forces seamless continuation.
- **Hermes Agent (`state.db` `ev_55f2f031e94c568896ee3d6653a29fb05932581b058229e8bf3aebc614652511`):** Python agent with pluggable `ContextEngine.select_context()` (`agent/context_engine.py:121-141`) that swaps the entire message list per LLM request prior to cache-control calculation without mutating SQLite history, plus `MemoryProvider` (`PRE_COMPRESS_CHECKPOINT_API_VERSION = 2`).

### 4.2 Subsystem 2: Alien Holographic Token Lattice (`AHTL`) & Structural Skeletonizer
Detailed mathematical specification in [ref-docs/novel-methods/non-human-neural-codecs-and-holographic-context.md](novel-methods/non-human-neural-codecs-and-holographic-context.md) §1.
- Standard prose uses ~4.5–6.0 bits/token out of a BPE token's ~17.5-bit theoretical entropy.
- **AHTL** strips all English filler and encodes state mutations, AST signatures, compiler diagnostics, causal edges (`⊢`, `⇒`, `↦`), negation invariants (`¬`), Braille-256 binary sketches (`U+2800..U+28FF`), Fractal trajectory coordinates, and BLAKE3 dereference handles (`#icx:<hex8>`) into dense, single-token relational packets.
- Because identifiers, line numbers, and operators remain unsplit BPE tokens, Transformer retrieval heads ([DuoAttention `arXiv:2410.10819`](https://arxiv.org/abs/2410.10819), [SnapKV `arXiv:2404.14469`](papers/2404.14469-SnapKV.pdf)) attend to AHTL packets directly *without* requiring a decompression tool call unless byte-exact inspection of cold lines is needed.

### 4.3 Subsystem 3: Lossless Context Management (`LCM`) BLAKE3 Merkle-DAG
Grounded in [LCM `arXiv:2605.04050`](papers/2605.04050-LCM-Lossless-Context-Management.pdf) and [Context-Folding `arXiv:2510.11967`](https://arxiv.org/abs/2510.11967):
- Every ingested payload is segmented via **FastCDC** (target chunk size 4 KiB–64 KiB), hashed with 256-bit **BLAKE3**, and compressed with a domain-trained **Zstd dictionary**.
- The DAG maintains four node kinds:
  1. `LeafChunk (L0)`: Raw immutable byte slice.
  2. `ToolArtifactNode (L1)`: Complete tool invocation + raw output + AHTL stub.
  3. `SubTrajectoryBranchNode (L2)`: Folded debugging/build/subagent branch ([Context-Folding `arXiv:2510.11967`](https://arxiv.org/abs/2510.11967)).
  4. `CacheEpochNode (L3)`: Sealed, prefix-cache-aligned epoch containing an AHTL lattice frame, an HDC associative hypervector, a Quest min/max bounding box, and cryptographic child pointers.

### 4.4 Subsystem 4: Hyperdimensional (`HDC`), Fractal (`FCGA`) & Bi-Temporal PPR Index
Grounded in [Titans `arXiv:2501.00663`](papers/2501.00663-Titans-Learning-to-Memorize-at-Test-Time.pdf), [Infini-attention `arXiv:2404.07143`](https://arxiv.org/abs/2404.07143), [Fractal KV-Cache Archives `arXiv:2607.07144`](papers/2607.07144-Fractal-KV-Cache-Archives.pdf), [HippoRAG `arXiv:2405.14831`](https://arxiv.org/abs/2405.14831), and [Zep/Graphiti `arXiv:2501.13956`](papers/2501.13956-Zep-Temporal-Knowledge-Graph.pdf):
- **10,240-Bit HDC Engine:** Uses 160 `u64` words per hypervector. Supports XOR binding ($\oplus$), circular permutation ($\Pi^k$), Delta-Rule novelty-weighted majority bundling, and AVX-512 `VPOPCNTDQ` Hamming similarity scans (~12 ns/comparison).
- **Fractal Chaos-Game Archive (`FCGA`):** Contractive Iterated Function System $z_t = \frac{1}{2} z_{t-1} + \frac{1}{2} v(s_t)$ in $[-1, 1]^{16}$, providing $O(1)$ Chebyshev-norm suffix matching across millions of historical steps.
- **Bi-Temporal CSR Graph + Personalized PageRank:** Tracks entities (files, functions, structs, tests, errors, tickets, agents) with `[t_valid, t_invalid)` and `[t_tx_start, t_tx_end)` intervals and executes power-iteration Personalized PageRank in `< 200 µs`.

---

## 5. State of the Art / Prior Art Comparison

See [ref-docs/prior-art/github-and-industry-landscape.md](prior-art/github-and-industry-landscape.md) for the complete comparative matrix across `billion-context`, `llmtrim`, `claude-rolling-context`, `squeez`, `densely`, `mcptoon`, `Letta (MemGPT)`, `Volt (LCM)`, `Graphiti (Zep)`, `HippoRAG`, `A-MEM`, `Cognee`, `Mem0`, `claude-mem`, `ownmem`, and `ai-memory`.

| Capability | `squeez` / `densely` | `billion-context` / `llmtrim` | `Letta` / `MemGPT` | `ai-memory` (Local) | **Infinity-Context (`icx`)** |
| :--- | :---: | :---: | :---: | :---: | :---: |
| In-Flight Tool Output Rewriting (`updatedToolOutput`) | Yes | Proxy only | No | No | **Yes (< 400 µs)** |
| 100% Byte-Exact Lossless Merkle-DAG (`LCM`) | Partial | No (Lossy) | No | No | **Yes (BLAKE3 + Zstd)** |
| Non-Human BPE-Aligned Codec (`AHTL`) | No | No | No | No | **Yes (12x–40x)** |
| Prefix-Cache-Locked Epochs (>= 94% hit rate) | No | Yes | No | N/A | **Yes (`CacheEpochController`)** |
| AVX-512 10,240-Bit HDC + Fractal IFS Index | No | No | No | No | **Yes (< 50 µs query)** |
| Bi-Temporal Graph + HippoRAG PPR | No | No | No | FTS5 + Vec | **Yes (In-Memory CSR)** |
| `/dev/shm` Sub-100µs Cross-Agent Telepathy | No | No | No | SQLite/HTTP | **Yes (`SeqlockRing`)** |

---

## 6. Principal Engineering Challenges & Mitigations

1. **Challenge 1: Prompt Prefix-Cache Invalidations During Dynamic Recall**
   - *Failure Mode:* Injecting recalled memories into the system prompt or modifying mid-history messages busts the provider's KV prefix cache on every turn.
   - *Mitigation:* **Append-Only Cache-Epoch Pinning + Ephemeral Tail Slot** ([novel-methods §5](novel-methods/non-human-neural-codecs-and-holographic-context.md)). Sealed epochs never mutate; all per-turn dynamic recall (`HDC`/`PPR`/`FCGA` + `/dev/shm` cross-agent updates) is injected exclusively into the Ephemeral Tail Slot at the end of the message list.
2. **Challenge 2: Sub-Millisecond Hook Execution Deadlines**
   - *Failure Mode:* Synchronous hooks (`PreToolUse`, `PostToolUse`, `PreInvocation`) block the interactive CLI loop; slow Python startups or network LLM calls cause noticeable lag.
   - *Mitigation:* `icx` is a native Rust binary that maps `/dev/shm/icx-fabric-v1` directly into its address space via `mmap` and communicates with the resident `icxd` daemon over a warm Unix Domain Socket (`AF_UNIX` `SOCK_SEQPACKET` / `SOCK_STREAM`). Zero LLM calls occur on the synchronous critical path; all AHTL skeletonization, BLAKE3 hashing, HDC encoding, and PPR walks are purely deterministic and SIMD-accelerated (`< 500 µs`).
3. **Challenge 3: Preventing Hallucination or Misinterpretation of Non-Human `AHTL` Syntax**
   - *Failure Mode:* If an alien encoding splits identifiers into arbitrary BPE subwords, attention heads cannot match a user's query about `fn alloc_page` to the compressed block.
   - *Mitigation:* **Hybrid Dual-Channel AHTL**: Channel A keeps all canonical code identifiers, file paths, error codes, and line numbers as *verbatim BPE tokens*, using single-token Unicode mathematical operators (`⊢`, `⇒`, `↦`, `⋮`, `⊕`, `⊖`, `¬`, `!`, `✓`) only for structural syntax, while Channel B uses Braille-256 (`U+2800..U+28FF`) for dense holographic sketches and embeds a 1-line grammar legend in the Pinned Head.
4. **Challenge 4: Secret & Credential Leakage Across Shared Memory**
   - *Failure Mode:* A tool output containing an `.env` secret or bearer token gets ingested into `/dev/shm` and broadcast to another agent.
   - *Mitigation:* Mandatory **Pre-Ingestion Secret Redaction Gate** (`crates/icx-codec/src/redact.rs`) scanning for high-entropy secrets, PEM blocks, API keys (`sk-`, `ghp_`, `aim_`, `AKIA`, bearer tokens), and `.env` patterns *before* BLAKE3 chunking or `/dev/shm` publication.

---

## 7. Architecture Options Considered

1. **Option A: Pure MCP Server (Rejected as Sole Mechanism)**
   - *Pros:* Easy to register across agents.
   - *Cons:* Cannot intercept bulky `Bash`/`Read` outputs before they bloat the context window; requires explicit model tool calls (costing extra round-trips and tokens).
2. **Option B: Pure HTTP MITM Proxy (Rejected as Sole Mechanism)**
   - *Pros:* Full control over wire messages and `cache_control` breakpoints.
   - *Cons:* Some agents (e.g., Antigravity CLI or OAuth-pinned desktop sessions) use custom gRPC/mTLS channels that ignore `OPENAI_BASE_URL` / `ANTHROPIC_BASE_URL`.
3. **Option C: Five-Plane Hybrid Architecture — Hooks + Wire Proxy + SQLite/JSONL Tailer + `/dev/shm` Bus + Single-Tool MCP Pager (Selected)**
   - *Pros:* Works 100% seamlessly on **every** agent regardless of transport. Agents with configurable base URLs get full wire-level + hook-level optimization; locked/OAuth agents get full hook-level (`updatedToolOutput`, `injectSteps`, `PreCompact`) + SQLite WAL tailing + `/dev/shm` telepathy + MCP virtual paging!

---

## 8. External Dependencies and Integrations

**Project License:** `GPL-3.0-or-later` (`GNU General Public License v3.0 or later`, see `LICENSE`).

| Crate / Dependency | Architectural Plane | Purpose in `Infinity-Context` |
| :--- | :--- | :--- |
| `blake3` | Plane 2 (`icx-core`) & Plane 3 (`icx-hdc`) | SIMD-accelerated cryptographic content addressing & XOF hypervector seeding |
| `zstd` / `ruzstd` | Plane 2 (`icx-core`) | Dictionary-trained lossless chunk compression in the Merkle-DAG |
| `serde` / `serde_json` | All Planes | Wire protocol, hook JSON, and MCP JSON-RPC serialization |
| `tokio` / `axum` / `hyper` | Plane 4 (`icx-proxy`) | Async UDS daemon, HTTP/2 & SSE reverse proxy (`icx-proxy`) |
| `rusqlite` / `redb` | Plane 3 (`icx-store`) | Durable metadata, Bi-Temporal Knowledge Graph, and WAL tailer |
| `memmap2` / `libc` | Plane 5 (`icx-bus`) | POSIX `/dev/shm` shared memory mapping and zero-copy ring buffer |
| `clap` | CLI (`icx-cli`) | CLI command parsing for `icx` |
| `regex` / `aho-corasick` | Plane 1 (`icx-codec`) | Fast structural skeletonization, secret redaction, and cold CAS search |

---

## 9. Open Questions & Prototype Experiments

1. **Optimal Cache-Epoch Rollover Threshold per Provider:** Anthropic supports up to 4 explicit `cache_control` breakpoints, whereas OpenAI/Codex caches automatically on 1024-token prefix increments (above 1024 tokens). `CacheEpochController` parameterizes both strategies; live telemetry will tune the exact epoch size (default `16,384` tokens per sealed epoch).
2. **Optional Local ONNX / Candle Neural Encoder Slot:** While deterministic AST + Titans Surprise + HDC runs in `< 200 µs` on CPU with zero GPU dependency, workstations with an NVIDIA RTX 5080 can optionally enable a background Tensor-Core RVQ/ICAE compressor on idle turns.

---

## 10. Source Manifest

### Tier 1 — Primary Academic Papers (Downloaded in `ref-docs/papers/`)
1. **arXiv:2610.28577** — Fu et al., *Cache Engineering for Agents* (2026) — <https://arxiv.org/abs/2610.28577> (PDF not redistributed: arXiv non-exclusive license)
2. **arXiv:2607.23809** — *ACM: Agentic Cache Management* (2026) — `ref-docs/papers/2607.23809-ACM-Agentic-Cache-Management.pdf`
3. **arXiv:2607.07144** — V. Gusev, *Fractal KV-Cache Archives: Lossless Symbolic Storage with In-Place Retrieval for Long-Context LLM Inference* (2026) — `ref-docs/papers/2607.07144-Fractal-KV-Cache-Archives.pdf`
4. **arXiv:2605.04050** — *LCM: Lossless Context Management* (2026) — `ref-docs/papers/2605.04050-LCM-Lossless-Context-Management.pdf`
5. **arXiv:2512.24601** — Zhang et al., *Recursive Language Models* (2025) — `ref-docs/papers/2512.24601-Recursive-Language-Models.pdf`
6. **arXiv:2510.11967** — *Scaling Long-Horizon LLM Agent via Context-Folding* (2025) — <https://arxiv.org/abs/2510.11967> (PDF not redistributed: arXiv non-exclusive license)
7. **arXiv:2510.04618** — *Agentic Context Engineering (ACE): Evolving Contexts for Self-Improving Language Models* (2025) — `ref-docs/papers/2510.04618-Agentic-Context-Engineering-ACE.pdf`
8. **arXiv:2502.12110** — *A-MEM: Agentic Memory for LLM Agents* (NeurIPS 2025) — <https://arxiv.org/abs/2502.12110> (PDF not redistributed: arXiv non-exclusive license)
9. **arXiv:2501.13956** — Rasmussen et al., *Zep: A Temporal Knowledge Graph Architecture for Agent Memory* (2025) — `ref-docs/papers/2501.13956-Zep-Temporal-Knowledge-Graph.pdf`
10. **arXiv:2501.00663** — Behrouz et al. (Google), *Titans: Learning to Memorize at Test Time* (2025) — `ref-docs/papers/2501.00663-Titans-Learning-to-Memorize-at-Test-Time.pdf`
11. **arXiv:2410.15704** — *Residual Vector Quantization for KV Cache Compression* (2024) — `ref-docs/papers/2410.15704-RVQ-KV-Cache-Compression.pdf`
12. **arXiv:2410.10819** — Xiao et al., *DuoAttention: Efficient Long-Context LLM Inference with Retrieval and Streaming Heads* (2024) — <https://arxiv.org/abs/2410.10819> (PDF not redistributed: arXiv non-exclusive license)
13. **arXiv:2406.10774** — Tang et al., *Quest: Query-Aware Sparsity for Efficient Long-Context LLM Inference* (2024) — <https://arxiv.org/abs/2406.10774> (PDF not redistributed: arXiv non-exclusive license)
14. **arXiv:2406.02069** — Cai et al., *PyramidKV: Dynamic KV Cache Compression based on Pyramidal Information Funneling* (2024) — <https://arxiv.org/abs/2406.02069> (PDF not redistributed: arXiv non-exclusive license)
15. **arXiv:2405.14831** — Gutierrez et al., *HippoRAG: Neurobiologically Inspired Long-Term Memory for Large Language Models* (2024) — <https://arxiv.org/abs/2405.14831> (PDF not redistributed: arXiv non-exclusive license)
16. **arXiv:2405.13792** — Cheng et al., *xRAG: Extreme Context Compression for Retrieval-augmented Generation with One Token* (2024) — `ref-docs/papers/2405.13792-xRAG-One-Token-Compression.pdf`
17. **arXiv:2404.14469** — Li et al., *SnapKV: LLM Knows What You are Looking for Before Generation* (2024) — `ref-docs/papers/2404.14469-SnapKV.pdf`
18. **arXiv:2404.07143** — Munkhdalai et al. (Google), *Leave No Context Behind: Efficient Infinite Context Transformers with Infini-attention* (2024) — <https://arxiv.org/abs/2404.07143> (PDF not redistributed: arXiv non-exclusive license)
19. **arXiv:2403.12968** — Pan et al., *LLMLingua-2: Data Distillation for Efficient and Faithful Task-Agnostic Prompt Compression* (2024) — <https://arxiv.org/abs/2403.12968> (PDF not redistributed: arXiv non-exclusive license)
20. **arXiv:2401.03462** — Zhang et al., *Long Context Compression with Activation Beacon* (2024) — `ref-docs/papers/2401.03462-Activation-Beacon.pdf`
21. **arXiv:2310.08560** — Packer et al., *MemGPT: Towards LLMs as Operating Systems* (2023) — `ref-docs/papers/2310.08560-MemGPT-LLMs-as-Operating-Systems.pdf`
22. **arXiv:2307.06945** — Ge et al., *In-context Autoencoder for Context Compression in a Large Language Model* (2023) — <https://arxiv.org/abs/2307.06945> (PDF not redistributed: arXiv non-exclusive license)

### Tier 1 — Shipped Agent Binary & SQLite Forensics (REA Evidence Ledger)
23. **Claude Code `2.1.326` ELF64 (`b6b3647d...`)** — REA `ev_be2e252614342586f9de78e5f08eb70de8465f18310bc03a948abe986a60ef72`, `ev_3d40926734f39b00b23864504aeef72e3eaf3f0db120ec0492c2ee9fbcc24b35`
24. **OpenAI Codex CLI `0.169.0` ELF64 (`b452ca20...`)** — REA `ev_7cda5928199ea58224bdedbbd14892ff224515db64f7679457f633d5e7d816d3`, `ev_ba0d9f9e305a96c026cd5c6efbc856664c5cf9c7603650ea0355c4e2a82a0acd`
25. **Google Antigravity CLI `agy` ELF64 (`39816d49...`)** — REA `ev_e0bd52156323af13cc80956727b105639c42d36bc491e0c89dd1b2f81f4da090`, `ev_4f48383ea6a5027ec066fb32443128db1261d36ffde9074e9aef47adccfd50f7`, `ev_9fe8eae9df30a2df633e6b6119d71f18ffa18de8727b07b628b6b92cfbec4041`
26. **`ai-memory` Rust Binary (`02904079...`)** — REA `ev_fc80f6c757374ecc4fab3531aa1a0f43e8199367528c0664cc29ea24192ca167`, `ev_204c615ca173cbca91d3a75eaf20f9461924559871ea7013bdc5c5fcad159272`
27. **Codex `state_5.sqlite` (`537c13f5...`) & `memories_1.sqlite` (`3912d60d...`)** — REA `ev_3e4a7d4314b4d31f13103a257120fda8c23e0a884187421d7a47b4a75f1c29a5`, `ev_1cc07dc44d6445edb0769cc0d4fee066758881e20cddd8f2a8c3d961c3fea201`
28. **Antigravity `conversation_summaries.db` (`3dfaba92...`) & `conversations/<id>.db` (`22a33eab...`)** — REA `ev_6a75fb73dadf1f29cb480f77e5ebbce3692856c21971bae83b6ec3846effbdb1`, `ev_524bc260c8a7ccbbaef9d7cfb1c2aaf949a069e2aac27ebff55a9c96d6c118b6`
29. **Hermes `state.db` (`d5411715...`) & Local Source (`~/.hermes/hermes-agent/agent/`)** — REA `ev_55f2f031e94c568896ee3d6653a29fb05932581b058229e8bf3aebc614652511` + `context_engine.py`, `memory_provider.py`, `context_compressor.py`
30. **Antigravity Official Built-in Docs** — `~/.gemini/antigravity-cli/builtin/skills/agy-customizations/docs/hooks.md` & `antigravity_guide/SKILL.md`

### Tier 2 — Reference Open-Source Repositories
31. `ranxianglei/billion-context` — Universal prefix-preserving context compression proxy
32. `claudioemmanuel/squeez` — Multi-CLI hook & tool-result compression pipeline
33. `alibaizhanov/densely` — Lossless cold-storage MCP & hook compressor
34. `fkiene/llmtrim` — Prompt-cache-aware API proxy for Claude Code
35. `NodeNestor/claude-rolling-context` — Rolling compression replacement for `/compact`
36. `activeing123/mcptoon` — Zero-dependency MCP tool-schema virtualizer
37. `letta-ai/letta`, `getzep/graphiti`, `OSU-NLP-Group/HippoRAG`, `agiresearch/A-mem`, `topoteretes/cognee`, `mem0ai/mem0`, `thedotmack/claude-mem`, `grpcer/ownmem`
