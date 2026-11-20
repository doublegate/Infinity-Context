# Hyperdimensional (`HDC`), Fractal (`FCGA`) & Bi-Temporal Graph Index Specification

**Applies to:** `icx` v1.1.0  
**References:** [ref-docs/novel-methods/non-human-neural-codecs-and-holographic-context.md](../ref-docs/novel-methods/non-human-neural-codecs-and-holographic-context.md) §§2–4, [ref-docs/papers/INDEX.md](../ref-docs/papers/INDEX.md) §§2–4  
**Target Crates:** `crates/icx-hdc`, `crates/icx-store`

---

## 1. Purpose

To page relevant historical context into the **Ephemeral Tail Slot** without calling an external vector database or running neural encoders on the hook path, Infinity-Context combines three indexing engines (measured query costs: [docs/performance.md](performance.md)):

1. **10,240-Bit Hyperdimensional Computing (`HDC/HRR`)** in `crates/icx-hdc` for instant role-bound associative similarity search.
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
2. **Permute (`permute(shifts: usize)` / `inverse_permute(shifts: usize)`):** Cyclic word and bit rotation $\Pi^k$ and exact inverse $\Pi^{-k}$, used to encode non-commutative sequences (`encode_sequence`) and AST nesting depth.
3. **Delta-Rule Bundling (`HdcBundler`):**
   Maintains `i32` accumulator counters per bit position `acc: [i32; 10240]` (v1.0.0 used `i16`, which saturated on long histories; ICX-AUD-050) and a 128-bit non-linear fingerprint set (`dedup_fingerprint_u128`). When adding a new hypervector $V$ to an existing bundle with current thresholded readout $M$:
   - Compute similarity $\text{sim}(V, M)$; if $\text{sim}(V, M) \ge 0.85$ or its 128-bit fingerprint is already active, skip accumulation (`w = 0`, Infini-attention Delta Rule, `arXiv:2404.07143`).
   - Otherwise increment/decrement `acc[b]` by $\pm \max(1, \text{round}(16 \cdot (1 - \text{sim}(V, M))))$ (weights clamped to `1..=64`) and threshold with deterministic coordinate-parity tie-breaking (`((word_idx ^ bit) & 1) == 0` when `acc[b] == 0`) so even-cardinality bundles stay centered at Hamming weight `5,120`.
4. **Similarity (`similarity(&self, other: &Self) -> f32` & `cosine_bipolar`):**
   $$\text{sim}(A, B) = 1.0 - \frac{\min(d_{\text{Hamming}}(A, B), 5120)}{5120} \in [0.0, 1.0]$$
   where random orthogonal vectors have $\mathbb{E}[\text{sim}] \approx 0.0$ and identical vectors have $\text{sim} = 1.0$.
5. **Two-Stage Associative Query (`HdcAssociativeIndex::query_associative`):**
   Each indexed entry has a compact `EntrySummary` (`hv_sub64`, `subj_bloom32`, `quadrant_u16`, `surprise_q8`, `flags`, and since v1.1.0 a 256-bit `content_bloom` over the entry's subject and content tokens).
   - **Stage 1** scores every entry on: similarity of the first 1,024 hypervector bits (16 words; v1.0.0 used 64 bits, which left a single matching token below the noise floor), exact query-token hits in the content Bloom filter, subject Bloom overlap, fractal quadrant similarity, surprise and the error flag. It keeps the top `candidate_cap` entries, where `candidate_cap = min(n, max(clamp(4k, 16, 64), clamp(n / 64, 16, 512), ceil(4 * sqrt(n)), k))` for `top_k = k`. The `sqrt(n)` floor makes the candidate set grow with the corpus so recall does not collapse as `n` grows. Indexes with at most 64 entries are scored exhaustively.
   - **Stage 2** runs the full 10,240-bit Hamming comparison, `verified_suffix_depth` and a content-token bonus on the candidates, and returns each artifact at most once (ICX-AUD-042).

   Measured unique-token recall on 1,000 log documents: top-1 5.1% (v1.0.0) to 99.0%, top-5 5.6% to 99.7%, with query time 24 µs to 69 µs (ICX-AUD-049). The gate test `crates/icx-hdc/tests/recall_v110.rs` requires recall@5 `>= 95%` at n = 300.

### 2.3 Persisted Snapshot (`ICXHDC03`)

The index is persisted in the workspace store as a versioned binary snapshot (`crates/icx-hdc/src/snapshot.rs`), so loading a workspace does not re-encode every stored event. The format is little-endian and length-prefixed with magic `ICXHDC03`. Every count is checked against a cap (16 Mi items, 64 KiB strings) before allocation, and truncated, oversized, non-UTF-8, inconsistent or trailing-byte input is rejected with `SnapshotError`. A fuzz target (`fuzz/fuzz_targets/hdc_snapshot.rs`) covers the decoder.

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

1. Extract 16 bipolar signs $v_j(s_t) \in \{-1.0, +1.0\}$ from the parity of the 160 `u64` words of $H(s_t)$ grouped into 16 blocks of 10 words.
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
