# Phase 3 — Hyperdimensional (`HDC`), Fractal (`FCGA`) & Bi-Temporal Graph Index (COMPLETED — `v0.4.0`)

## Goal

Implement the sub-50µs **10,240-Bit Hyperdimensional Computing (`HDC/HRR`)** associative engine, **Fractal Chaos-Game Trajectory Archive (`FCGA`)**, **Titans Surprise-Gated Momentum Tracker** (`crates/icx-hdc`), and **Bi-Temporal CSR Knowledge Graph + HippoRAG Personalized PageRank (`PPR`)** (`crates/icx-store`).

## Exit Criteria

- [x] `Hypervector` (`[u64; 160]`) supports deterministic BLAKE3 XOF seeding (`from_seed`, `from_seed_framed`, `from_u64_seed`), XOR binding (`BitXor`, `BitXorAssign`, `bind`, `unbind`, `bind_role_filler`), circular permutation and inverse (`permute`, `inverse_permute`, `encode_sequence`), Infini-attention Delta-Rule majority bundling with coordinate-parity tie-breaking and 128-bit non-linear deduplication (`HdcBundler`), and two-stage L2-cache pre-screened similarity search (`HdcAssociativeIndex` / `AssociativeMemoryIndex`).
- [x] `FractalCoord` (`[-1.0, 1.0]^16`) and `FractalTrajectoryArchive` compute shared suffix depth in $O(1)$ time via Chebyshev distance (`shared_suffix_depth`), boundary-aware inverse-IFS verification (`verified_suffix_depth`), `Q15` / `Q1.14` fixed-point quantization, 64-bit Morton Z-order prefix codes (`morton_z64`), and `@o1:o2` octal quadrant addressing.
- [x] `TitansSurpriseGate` tracks surprise momentum $S_t$, supports idle exponential decay (`apply_idle_decay(steps)`), exit-code anomaly scoring (`observe_with_exit_code`), and Delta-Rule novelty gating.
- [x] `BiTemporalGraph` auto-invalidates superseded functional edges (`t_invalid`, `t_tx_end`), compiles contiguous Compressed Sparse Row (`CsrGraphSnapshot`) snapshots, and executes multi-hop Personalized PageRank (`PPR`).

## Sprints

- [Sprint 1 — 10,240-Bit HDC/HRR Engine & Infini-Attention Delta Bundler (COMPLETED)](sprint-1-hdc-10240bit-engine.md)
- [Sprint 2 — Fractal Chaos-Game Archive (`FCGA`) & Titans Surprise Gate (COMPLETED)](sprint-2-fractal-ifs-and-surprise-gate.md)
- [Sprint 3 — Bi-Temporal CSR Knowledge Graph & Personalized PageRank (`PPR`) (COMPLETED)](sprint-3-bitemporal-graph-and-ppr.md)

## Reference Docs

- [docs/hyperdimensional-and-fractal-index.md](../../docs/hyperdimensional-and-fractal-index.md)
