# Corrections to the Research Corpus (2026-11-18)

The files under `ref-docs/` are a dated research corpus and are not rewritten in place. This
file records the corrections that the v1.1.0 audit verified against the primary sources and the
code. Read it alongside the corpus: where the two disagree, this file is correct. Current
quantitative values are listed in [`docs/numbers.md`](../docs/numbers.md).

Sources of the verification: audit findings `ICX-AUD-031`, `ICX-AUD-032` and `ICX-AUD-033`
([`docs/claude/09-docs-citations-governance.md`](../docs/claude/09-docs-citations-governance.md),
[`docs/claude/01-ahtl-spec-conformance.md`](../docs/claude/01-ahtl-spec-conformance.md)). They
checked all 22 arXiv identifiers against the arXiv export API and abstract pages, the full text of
each paper, and the source code on branch `claude-audit`. All 22 identifiers resolve. Every local
PDF matches its arXiv title and authors.

Line numbers refer to the files as they stand on 2026-11-18, after the v1.1.0 removal of ten
PDFs and the pointer line added at the top of `research-report.md` and `papers/INDEX.md`. A
line number given as "audit line" is the one recorded by the audit; that file is maintained
outside `ref-docs/` and may since have been corrected in place.

## 1. Citation and Attribution Corrections (ICX-AUD-033)

### arXiv:2607.23809: wrong title and wrong description

- **Correct citation:** *ACM: Agentic Context Management for Long Horizon Tasks*, Xiaochuan Li
  et al., arXiv:2607.23809, 26 Jul 2026.
- **Content:** the paper gives an agent memory tools for summarizing, offloading and querying its
  own context. KV cache appears only in related work and as a side benefit. It does not co-design
  agent state with KV-cache block retention.

| File | Line | As written | Correction |
| :--- | :--- | :--- | :--- |
| `ref-docs/papers/INDEX.md` | 48 (heading, link target) | "2607.23809-ACM-Agentic-Cache-Management.pdf" | The title is "Agentic Context Management". The local file name repeats the wrong title; its contents are the correct paper. |
| `ref-docs/papers/INDEX.md` | 49 | *ACM: Agentic Cache Management* | *ACM: Agentic Context Management for Long Horizon Tasks* (Li et al.) |
| `ref-docs/papers/INDEX.md` | 50 | "Co-designs agent state transitions with KV-cache block retention policies, separating immutable tool/system prefixes from volatile working scratchpads." | Not in the paper. Its subject is agent-managed context (summarize, offload, query). |
| `ref-docs/papers/INDEX.md` | 51 | ACM used to order `cache_control` breakpoints "for >94% cache hit ratios" | The breakpoint placement does not derive from this paper. The current cache figure is a 95.46% byte-weighted prefix-reuse proxy on a synthetic transcript (`docs/numbers.md`). |
| `ref-docs/research-report.md` | 63 | ACM cited, with arXiv:2610.28577, for prefix-cache invalidation by mid-history edits | ACM does not support this point. Cite arXiv:2610.28577 alone. |
| `ref-docs/research-report.md` | 173 | *ACM: Agentic Cache Management* (2026) | *ACM: Agentic Context Management for Long Horizon Tasks*, Li et al. (2026) |

### arXiv:2607.07144 (Fractal KV-Cache Archives): precision and dimension

| File | Line | As written | Correction |
| :--- | :--- | :--- | :--- |
| `ref-docs/papers/INDEX.md` | 58 | Mapped "onto a multi-scale Chaos-Game coordinate in R^16 (using 128-bit fixed-point arithmetic for lossless prefix recovery up to depth 32 ...)" | The paper uses IEEE-754 double precision and 2-D points; it never mentions 128-bit fixed point or R^16. The project's own `FractalCoord` stores `coords: [f64; 16]` in memory (`crates/icx-hdc/src/fractal.rs:14`) and uses i16 Q15/Q1.14 on the wire, not 128-bit fixed point. The paper's O(1) random access and amortized O(1) append are correctly stated (line 57). |

### arXiv:2410.15704 (RVQ for KV cache): ratio, quality and attribution

| File | Line | As written | Correction |
| :--- | :--- | :--- | :--- |
| `ref-docs/papers/INDEX.md` | 62 | "5.5x–8x compression with negligible fidelity loss" | The paper reports 5.5x; no 8x figure appears. It reports that depth K = 8 recovers most of the performance and notes a significant gap on GSM8k for key quantization, which light fine-tuning reduces. "Negligible" overstates this. Non-contiguous channel grouping is correctly attributed. |
| `ref-docs/papers/INDEX.md` | 61 | *Residual Vector Quantization for KV Cache Compression*, no author | *Residual vector quantization for KV cache compression in large language model*, Ankur Kumar (single author), Oct 2024. |

### arXiv:2605.04050 (LCM)

| File | Line | As written | Correction |
| :--- | :--- | :--- | :--- |
| `ref-docs/papers/INDEX.md` | 15 | "May 2026" | The arXiv record and the PDF stamp give 14 Feb 2026 (Ehrlich and Blackman, Voltropy PBC), despite the 2605 prefix. |
| `ref-docs/papers/INDEX.md` | 16 | "lossless cryptographic pointers" | The paper describes lossless pointers from summary nodes to the raw messages. It says nothing about hashes or cryptography; "cryptographic" is the project's own design (BLAKE3), not the paper's. The DAG, LLM-Map and OOLONG 32K to 1M results are correctly attributed. |

### Minor bibliographic corrections

| File | Line | As written | Correction |
| :--- | :--- | :--- | :--- |
| `ref-docs/papers/INDEX.md` | 76 | *In-Context Autoencoder (ICAE) for Context Compression* | *In-context Autoencoder for Context Compression in a Large Language Model*, Ge et al., ICLR 2024 (`research-report.md` line 193 has it correctly). |
| `ref-docs/papers/INDEX.md` | 85 | Titans, "Jan 2025" | Submitted 31 Dec 2024 (Behrouz, Zhong, Mirrokni). |

### Titans naming (ICX-AUD-051)

| File | Line | As written | Correction |
| :--- | :--- | :--- | :--- |
| `ref-docs/papers/INDEX.md` | 88 | Titans "Directly implemented in `crates/icx-hdc/src/surprise.rs` (`TitansSurpriseGate`)" | `TitansSurpriseGate` is a heuristic inspired by Titans: momentum-smoothed novelty from hypervector distance plus diagnostic and exit-code signals. It is not the paper's learned neural memory with gradient-based surprise. Describe it as "Titans-inspired". |

### Unsourced figures in the corpus

| File | Line | As written | Correction |
| :--- | :--- | :--- | :--- |
| `ref-docs/research-report.md` | 78 | "Standard prose uses ~4.5–6.0 bits/token out of a BPE token's ~17.5-bit theoretical entropy." | No paper in the corpus states this. It is the project's own estimate. |
| `ref-docs/novel-methods/non-human-neural-codecs-and-holographic-context.md` | 21 | "~1.1 to 1.5 bits/character (~4.5 to 6.0 bits/token)" | This line derives the figure from a Shannon-style per-character estimate multiplied by about 4 characters per token. The derivation is plausible, but no source is given; label it as an estimate. |
| `ref-docs/prior-art/github-and-industry-landscape.md` | 51 | "ai-memory" cited by a local filesystem path | It is not a public repository and so not citable prior art; the other 14 named repositories exist, are not archived, and match their descriptions. |

### Same findings in files outside `ref-docs/`

These files are not part of the immutable corpus. The audit recorded the following, and the files
are corrected in place by their maintainers; they are listed here so that the citation record is
complete.

| File | Audit line | As written | Correction |
| :--- | :--- | :--- | :--- |
| `docs/adr/0003-prefix-cache-epoch-pinning.md` | 9 | arXiv:2610.28577 cited for "up to 10x" input cost and "4x–8x" TTFT | The paper reports a 27.7% average cost reduction, 1.2x output throughput and 1.5x P95 latency; KV reads are 82.4% of API cost; its Table 6 cache-miss/hit price ratios are 2x to 5x. Neither 10x nor 4x–8x TTFT appears. |
| `docs/ahtl/07-transformer-attention-and-bpe-mechanics.md` | 17 | "19.0%" passkey/line recall after summarization | In none of the 22 papers. The only other occurrence is a hard-coded value in `docs/ahtl/generate_ahtl_master_diagrams.py` (audit lines 1163, 1245) and the SVG it generates; no measurement lies behind it. |
| `docs/adr/0002-alien-holographic-token-lattice.md` | 9 | "~4.5–6.0 bits/token" | Unsourced estimate, as above. |

## 2. Arithmetic Correction (ICX-AUD-032)

| File | Line | As written | Correction |
| :--- | :--- | :--- | :--- |
| `ref-docs/ahtl-exactness-review-and-analysis.md` | 82 | 48-bit Profile A "reaches 50% at the birthday bound N_0.5 ≈ 1.18 × 10^7 (≈ 2^24.5)" | N_0.5 = sqrt(2 ln 2 · 2^48) = 1.1774 · 2^24 ≈ 1.98 × 10^7. The two figures given are also inconsistent with each other: 2^24.5 ≈ 2.37 × 10^7. The probabilities on the same line (1.77 × 10^-7 at N = 10^4, 1.77 × 10^-5 at N = 10^5) are consistent with the formula. |
| `ref-docs/ahtl-exactness-review-and-analysis.md` | 95 and 193 | Line 95 says clause keys and values are not percent-escaped; line 193 marks round-trip escaping (`escape_clause_key`, `escape_clause_val`) as ENFORCED | The review contradicts itself. The audit also found that items raised in sections 2.3.4 and 2.4 have no recorded resolution but sit under the blanket "ENFORCED". Treat the line 193 verdict as applying only to what the cited code shows. |
| `ref-docs/ahtl-exactness-review-and-analysis.md` | 196 | "ENFORCED (`97.5%+` LINE COV)" | The audit measured 95.75% line coverage (95.78% production-only) at commit `c176d03`. The enforced gate is 95% (`.github/workflows/ci.yml:105`). |

## 3. Cross-Document Numeric Contradictions (ICX-AUD-031)

The audit found the same quantity stated differently across `docs/`, `docs/ahtl/`, `ref-docs/`,
README, CHANGELOG and ROADMAP. The canonical v1.1.0 value is given for each. Only the `ref-docs/`
locations are listed by line; the other locations are recorded in `ICX-AUD-031`.

| Quantity | `ref-docs/` statements | Canonical v1.1.0 value |
| :--- | :--- | :--- |
| Epoch rollover threshold | `research-report.md:164`: "default `16,384` tokens per sealed epoch" | `EpochControllerConfig::default()`, used by the MMU: 4,096 tokens, 4 protected turns, 1,024-token tail slot (`crates/icx-core/src/epoch.rs:21-29`). The presets `anthropic_default()` (16,384 / 6 / 2,048) and `openai_default()` (8,192 / 6 / 2,048) exist but are not the runtime default. |
| Protected tail turns | none in `ref-docs/` | 4 (default); 6 in the provider presets |
| Tail slot size | none in `ref-docs/` | 1,024 tokens (default); 2,048 in the provider presets |
| Anthropic breakpoint layout | none in `ref-docs/` | Client breakpoints are kept; icx adds at most one, on the last content block of the folded history, only while the total is below 4 (`crates/icx-proxy/src/wire.rs:109-148`). |
| Compression ratio | `research-report.md:17` and `papers/INDEX.md:68`: "12x–40x" | Input-dependent. Benchmark: 214.0x on a repetitive 1,200-line test log, 159.8x on a varied log. The audit measured 34.4x to 55.2x on three real inputs at `c176d03`. A packet is a skeleton plus a handle, not a lossless encoding. |
| HDC Hamming scan | `research-report.md:109`: "< 50 µs query" | 64.20 µs per 1,000 nodes, 642.05 µs per 10,000 nodes (synthetic benchmark). |
| AVX-512 Hamming search | `research-report.md:17, 36, 93, 109`; `novel-methods/...:81-82`; `prior-art/...:18, 66` | The benchmark host has AVX2 and POPCNT, no AVX-512; no AVX-512 figure has been measured. |
| Seqlock publish and propagation | `research-report.md:17, 41`: "sub-100-microsecond", "< 100 µs" | In-process publish 58.3 ns per slot; file-backed publish-to-visible worst 38 µs over 200 events. |
| Shm transport | `research-report.md:122` (`mmap`), `:156` (`memmap2` / `libc`, zero-copy), `:17, :41` (lock-free) | Positional I/O (`pread`/`pwrite`) under a writer mutex and a file lock. Not memory-mapped, not lock-free, not zero-copy. Only the in-process ring uses atomics. `forbid(unsafe_code)` excludes `mmap` in this codebase. |
| Shm path | `research-report.md:41, 122`: `/dev/shm/icx-fabric-v1` | `ICX_SHM_PATH` if set, else `icx-fabric-v1.shm` inside the workspace directory. It is under `/dev/shm` only if configured there. |
| Hook budget | `research-report.md:52`: "`< 500 µs` p99 via `/dev/shm` + UDS with `icxd`"; `:122`: "`< 500 µs`" | Benchmark: 244.05 µs for one in-process call on a fresh MMU. Real workspaces: 14 ms in-process at 338 artifacts (940 ms in v1.0.0), 69–72 ms at about 2,000 artifacts, median 3.1 ms via `icx daemon` over a Unix stream socket. |
| Prefix-cache figure | `research-report.md:17, 53, 108`: ">= 94% hit rate" | 95.46% byte-weighted prefix reuse over all turns (95.38% excluding seal turns) on a synthetic 50-turn transcript. This is a byte proxy, not a measured provider cache hit rate. |
| CAS chunk storage | `research-report.md:32, 84, 106, 152`; `papers/INDEX.md:37`: Zstd-compressed chunks | Raw bytes in SQLite; the workspace has no zstd dependency. |
| MCP tool | `research-report.md:43`: "`< 180 token`" tool with ops `deref`, `slice`, `grep_cold`, `ast_query`, `fold_branch`, `xagent_broadcast`, `map_partition` | One tool `icx` with ops `deref`, `slice`, `grep`, `recall`, `fold`, `expand`, `xagent`, `status` (`crates/icx-adapters/src/mcp.rs:37`); schema under 180 tokens by a characters/4 estimate (test-asserted). |
| PPR iterations | `ahtl-exactness-review-and-analysis.md:163`: 6 | 6 for tail-slot recall (`crates/icx-adapters/src/mmu.rs:848`); 8 in the benchmark (`crates/icx-cli/src/main.rs:1429`). Documents that say 8 for runtime recall are wrong. |
| Page-fault latency | `papers/INDEX.md:37`: "`< 500 µs` via lifecycle hooks" | See "Hook budget" above. |
| Coverage | `ahtl-exactness-review-and-analysis.md:196`: "97.5%+" | Gate 95% lines; 95.75% measured at `c176d03`. |
| Fractal precision | `papers/INDEX.md:58`: 128-bit fixed point | `[f64; 16]` in memory, i16 Q15/Q1.14 on the wire. |

Quantities in `ICX-AUD-031` that have no statement in `ref-docs/`, with their canonical values:
redactor tiers (4; entropy gate 4.35 bits/char), corner-vertex projection (popcount majority per
block), DAG tier names (`L1` turn/tool, `L2` branch, `L3` epoch, `L4` fold), `AhtlWirePacketV1`
fields (the set in `crates/icx-codec/src/ahtl.rs`), and sprint count (15).

`ref-docs/initial-concept.md` is the pre-implementation concept. Its targets (AVX-512, 10,000-bit
vectors, Zstd dictionaries, a memory-mapped lock-free bus, `rkyv` zero-copy) describe the original
plan, not v1.1.0.

## 4. Unverified

The audit did not verify the following. They are neither confirmed nor refuted here.

- `ref-docs/research-report.md:13`: naive mid-prompt editing "multiplies API latency and cost by up
  to 10x". No source is given. The audit suggested the 10x may come from Anthropic's cache-read
  price (0.1x of the base input price), but did not confirm this. The latency part has no source.
- `ref-docs/prior-art/github-and-industry-landscape.md:28`: that `ranxianglei/billion-context` "relies on slow, lossy LLM-generated English summaries". The
  repository exists and describes itself as a context-compression plugin with a proxy mode; the
  quoted characterization was not checked.
- The non-arXiv references in `docs/ahtl/` (Kanerva 2009, Plate 1995, Hutchinson 1981, Barnsley
  1988, Jeffrey 1990, Vaswani 2017, Olsson 2022, Xiao 2024) are well-known works with the stated
  years. Their full bibliographic details were not looked up.
- That each Braille-pattern glyph is a single BPE token in Anthropic tokenizers. Checking this
  requires the token-counting API with a key, and it was not run.
