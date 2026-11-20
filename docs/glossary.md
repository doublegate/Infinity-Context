# Glossary of Terms & Abbreviations

**Applies to:** `icx` v1.1.0  
**References:** [ref-docs/research-report.md](../ref-docs/research-report.md), [docs/architecture.md](architecture.md)

| Term / Acronym | Definition |
| :--- | :--- |
| **ACP** | **Agent Client Protocol** — Standard stdio/RPC protocol used by Antigravity CLI (`agy acp`) and Hermes (`acp_adapter`) for structured agent orchestration. |
| **AHTL** | **Alien Holographic Token Lattice** — Infinity-Context's non-human, BPE-aligned symbolic representation (`⟪...⟫`, `⟦Δ/Ψ/Ω/Θ/Φ:...⟧`) that replaces bulky tool output in the context window with a structural skeleton plus a `#icx:` handle to the stored bytes. A packet is not a lossless encoding; the full text is recovered by dereferencing the handle. |
| **Bi-Temporal Graph** | Knowledge graph model (inspired by Zep/Graphiti `arXiv:2501.13956`) tracking both *valid time* (`[t_valid, t_invalid)`) and *transaction time* (`[t_tx_start, t_tx_end)`) so superseded facts are invalidated without losing lineage. |
| **Braille-256 Sketch** | Encoding of a 64-bit topological fingerprint into 8 Unicode Braille Pattern codepoints (`U+2800..U+28FF`), embedded in the `⋮Ψ:` tail of an AHTL packet. |
| **Cache Epoch (`E_k`)** | An immutable, cryptographically sealed block of compressed historical context (`L3` DAG node) aligned to provider prompt-cache boundaries (`arXiv:2610.28577`). |
| **CAS** | **Content-Addressed Storage** — Storage engine where every chunk and artifact is identified by its `BLAKE3` digest and addressed by a 64-bit handle (`#icx:<16-hex>`, or `#icx:<12-hex>` in Profile A); a handle collision is reported as `IcxError::HashCollision`. |
| **Context-Folding** | Technique (`arXiv:2510.11967`) that branches multi-step debugging/subagent trajectories and folds the intermediate loop into a single compact commit packet (`⟦Φ:...⟧`) upon completion. |
| **Ephemeral Tail Slot** | The per-turn region placed after the stable prompt prefix, containing HDC/PPR/fractal recall and cross-agent telepathy events. Budget: 1,024 tokens by default, 2,048 in the Anthropic and OpenAI presets. |
| **FastCDC** | **Fast Content-Defined Chunking** — Rolling gear-hash chunking algorithm that splits byte streams on content-determined boundaries for deduplication. `icx` uses spread (high-bit) masks as in Xia et al. (USENIX ATC 2016), with 2 KiB / 8 KiB / 64 KiB minimum / target / maximum chunk sizes. |
| **FCGA** | **Fractal Chaos-Game Archive** — Contractive Iterated Function System (`arXiv:2607.07144`) mapping discrete agent state sequences into continuous coordinates in $[-1, 1]^{16}$ where Chebyshev distance $d_{\infty}$ gives an $O(1)$ upper bound on shared suffix length; `verified_suffix_depth` checks the inverse IFS transitions to reject false matches. |
| **HDC / HRR** | **Hyperdimensional Computing / Holographic Reduced Representations** — Algebraic memory paradigm representing symbols and bound relations as 10,240-bit quasi-orthogonal hypervectors manipulated via XOR binding ($\oplus$), permutation ($\Pi$), and Delta-Rule majority bundling. |
| **LCM** | **Lossless Context Management** — Deterministic hierarchical Merkle-DAG architecture (`arXiv:2605.04050`) In `icx`, every compacted message or tool output can be recovered byte for byte as stored, which is the content after secret redaction. |
| **Owner seal** | Keyed BLAKE3 MAC of a workspace's canonical path under the per-install key, stored in the workspace's `OWNER` file. A workspace without a valid seal is refused until `icx trust` adopts it. |
| **PPR** | **Personalized PageRank** — Graph random-walk algorithm with restart probability $\alpha$, used in HippoRAG (`arXiv:2405.14831`) and `icx-store` for single-pass multi-hop associative retrieval. |
| **Seqlock Ring** | The 256-slot telepathy ring (`SeqlockTelemetryBus`). Odd/even per-slot version counters let readers detect torn or overwritten slots without locking. The in-process backend is atomic; the file-backed backend (default `icx-fabric-v1.shm` in the workspace, or `ICX_SHM_PATH`) uses positional I/O under a writer mutex and advisory file lock, and is not memory-mapped. Dropped events are reported (`dropped_publishes`, `poll_since_with_loss`). |
| **Store seal** | Keyed BLAKE3 MAC over a workspace store's canonical path, generation and a digest of all persisted state, written on every save and checked on load. |
| **Titans-inspired surprise gate** | `TitansSurpriseGate`: an importance and retention heuristic inspired by Titans (Behrouz et al., `arXiv:2501.00663`), combining HDC novelty, byte entropy, diagnostic and exit-code signals, and momentum smoothing ($S_t = \eta S_{t-1} + (1-\eta)g_t$). It is a hand-tuned heuristic, not the paper's learned neural memory with gradient-based surprise. |
| **Workspace** | Per-project private directory (default `$XDG_STATE_HOME/icx/workspaces/<16 hex>`) holding the SQLite store `icx.sqlite3`, the owner seal, the lock file and the default bus file. |
