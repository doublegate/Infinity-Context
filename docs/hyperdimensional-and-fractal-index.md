# Hyperdimensional (`HDC`), Fractal (`FCGA`) & Bi-Temporal Graph Index Specification

**References:** [ref-docs/novel-methods/non-human-neural-codecs-and-holographic-context.md](../ref-docs/novel-methods/non-human-neural-codecs-and-holographic-context.md) §§2–4, [ref-docs/papers/INDEX.md](../ref-docs/papers/INDEX.md) §§2–4  
**Target Crates:** `crates/icx-hdc`, `crates/icx-store`

---

## 1. Purpose

To page the right historical context into the **Ephemeral Tail Slot** within `< 250 microseconds`—without calling an external vector database or running slow neural encoders on the critical path—Infinity-Context combines three complementary mathematical indexing engines:
1. **10,240-Bit Hyperdimensional Computing (`HDC/HRR`)** in `crates/icx-hdc` for instant role-bound associative similarity search.
2. **Fractal Chaos-Game Trajectory Archives (`FCGA`)** in `crates/icx-hdc` for $O(1)$ topological state-sequence suffix matching.
3. **Bi-Temporal CSR Knowledge Graph + HippoRAG Personalized PageRank (`PPR`)** in `crates/icx-store` for multi-hop structural reasoning and temporal contradiction invalidation.

---

## 2. 10,240-Bit Hyperdimensional Computing Engine (`crates/icx-hdc`)

### 2.1 Memory Layout
```rust
/// 10,240-bit binary hypervector aligned to 64-byte cache lines / AVX-512 ZMM registers.
#[repr(C, align(64))]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Hypervector {
    pub words: [u64; 160],
}
```
- Total size: `1,280` bytes (`20` x 64-byte cache lines = `20` AVX-512 `zmm` registers).
- Orthogonal seed generation: `Hypervector::from_seed(domain: &str, token: &str)` hashes `(domain, token)` via `blake3::Hasher::finalize_xof()` and fills the `1,280` bytes directly in `< 150 nanoseconds`.

### 2.2 Algebraic Operations
1. **Bind (`bind` / `^`):** Element-wise XOR (`self.words[i] ^ other.words[i]`). Self-inverse, distance-preserving.
2. **Permute (`permute(shifts: usize)`):** Cyclic word and bit rotation $\Pi^k$, used to encode non-commutative sequences (`token_t`, `token_{t+1}`, `token_{t+2}`) and AST nesting depth.
3. **Delta-Rule Bundling (`Bundler`):**
   Maintains `i16` accumulator counters per bit position `acc: [i16; 10240]`. When adding a new hypervector $V$ to an existing bundle with current thresholded readout $M$:
   - Compute novelty weight $w = \text{clamp}\left(\frac{2 \cdot d_{\text{Hamming}}(V, M)}{10240}, \, 0.1, \, 2.0\right)$ (Infini-attention Delta Rule, `arXiv:2404.07143`).
   - Increment/decrement `acc[b]` by $\pm \text{round}(16 \cdot w)$ and threshold `acc[b] >= 0` to produce the updated `Hypervector`.
4. **Similarity (`similarity(&self, other: &Self) -> f32`):**
   $$\text{sim}(A, B) = 1.0 - \frac{2.0 \cdot \min(d_{\text{Hamming}}(A, B), 5120)}{10240} \in [0.0, 1.0]$$
   where random orthogonal vectors have $\mathbb{E}[\text{sim}] \approx 0.0$ and identical vectors have $\text{sim} = 1.0$.

---

## 3. Fractal Chaos-Game Trajectory Archive (`FCGA`)

Based on Gusev (`arXiv:2607.07144`), `FractalTrajectoryArchive` maps any sequence of discrete state hypervectors $(H_1, H_2, \dots, H_t)$ into a 16-dimensional coordinate in $[-1.0, 1.0]^{16}$ (and an exact 64-bit Morton Z-order prefix code):

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
3. **Chebyshev Suffix Matching (`shared_suffix_depth`):**
   Given two trajectory coordinates $z_a, z_b$:
   $$d_{\infty}(z_a, z_b) = \max_{j \in 0..16} |z_{a,j} - z_{b,j}|$$
   The exact number of shared trailing state transitions is:
   $$L_{\text{shared}} = \lfloor -\log_2(\max(d_{\infty}(z_a, z_b), 2^{-52})) \rfloor$$
   This allows `icx` to detect in **< 10 nanoseconds** if an agent is entering a multi-step trajectory pattern previously seen in the current or another agent's history!

---

## 4. Titans Surprise-Gated Momentum (`TitansSurpriseGate`)

Adapting Behrouz et al. (`arXiv:2501.00663`), `TitansSurpriseGate` maintains a running associative memory trace $M_{t-1}$ and scalar momentum $S_{t-1} \in [0.0, 1.0]$:
1. For each incoming event $e_t$ with hypervector $H_t$:
   - Associative novelty: $\nu_t = 1.0 - \text{sim}(H_t, M_{t-1})$
   - Diagnostic anomaly boost $\delta_t \in [0.0, 0.5]$ (triggered by error tokens, non-zero exit codes, panics, or user negations).
   - Instantaneous surprise: $g_t = \text{clamp}(0.7 \nu_t + 0.3 \rho_{\text{entropy}} + \delta_t, \, 0.0, \, 1.0)$.
2. Momentum update ($\eta = 0.82$, decay $\alpha = 0.05$):
   $$S_t = \text{clamp}(\eta S_{t-1} + (1.0 - \eta) g_t, \, 0.0, \, 1.0)$$
3. Update memory trace $M_t \leftarrow \text{Bundler::add\_weighted}(M_{t-1}, H_t, S_t)$.

---

## 5. Bi-Temporal CSR Graph & HippoRAG Personalized PageRank (`crates/icx-store`)

### 5.1 Bi-Temporal Edge Model (`arXiv:2501.13956`)
Every extracted relational edge $e = (u \xrightarrow{\text{rel}} v)$ carries:
- `t_valid: u64` (Unix epoch ms when fact became true)
- `t_invalid: Option<u64>` (`None` if currently valid; `Some(ts)` when superseded by a newer edit/refactor/test result)
- `t_tx_start: u64`, `t_tx_end: Option<u64>`
- `provenance_artifact: ArtifactId` (`#icx:<hash>`)

When a new fact `(u, rel, v_new)` contradicts an functional relation `(u, rel, v_old)` (e.g., a function's return type or a test's pass/fail status changes), `BiTemporalGraph::upsert_edge()` automatically stamps `t_invalid = Some(now)` on the old edge rather than deleting it.

### 5.2 Fast In-Memory Personalized PageRank (`arXiv:2405.14831`)
Given query seed nodes $S_Q$ (symbols/files mentioned in the current user prompt or tool call):
1. Initialize probability vector $p^{(0)}_i = \frac{1}{|S_Q|}$ for $i \in S_Q$, else $0$.
2. For $k = 1 \dots 8$ iterations (with restart probability $\alpha = 0.15$):
   $$p^{(k)} = \alpha p^{(0)} + (1 - \alpha) A_{\text{valid}}^T D^{-1} p^{(k-1)}$$
   where $A_{\text{valid}}$ includes only edges with `t_invalid.is_none()` (or `t_valid <= query_time < t_invalid` for historical time-travel queries).
3. Collect the `provenance_artifact` handles of the top-$K$ highest-scoring nodes and promote their AHTL frames into the Ephemeral Tail Slot.
