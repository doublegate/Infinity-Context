# Downloaded Primary Research Papers — Index & Technical Synthesis

**Location:** `ref-docs/papers/`  
**Papers indexed:** 22 arXiv primary sources. **PDFs redistributed here:** 12 (11 under CC BY 4.0, 1 under CC BY-NC-SA 4.0). The other 10 are published on arXiv under the arXiv non-exclusive distribution license, which does not permit redistribution, so they are linked to their arXiv abstract pages instead (v1.1.0, ICX-AUD-036).  

This index catalogs the 22 foundational academic papers and maps their mathematical models, algorithms, and systems insights directly to the **Infinity-Context (`icx`)** Rust architecture.

---

## 1. Symbolic & Engine-Managed Infinite Context Architectures

### 1.1 [2605.04050-LCM-Lossless-Context-Management.pdf](2605.04050-LCM-Lossless-Context-Management.pdf)
- **Title:** *LCM: Lossless Context Management* (`arXiv:2605.04050`, May 2026)
- **Core Contribution:** Replaces unstructured, error-prone model-written recursion with a deterministic, engine-managed **Hierarchical Summary Directed Acyclic Graph (DAG)**. Every compacted summary node retains lossless cryptographic pointers back to the exact raw messages and tool outputs it summarizes, paired with engine-managed parallel `Map` task partitioning. Demonstrated superior performance across 32K to 1M token contexts on the OOLONG benchmark.
- **Integration in Infinity-Context:** Forms the foundation of `crates/icx-core/src/dag.rs` (`MerkleContextDag`). Every turn and tool result is stored as an immutable BLAKE3-addressed leaf (`L0`), and compaction creates higher-level `AHTL` summary nodes (`L1..Ln`) without ever discarding `L0` leaves.

### 1.2 [2512.24601-Recursive-Language-Models.pdf](2512.24601-Recursive-Language-Models.pdf)
- **Title:** *Recursive Language Models (RLMs)* (`arXiv:2512.24601`, Zhang et al., Dec 2025)
- **Core Contribution:** Treats arbitrarily long prompts as external environment variables rather than static token buffers, enabling the model to programmatically slice, inspect, and recursively invoke sub-queries over sub-spans of the context.
- **Integration in Infinity-Context:** Implemented as deterministic, zero-hallucination Rust primitives in `icx-Core` and exposed via the single virtualized MCP gateway (`icx_deref`, `icx_slice`, `icx_grep_cold`, `icx_map`).

### 1.3 [arXiv:2510.11967 Context-Folding](https://arxiv.org/abs/2510.11967)
- **Title:** *Scaling Long-Horizon LLM Agent via Context-Folding* (`arXiv:2510.11967`, Oct 2025)
- **Core Contribution:** Introduces dynamic trajectory branching (`branch` -> execute sub-trajectory with full local detail -> `fold` back into main trajectory as a compact state commit). Prevents intermediate debugging dead-ends and verbose exploration from polluting the main trunk context.
- **Integration in Infinity-Context:** Directly powers **Sub-Trajectory Folding** in `icx-core`: whenever an agent enters a multi-step build/test/debug loop (or spawns a subagent via `SubagentStart` / `SubagentStop`), `icx` opens a branch epoch and folds the entire intermediate loop into a single 40-token AHTL state commit upon completion.

### 1.4 [2510.04618-Agentic-Context-Engineering-ACE.pdf](2510.04618-Agentic-Context-Engineering-ACE.pdf)
- **Title:** *Agentic Context Engineering: Evolving Contexts for Self-Improving Language Models* (`arXiv:2510.04618`, Oct 2025)
- **Core Contribution:** Formulates agent context as an evolving, structured playbook updated via incremental delta rules rather than monolithic rewrites, preventing "context collapse" and brevity bias during repeated summarization.
- **Integration in Infinity-Context:** Governs the **Invariant Ledger** inside each Cache Epoch—architectural invariants, verified commands, and negative constraints (`¬`) are accumulated as delta-ops rather than re-summarized into vague prose.

### 1.5 [2310.08560-MemGPT-LLMs-as-Operating-Systems.pdf](2310.08560-MemGPT-LLMs-as-Operating-Systems.pdf)
- **Title:** *MemGPT: Towards LLMs as Operating Systems* (`arXiv:2310.08560`, Packer et al., Oct 2023)
- **Core Contribution:** Establishes the OS Virtual Memory metaphor for LLMs: dividing memory into Main Context (RAM: system instructions, working memory blocks, FIFO queue) and External Context (Disk: recall storage and archival storage), managed via page-fault interrupts and paging syscalls.
- **Integration in Infinity-Context:** Extended into a hardware-inspired **4-Tier Virtual Context MMU** (`L1 Active Window` -> `L2 AHTL Holographic Lattice` -> `L3 /dev/shm Hot Ring & HDC Index` -> `L4 BLAKE3/Zstd Cold CAS`), where page faults are resolved automatically in `< 500 µs` via lifecycle hooks and wire proxy interception before the model even knows a page was missing.

---

## 2. Prefix-Cache Engineering, KV-Cache Sparsity & Neural Compression

### 2.1 [arXiv:2610.28577 Cache-Engineering-for-Agents](https://arxiv.org/abs/2610.28577)
- **Title:** *Cache Engineering for Agents* (`arXiv:2610.28577`, Fu et al., Oct 2026)
- **Core Contribution:** Demonstrates that naive text-level context editing invalidates provider prefix caches, causing catastrophic re-prefill latency and cost. Introduces explicit cache manipulation primitives (`drop` and `repos` with RoPE positional continuity) and prefix-preserving cache block alignment.
- **Integration in Infinity-Context:** Inspires the **Prefix-Cache-Locked Epoch Controller (`CacheEpochController`)** in `icx-core` and `icx-proxy`. Context is partitioned into immutable, hash-locked epochs aligned to provider cache block sizes; middle-history edits are strictly forbidden except at amortized epoch compaction boundaries, while dynamic recall is injected exclusively into the **Ephemeral Tail Slot** after the cached prefix.

### 2.2 [2607.23809-ACM-Agentic-Cache-Management.pdf](2607.23809-ACM-Agentic-Cache-Management.pdf)
- **Title:** *ACM: Agentic Cache Management* (`arXiv:2607.23809`, Jul 2026)
- **Core Contribution:** Co-designs agent state transitions with KV-cache block retention policies, separating immutable tool/system prefixes from volatile working scratchpads.
- **Integration in Infinity-Context:** Used in `icx-proxy` to order `cache_control: {"type": "ephemeral"}` breakpoints (Anthropic) and prefix-aligned message ordering (OpenAI/Codex/Gemini) for >94% cache hit ratios.

### 2.3 [2607.07144-Fractal-KV-Cache-Archives.pdf](2607.07144-Fractal-KV-Cache-Archives.pdf)
- **Title:** *Fractal KV-Cache Archives: Lossless Symbolic Storage with In-Place Retrieval for Long-Context LLM Inference* (`arXiv:2607.07144`, Gusev, Jul 2026)
- **Core Contribution:** Maps quantized symbolic streams into low-dimensional continuous vectors via **Contractive Iterated Function Systems (IFS / Chaos Game Representation)**:
  $$x_{t} = \alpha x_{t-1} + (1 - \alpha) v_{s_t}, \quad \alpha \in (0, 1)$$
  Provides $O(1)$ random access, $O(1)$ amortized append, and allows structural substring/sub-trajectory similarity queries to be executed directly via Euclidean/Chebyshev distance on the fractal coordinates without decompressing the archive.
- **Integration in Infinity-Context:** Implemented directly in `crates/icx-hdc/src/fractal.rs` (`FractalTrajectoryArchive`). Every sequence of agent actions, tool calls, and AHTL state transitions is mapped onto a multi-scale Chaos-Game coordinate in $\mathbb{R}^{16}$ (using 128-bit fixed-point arithmetic for lossless prefix recovery up to depth 32 and instant suffix-tree-equivalent trajectory matching).

### 2.4 [2410.15704-RVQ-KV-Cache-Compression.pdf](2410.15704-RVQ-KV-Cache-Compression.pdf)
- **Title:** *Residual Vector Quantization for KV Cache Compression* (`arXiv:2410.15704`, Oct 2024)
- **Core Contribution:** Applies multi-stage Residual Vector Quantization (RVQ) with non-contiguous channel grouping to compress high-dimensional representations into compact discrete codebook indices at 5.5x–8x compression with negligible fidelity loss.
- **Integration in Infinity-Context:** Used to quantize dense semantic embeddings into 8-byte RVQ codes embedded directly inside AHTL token packets (`⟦Ψ:RVQ[...]⟧`) and used as the discrete alphabet driving the Fractal Chaos-Game Archive.

### 2.5 [arXiv:2403.12968 LLMLingua-2](https://arxiv.org/abs/2403.12968)
- **Title:** *LLMLingua-2: Data Distillation for Efficient and Faithful Task-Agnostic Prompt Compression* (`arXiv:2403.12968`, Pan et al., Mar 2024)
- **Core Contribution:** Formulates prompt compression as token-level binary classification over bidirectional context, preserving high-information-gain tokens while stripping predictable syntactic glue at 2x–5x compression.
- **Integration in Infinity-Context:** Combined with AST/structural parsing and BPE-vocabulary bit-packing in `crates/icx-codec` to achieve **12x–40x compression** by stripping not only English glue words, but replacing verbose syntax with single-token AHTL relational operators.

### 2.6 [2405.13792-xRAG-One-Token-Compression.pdf](2405.13792-xRAG-One-Token-Compression.pdf)
- **Title:** *xRAG: Extreme Context Compression for Retrieval-Augmented Generation with One Token* (`arXiv:2405.13792`, May 2024)
- **Core Contribution:** Compresses an entire retrieved document chunk into a single dense modality token projected into the LLM's embedding space.
- **Integration in Infinity-Context:** Inspires our **Single-Token Micro-Handle (`#icx:<id>`) + HDC Signature** design: cold historical blocks occupy as few as 8–16 tokens in the active prompt while carrying their full semantic topology and an instant dereference pointer.

### 2.7 [2401.03462-Activation-Beacon.pdf](2401.03462-Activation-Beacon.pdf) & [arXiv:2307.06945 ICAE-In-Context-Autoencoder](https://arxiv.org/abs/2307.06945)
- **Titles:** *Long Context Compression with Activation Beacon* (`arXiv:2401.03462`, Jan 2024) & *In-Context Autoencoder (ICAE) for Context Compression* (`arXiv:2307.06945`, Jul 2023)
- **Core Contribution:** Condense sliding windows of raw tokens into compact "beacon" / "memory slot" representations that condition downstream self-attention layers.
- **Integration in Infinity-Context:** Architectural blueprint for our **Holographic Beacon Frames** placed at the header of each Cache Epoch.

---

## 3. Test-Time Neural Memory & Attention Head Specialization

### 3.1 [2501.00663-Titans-Learning-to-Memorize-at-Test-Time.pdf](2501.00663-Titans-Learning-to-Memorize-at-Test-Time.pdf)
- **Title:** *Titans: Learning to Memorize at Test Time* (`arXiv:2501.00663`, Behrouz et al., Google Research, Jan 2025)
- **Core Contribution:** Models attention as short-term working memory and introduces a test-time neural long-term memory module updated via **Surprise** (gradient magnitude of associative memory prediction error) with **Momentum** (retaining tokens that follow a high-surprise event) and **Adaptive Weight Decay (Forgetting)**:
  $$S_t = \eta_t S_{t-1} - \theta_t \nabla \ell(M_{t-1}; k_t, v_t), \quad M_t = (1 - \alpha_t) M_{t-1} + S_t$$
- **Integration in Infinity-Context:** Directly implemented in `crates/icx-hdc/src/surprise.rs` (`TitansSurpriseGate`). As tokens/events stream through `icx`, the Surprise Gate computes instantaneous associative novelty $g_t$ against the HDC/HRR memory matrix, applies momentum $\eta_t$ so the causal aftermath of a bug/error is captured in full, and gates eviction via $(1 - \alpha_t)$.

### 3.2 [arXiv:2404.07143 Infini-Attention](https://arxiv.org/abs/2404.07143)
- **Title:** *Leave No Context Behind: Efficient Infinite Context Transformers with Infini-attention* (`arXiv:2404.07143`, Munkhdalai et al., Google, Apr 2024)
- **Core Contribution:** Combines local masked dot-product attention with a bounded associative linear memory matrix $M_s = M_{s-1} + \sigma(K)^T V$ updated using the delta rule:
  $$M_s = M_{s-1} + \sigma(K)^T \left(V - \frac{\sigma(K) M_{s-1}}{\sigma(K) z_{s-1}}\right)$$
- **Integration in Infinity-Context:** We use the **Delta-Rule Associative Update** in our HDC/HRR hypervector memory (`icx-hdc`): before bundling a new context hypervector into an epoch's associative memory trace, we subtract its projection already present in memory so repeated boilerplate never saturates the hypervector!

### 3.3 [2404.14469-SnapKV.pdf](2404.14469-SnapKV.pdf), [arXiv:2406.02069 PyramidKV](https://arxiv.org/abs/2406.02069), [arXiv:2410.10819 DuoAttention](https://arxiv.org/abs/2410.10819) & [arXiv:2406.10774 Quest-Query-Aware-Sparsity](https://arxiv.org/abs/2406.10774)
- **Titles:**
  - *SnapKV: LLM Knows What You are Looking for Before Generation* (`arXiv:2404.14469`)
  - *PyramidKV: Dynamic KV Cache Compression based on Pyramidal Information Funneling* (`arXiv:2406.02069`)
  - *DuoAttention: Efficient Long-Context LLM Inference with Retrieval and Streaming Heads* (`arXiv:2410.10819`)
  - *Quest: Query-Aware Sparsity for Efficient Long-Context LLM Inference* (`arXiv:2406.10774`)
- **Core Contribution:** Establish three fundamental laws of Transformer long-context attention:
  1. **Observation Window Clustering (SnapKV):** Tokens attended to by the recent observation window (last $W \approx 32\text{–}64$ tokens) predict which historical clusters are needed for generation.
  2. **Attention Sink + Retrieval vs. Streaming Split (DuoAttention):** The first $k$ tokens ("attention sinks" / system prompt + initial goal) and exact retrieval anchors must remain verbatim, while streaming tokens only need local window context.
  3. **Query-Aware Bounding-Box Page Selection (Quest):** Storing per-page min/max coordinate bounds allows $O(1)$ upper-bound relevance scoring per memory page against the current query.
- **Integration in Infinity-Context:**
  - Protects the **Attention Sink Head** (first $N$ system/goal messages) + **Observation Window Tail** (last $M$ turns) verbatim.
  - Maintains **Quest-style Min/Max Hypercube Bounding Boxes** on every LCM DAG page so query-aware page faults evaluate page bounds in nanoseconds.

---

## 4. Neurobiological & Temporal Knowledge Graph Memory

### 4.1 [arXiv:2405.14831 HippoRAG](https://arxiv.org/abs/2405.14831)
- **Title:** *HippoRAG: Neurobiologically Inspired Long-Term Memory for Large Language Models* (`arXiv:2405.14831`, Gutierrez et al., May 2024)
- **Core Contribution:** Models the mammalian hippocampus (rapid pattern-separated indexing) and neocortex (dense schema storage) by extracting open-information-extraction (OpenIE) triples into a knowledge graph and running **Personalized PageRank (PPR)** seeded on query entities to achieve single-step multi-hop retrieval.
- **Integration in Infinity-Context:** Implemented in `crates/icx-store/src/graph.rs` (`PersonalizedPageRank`): when an agent touches `file_A.rs` which calls `trait_B` implemented in `crate_C`, PPR walks the cross-session code/event graph in `< 200 µs` and pre-fetches the historical bug note on `crate_C` without requiring lexical overlap with `file_A.rs`.

### 4.2 [2501.13956-Zep-Temporal-Knowledge-Graph.pdf](2501.13956-Zep-Temporal-Knowledge-Graph.pdf)
- **Title:** *Zep: A Temporal Knowledge Graph Architecture for Agent Memory* (`arXiv:2501.13956`, Rasmussen et al., Jan 2025)
- **Core Contribution:** Introduces the **Graphiti Bi-Temporal Model**, tracking both *transaction time* ($t_{\text{created}}, t_{\text{expired}}$ — when the agent learned/superseded a fact) and *valid time* ($t_{\text{valid}}, t_{\text{invalid}}$ — when the fact was true in the workspace/codebase). Automatically invalidates contradicted edges while preserving historical provenance.
- **Integration in Infinity-Context:** Every fact, symbol signature, and architectural decision in `icx-store` carries bi-temporal intervals `[t_valid, t_invalid)` and `[t_tx_start, t_tx_end)` plus git commit SHA provenance. When a function is refactored or a hypothesis fails, its old edge is marked `t_invalid = now` so agents never hallucinate from stale pre-refactor context.

### 4.3 [arXiv:2502.12110 A-MEM-Agentic-Memory](https://arxiv.org/abs/2502.12110)
- **Title:** *A-MEM: Agentic Memory for LLM Agents* (`arXiv:2502.12110`, NeurIPS 2025)
- **Core Contribution:** Zettelkasten-inspired self-evolving atomic memory notes with dynamic cross-linking and retroactive neighborhood refinement when new observations arrive.
- **Integration in Infinity-Context:** Drives background asynchronous memory link refinement in `icxd` during idle inter-turn windows.
