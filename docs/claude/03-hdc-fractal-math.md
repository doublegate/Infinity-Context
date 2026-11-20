# 03 — HDC, Fractal IFS and "Titans" Surprise Gate (icx-hdc)

Workstream W3.

## Theory check (confirmed)

- **Geometry:** 10,240 bits = 160 × u64 = 1,280 B. For independent random hypervectors the Hamming
  distance is Binomial(10240, ½): μ = 5,120, σ = √(10240/4) = 50.596. The Hoeffding numbers in
  `docs/ahtl/04` are correct.
- **Bind and permute:** XOR binding is self-inverse and permutation is a word/bit rotation. Both
  are covered by `hv.rs` tests.
- **Majority bundling:** `HdcBundler` implements majority bundling with a deterministic
  parity tie-break (`hv.rs:673-690`), which is standard (Kanerva 2009; Plate HRR for the
  real-valued analogue).

## ICX-AUD-049 — Associative recall collapses as the corpus grows (High, claims)

- **Claim:** "sub-50 µs 10,240-bit HDC + Fractal + PPR associative index" used to fill the
  ephemeral tail slot with relevant prior artifacts.
- **Experiment** (seeded synthetic corpus):
  - Each document is a 120-token build/test log over a 20-word vocabulary, with one unique
    `needleNNNNNxHHHH` token.
  - The documents are indexed with `observe_event`.
  - Each needle is queried with `query_associative(needle, 5)`.

| Corpus | Needle position | Top-1 | Top-5 | Query latency |
| --- | --- | --- | --- | --- |
| 100 docs | random within 120 tokens | 28.0% | 47.0% | 20.1 µs |
| 1,000 docs | random within 120 tokens | **5.1%** | **5.6%** | 24.0 µs |
| 300 docs | beyond token 1,024 (1,400-token doc) | 16.3% | 22.3% | 18.7 µs |

- **Root cause** (`icx-hdc/src/lib.rs:361-430`):
  - The Stage-1 pre-screen ranks every entry with a *64-bit* sub-hypervector
    (`sub_hypervector_64`). On 64 bits the expected signal of a single matching token in a
    120-token bundle (≈0.09 × 64 ≈ 6 bits) barely exceeds the noise σ of ≈4 bits.
  - The Bloom filter term covers only the entry **subject**, not the content.
  - The quadrant and surprise terms are content-independent.
  - Only `candidate_cap = max(16, n/64)` entries survive to the exact 10,240-bit stage (16 of
    1,000).
- **Consequence:**
  - The latency SLO is met by not looking at the data. Content recall tends toward
    `candidate_cap / n` as *n* grows.
  - The project's exact fallback (`grep_cold`) is O(total store) (`cas.rs:387-422`).
  - The "lossless passkey recovery" benchmark (`icx-cli/src/main.rs:1196-1203`) derefs the
    handle returned at ingest time and runs `grep_cold`. It never exercises the associative
    index, so it cannot show that associative recall works.
- **Remediation:**
  - Pre-screen on at least 1,024 bits (a 16-word stripe costs about the same as one cache line
    per entry).
  - Or add a content token inverted index (exact rare-token postings, e.g. BM25 or trigram)
    alongside the HDC similarity.
  - Make `candidate_cap` grow with *n* (≥ √n).
  - Publish recall@k next to every latency number.
- **Regression gate:** recall@5 ≥ 90% on a 10k-document seeded corpus with single-token needles.

## ICX-AUD-050 — The bundler accumulator saturates, and encoding dominates CPU (Medium)

- **Saturation:** `accumulate_raw` adds ±w (w ≤ 64) into `i16` counters with `saturating_add`
  (`hv.rs:673-690`). With up to 1,024 tokens plus 1,024 bigrams per text, the counters saturate
  at ±32,767 on long or repetitive inputs. Once saturated, the readout stops tracking the
  majority, so long-history bundles are biased by early tokens. The novelty weighting
  (`add_delta`, `hv.rs:638-651`) slows this but does not prevent it.
- **CPU cost:** each token and bigram runs a scalar 10,240-iteration loop. `perf` attributes
  **87.5%** of a real hook's CPU time to `HdcBundler::add_delta` (see ICX-AUD-001).
- **Remediation:** use bit-sliced counters (carry-save adders over u64 words, as in popcount
  accumulation) or `i32` counters with periodic renormalization. Cache per-artifact hypervectors
  instead of re-encoding them.

## ICX-AUD-051 — The "Titans surprise gate" is a heuristic, not Titans (Low, naming)

`TitansSurpriseGate` (`surprise.rs:36-142`) combines a needle-string list, Shannon entropy and
bundler novelty: g = 0.65ν + 0.35H, η = 0.82. Titans (Behrouz et al., arXiv 2501.00663) defines
surprise as the gradient of an associative-memory loss with momentum and learned forgetting,
which involves no gradient-free string matching.

- **Remediation:** rename the gate (e.g. `NoveltyEntropyGate`), or state plainly that the name
  is an analogy. Document that the thresholds are hand-tuned.

## ICX-AUD-052 — Fractal suffix depth is inferred from distance, so false positives are possible (Low)

- **How it works:** `FractalCoord` is a 16-dimensional IFS trajectory stored as `[f64; 16]`
  (`fractal.rs:13-14`). `shared_suffix_depth` (`fractal.rs:88`) infers the length of the
  shared history suffix from the Chebyshev distance between two points. Contraction makes nearby
  points *likely* to share suffixes but does not guarantee it, so distinct histories can land
  within the threshold.
- **Benchmark gap:** benchmark SLO 3 asserts `shared_depth >= 6` on two trajectories built from
  the same six steps, which holds by construction.
- **Fixed-point:** `ref-docs/papers/INDEX.md:56` describes "128-bit fixed-point arithmetic"; the
  code is f64, and the wire form is i16 Q15/Q1.14 (see `09-docs-citations-governance.md`).
- **Remediation:** confirm candidate suffixes exactly (compare the stored symbol sequence) before
  reporting `shared_fractal_suffix`. Add a property test with random distinct histories that
  measures the false-positive rate.

## ICX-AUD-053 — `hv_digest_u64` is linear and collides trivially (Low)

The 64-bit rotate-XOR fold (`hv.rs:482`) is linear. A shipped test asserts that the all-zero and
all-ones vectors share a digest (`hv.rs:801`). The fold is used in Profile A `hv:` fields, so
equality of `hv:` must never be treated as identity.

- **Remediation:** document it as a sketch, or derive it with BLAKE3 over the words.
