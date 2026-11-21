# Hyperdimensional (`HDC`), Fractal (`FCGA`) & Bi-Temporal Graph Index Specification

**Applies to:** `icx` v1.2.0 candidate

**References:** [ref-docs/novel-methods/non-human-neural-codecs-and-holographic-context.md](../ref-docs/novel-methods/non-human-neural-codecs-and-holographic-context.md) §§2–4, [ref-docs/papers/INDEX.md](../ref-docs/papers/INDEX.md) §§2–4  
**Target Crates:** `crates/icx-hdc`, `crates/icx-store`

---

## 1. Purpose

To page relevant historical context into the **Ephemeral Tail Slot** without calling an external vector database or running neural encoders on the hook path, Infinity-Context combines three indexing engines (measured query costs: [docs/performance.md](performance.md)):

1. **10,240-Bit Hyperdimensional Computing (`HDC/HRR`)** in `crates/icx-hdc` for local role-bound associative similarity search.
2. **Fractal Chaos-Game Trajectory Archives (`FCGA`)** in `crates/icx-hdc` for $O(1)$ state-sequence suffix comparison.
3. **Bi-Temporal CSR Knowledge Graph + HippoRAG Personalized PageRank (`PPR`)** in `crates/icx-store` for multi-hop structural reasoning and temporal contradiction invalidation.

---

## 2. 10,240-Bit Hyperdimensional Computing Engine (`crates/icx-hdc`)

### 2.1 Memory Layout

```rust
/// 10,240-bit binary hypervector aligned to 64-byte cache lines.
#[repr(C, align(64))]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Hypervector {
    pub words: [u64; 160],
}
```

- Total size: `1,280` bytes (`20` x 64-byte cache lines).
- Seed generation: `Hypervector::from_seed(domain: &str, symbol: &str)` hashes a domain-separated `(domain, symbol)` string with BLAKE3 and fills the `1,280` bytes from the extendable output (`finalize_xof()`).
- Hamming distance (`hamming_distance_simd`) is a 4-way unrolled `u64::count_ones()` loop over the 160 words. The compiler emits `POPCNT` on x86-64; no AVX-512 code path is required or assumed (the benchmark machine has no AVX-512).

### 2.2 Algebraic Operations & Two-Stage Associative Index

1. **Bind (`bind` / `unbind` / `^` / `^=`):** Element-wise XOR (`self.words[i] ^ other.words[i]`). Self-inverse, involutory, and Hamming-distance preserving (`BitXor`, `BitXorAssign`, `bind_role_filler`).
2. **Permute (`permute(shifts: usize)` / `inverse_permute(shifts: usize)`):** A deterministic
   family of cyclic word/bit rotations and their exact inverses, used for non-commutative
   sequences (`encode_sequence`) and AST nesting depth. The implemented shift 0 rotates by one
   word; it is not an assertion that this API is the mathematical power family with identity
   at exponent 0.
3. **Delta-Rule Bundling (`HdcBundler`):**
   Maintains `i32` accumulator counters per bit position `acc: [i32; 10240]` (v1.0.0 used `i16`, which saturated on long histories; ICX-AUD-050) and a 128-bit non-linear fingerprint set (`dedup_fingerprint_u128`). When adding a new hypervector $V$ to an existing bundle with current thresholded readout $M$:
   - Compute similarity $\text{sim}(V, M)$; if $\text{sim}(V, M) \ge 0.85$ or its 128-bit fingerprint is already active, skip accumulation (`w = 0`, Infini-attention Delta Rule, `arXiv:2404.07143`).
   - Otherwise increment/decrement `acc[b]` by $\pm \max(1, \text{round}(16 \cdot (1 - \text{sim}(V, M))))$ (weights clamped to `1..=64`) and threshold with deterministic coordinate-parity tie-breaking (`((word_idx ^ bit) & 1) == 0` when `acc[b] == 0`) so even-cardinality bundles stay centered at Hamming weight `5,120`.
4. **Similarity (`similarity(&self, other: &Self) -> f32` & `cosine_bipolar`):**
   $$\text{sim}(A, B) = 1.0 - \frac{\min(d_{\text{Hamming}}(A, B), 5120)}{5120} \in [0.0, 1.0]$$
   where random orthogonal vectors have $\mathbb{E}[\text{sim}] \approx 0.0$ and identical vectors have $\text{sim} = 1.0$.
5. **Hybrid Lexical-First Associative Query (`HdcAssociativeIndex::query_associative`):**
   Each indexed entry has a compact `EntrySummary` (`hv_sub64`, `subj_bloom32`, `quadrant_u16`, `surprise_q8`, `flags`, and since v1.1.0 a 256-bit `content_bloom` over the entry's subject and content tokens).
   - **Exact candidate path:** normalized subject/content tokens map to entry postings. Every
     entry matching a query token joins the candidate set independently of approximate scores.
     Bloom matches are probabilistic and must not be described as exact token hits.
     The shared lookup tokenizer preserves alphanumerics and `_`, `:`, `/`, drops tokens shorter
     than two UTF-8 bytes, and applies ASCII lowercasing; it does not provide full Unicode case folding.
   - **Approximate Stage 1** scores every entry on similarity of the first 1,024 hypervector bits,
     content/subject Bloom overlap, fractal quadrant similarity, surprise and the error flag.
     It keeps `candidate_cap = min(n, max(clamp(4k, 16, 64), clamp(n / 64, 16, 512),
     ceil(4 * sqrt(n)), k))` candidates for `top_k = k`, then unions the exact postings.
     This growth heuristic alone did not prevent the audit's varied-corpus recall failures.
   - **Stage 2** refines the union with full 10,240-bit Hamming comparison and
     `verified_suffix_depth`. Final ordering is exact query-token coverage first, combined
     HDC/fractal/surprise score second, then artifact ID for determinism. Each artifact appears
     at most once. Common-token queries can include/refine the whole corpus; this is a quality
     tradeoff, not a sub-50 µs latency guarantee. Legacy indexes without postings refine exhaustively.

   The earlier v1.1.0 1,000-log figures (99.0% top-1, 99.7% top-5) were fixture-specific.
   The v1.2.0 replay reports recall@1/@5 of 100% on the 27 frozen synthetic audit rows
   (100/1k/10k artifacts, three seeds, repetitive/varied/multilingual). See
   [retrieval evidence](releases/v1.2.0/evidence/retrieval-results.json). This does not establish
   general semantic recall, model effectiveness or an isolated latency bound; timings shared
   the host with other validation work.

### 2.3 Persisted Snapshot (`ICXHDC04`, legacy `03` readable)

The little-endian, length-prefixed `ICXHDC04` snapshot persists entries, summaries, trajectory,
surprise state and optional exact token postings. Counts are capped (16 Mi items, 64 KiB strings).
Decoding rejects truncation, invalid UTF-8, trailing bytes, nonfinite/out-of-range coordinates or
surprise parameters, inconsistent cached summaries/bundler readout, and invalid posting indices
or content-Bloom caches. This validates internal consistency, not the source provenance of an
arbitrary unsealed snapshot; workspace MAC checks remain separate.

Legacy `ICXHDC03` remains readable. Because 03 lacks exact content tokens, the MMU replays its
persisted CAS event log to rebuild the index before taking the new fast path. A standalone
legacy index can still answer queries through exhaustive refinement. Saving writes 04, which
older binaries cannot read; keep a pre-upgrade workspace backup for downgrade. CAS handles and
unversioned Q15 coordinate interpretation are unchanged. The decoder fuzz target and an
independently generated 03 fixture exercise this compatibility boundary.

---

## 3. Fractal Chaos-Game Trajectory Archive (`FCGA`)

Based on Gusev (`arXiv:2607.07144`), `FractalTrajectoryArchive` maps any sequence of discrete state hypervectors $(H_1, H_2, \dots, H_t)$ into a 16-dimensional coordinate in $[-1.0, 1.0]^{16}$ (`Q15` / `Q1.14` fixed-point, `@o1:o2` octal quadrant address, and 64-bit Morton Z-order prefix code `morton_z64`):

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FractalCoord {
    /// 16-dimensional contractive IFS state in [-1.0, 1.0]^16
    pub coords: [f64; 16],
    /// Step depth encoded
    pub depth: u32,
}
```

At each transition $s_t$:

1. Extract 16 bipolar signs $v_j(s_t) \in \{-1.0, +1.0\}$ by majority over each block of
   10 hypervector words (640 bits): at least 320 set bits selects +1, otherwise −1. This is a
   lossy projection, not parity and not a lossless sequence encoding.
2. Apply the contractive affine map ($\alpha = 0.5$):
   $$\text{coords}[j] \leftarrow 0.5 \cdot \text{coords}[j] + 0.5 \cdot v_j(s_t) \quad \forall j \in 0..16$$
3. **Chebyshev & Inverse-IFS Verified Suffix Matching (`shared_suffix_depth` / `verified_suffix_depth`):**
   Given two trajectory coordinates $z_a, z_b$:
   $$d_{\infty}(z_a, z_b) = \max_{j \in 0..16} |z_{a,j} - z_{b,j}| \le 2^{1-K}, \quad d_2(z_a, z_b) \le 4 d_{\infty}(z_a, z_b) \le 2^{3-K}$$
   The upper-bound shared suffix depth is:
   $$L_{\text{shared}} = \min\left(\lfloor -\log_2(\max(d_{\infty}(z_a, z_b) / 2, 2^{-32})) \rfloor, \, \text{depth}_a, \, \text{depth}_b, \, 32\right)$$
   This is an upper bound inferred from distance and can over-estimate (ICX-AUD-052). `verified_suffix_depth` verifies boundary-compatible inverse IFS transitions ($2 z_j \mp 1$) across all 16 dimensions so opposite-orthant endpoints are rejected while zero-boundary matches are preserved.

---

## 4. Titans-Inspired Surprise Gate (`TitansSurpriseGate`)

The gate is inspired by Titans (Behrouz et al., `arXiv:2501.00663`). It is a hand-tuned heuristic (momentum-smoothed novelty from hypervector distance plus entropy and diagnostic/exit-code signals), not the paper's learned neural memory with gradient-based surprise. `TitansSurpriseGate` maintains a running associative memory trace $M_{t-1}$ and scalar momentum $S_{t-1} \in [0.0, 1.0]$:

1. For each incoming event $e_t$ with hypervector $H_t$:
   - Associative novelty: $\nu_t = 1.0 - \text{sim}(H_t, M_{t-1})$
   - Normalized byte entropy $\rho_{\text{entropy}} \in [0, 1]$ (Shannon byte entropy divided by 8).
   - Diagnostic anomaly boost $\delta_t \in \{0.0, 0.45\}$ (triggered by error tokens, non-zero process `exit_code`, panics, or `force_anomaly`).
   - Instantaneous surprise: $g_t = \text{clamp}(0.65 \nu_t + 0.35 \rho_{\text{entropy}} + \delta_t, \, 0.0, \, 1.0)$.
2. Momentum update ($\eta = 0.82$, idle forgetting rate $\alpha = 0.05$ via `apply_idle_decay(steps)`):
   $$S_t = \text{clamp}(\eta S_{t-1} + (1.0 - \eta) g_t, \, 0.0, \, 1.0)$$
3. Update memory trace $M_t$ using Delta-Rule novelty gating (`is_anomaly` with $\nu_t \ge 0.05$, or routine with $\nu_t \ge 0.15$) so repetitive boilerplate never saturates $M_t$.

---

## 5. Bi-Temporal CSR Graph & HippoRAG Personalized PageRank (`crates/icx-store`)

### 5.1 Bi-Temporal Edge Model (`arXiv:2501.13956`)

Every extracted relational edge $e = (u \xrightarrow{\text{rel}} v)$ carries:

- `t_valid: u64` (Unix epoch ms when fact became true)
- `t_invalid: Option<u64>` (`None` if currently valid; `Some(ts)` when superseded by a newer edit/refactor/test result or explicit `invalidate_edge`)
- `t_tx_start: u64`, `t_tx_end: Option<u64>`
- `provenance: ArtifactId` (`#icx:<hash>`)

When a new fact `(u, rel, v_new)` updates a functional relation `(u, rel)` (`is_functional == true`), `BiTemporalGraph::upsert_edge()` automatically stamps `t_invalid = Some(now_ms)` and `t_tx_end = Some(now_ms)` on any prior active edge with the same `(u, rel)` rather than deleting it, guaranteeing at most one active edge per functional `(u, rel)` pair.

### 5.2 Fast In-Memory CSR Personalized PageRank (`arXiv:2405.14831`)

`BiTemporalGraph::build_csr(at_ms)` compiles edges valid at `at_ms` into a contiguous `CsrGraphSnapshot` (`nodes`, `row_ptr`, `col_idx`, `weights`, `inv_out_weight`, `prov_row_ptr`, `prov_ids`). Given query seed nodes $S_Q$:

1. Initialize probability vector $p^{(0)}_i = \frac{1}{|S_Q|}$ for $i \in S_Q$, else $0$.
2. For $k = 1 \dots K$ iterations (the MMU uses $K = 6$) (with restart probability $\alpha = 0.15$ and $O(V + E)$ scalar dangling-mass redistribution):
   $$p^{(k)} = (\alpha + (1 - \alpha) m_{\text{dangling}}) p^{(0)} + (1 - \alpha) A_{\text{valid}}^T D^{-1} p^{(k-1)}$$
   where $A_{\text{valid}}$ includes only edges active at `at_ms`.
3. Collect the `provenance_artifacts` handles of the top-$K$ highest-scoring nodes and promote their AHTL frames into the Ephemeral Tail Slot.
