# Glossary of Terms & Abbreviations

**References:** [ref-docs/research-report.md](../ref-docs/research-report.md), [docs/architecture.md](architecture.md)

| Term / Acronym | Definition |
| :--- | :--- |
| **ACP** | **Agent Client Protocol** — Standard stdio/RPC protocol used by Antigravity CLI (`agy acp`) and Hermes (`acp_adapter`) for structured agent orchestration. |
| **AHTL** | **Alien Holographic Token Lattice** — Infinity-Context's non-human, BPE-aligned symbolic representation (`⟪...⟫`, `⟦Δ/Ψ/Ω/Θ/Φ:...⟧`) that achieves 12x–40x token compression while remaining directly queryable by Transformer self-attention heads. |
| **Bi-Temporal Graph** | Knowledge graph model (inspired by Zep/Graphiti `arXiv:2501.13956`) tracking both *valid time* (`[t_valid, t_invalid)`) and *transaction time* (`[t_tx_start, t_tx_end)`) so superseded facts are invalidated without losing lineage. |
| **Braille-256 Sketch** | Encoding of a 64-bit topological fingerprint into 8 Unicode Braille Pattern codepoints (`U+2800..U+28FF`), embedded in the `⋮Ψ:` tail of an AHTL packet. |
| **Cache Epoch (`E_k`)** | An immutable, cryptographically sealed block of compressed historical context (`L3` DAG node) aligned to provider prompt-cache boundaries (`arXiv:2610.28577`). |
| **CAS** | **Content-Addressed Storage** — Storage engine where every chunk is addressed by its cryptographic `BLAKE3` digest (`#icx:<hex16>`). |
| **Context-Folding** | Technique (`arXiv:2510.11967`) that branches multi-step debugging/subagent trajectories and folds the intermediate loop into a single compact commit packet (`⟦Φ:...⟧`) upon completion. |
| **Ephemeral Tail Slot** | The dynamic `< 2,048` token region placed at the very end of the outbound prompt (after all cached prefixes) containing per-turn HDC/PPR/Fractal recall and cross-agent `/dev/shm` telepathy updates. |
| **FastCDC** | **Fast Content-Defined Chunking** — Rolling gear-hash chunking algorithm that splits byte streams on content-determined boundaries for deduplication. |
| **FCGA** | **Fractal Chaos-Game Archive** — Contractive Iterated Function System (`arXiv:2607.07144`) mapping discrete agent state sequences into continuous coordinates in $[-1, 1]^{16}$ where Chebyshev distance $d_{\infty}$ encodes exact shared suffix length in $O(1)$ time. |
| **HDC / HRR** | **Hyperdimensional Computing / Holographic Reduced Representations** — Algebraic memory paradigm representing symbols and bound relations as 10,240-bit quasi-orthogonal hypervectors manipulated via XOR binding ($\oplus$), permutation ($\Pi$), and Delta-Rule majority bundling. |
| **LCM** | **Lossless Context Management** — Deterministic hierarchical Merkle-DAG architecture (`arXiv:2605.04050`) guaranteeing 100% byte-exact recovery of every compacted message or tool output. |
| **PPR** | **Personalized PageRank** — Graph random-walk algorithm with restart probability $\alpha$, used in HippoRAG (`arXiv:2405.14831`) and `icx-store` for single-pass multi-hop associative retrieval. |
| **Seqlock Ring** | Lock-free concurrent ring buffer in `/dev/shm/icx-fabric-v1` using monotonic odd/even sequence counters so writers never block readers and crashed processes never deadlock the bus. |
| **Titans Surprise Gate** | Online importance and eviction gate (`arXiv:2501.00663`) combining HDC associative novelty, Shannon entropy, diagnostic anomaly boosts, and momentum smoothing ($S_t = \eta S_{t-1} + (1-\eta)g_t$). |
