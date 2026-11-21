# Mathematics, graph semantics and retrieval scaling

Revision: `a996d59f1eb468e7c386747e15f4c7e58d08410e`. Sources: all production modules of `icx-hdc`, `icx-store`, relevant codec modules and tests; AHTL chapters and foundational research notes. Detailed read coverage and limits are in [the coverage note](evidence/codec-coverage.md). No production optimization was implemented.

## Independently checked mathematical contracts

- **Layout:** 160×64=10,240 bits=1,280 bytes. `repr(C,align(64))` does not change this multiple-of-64 size. Random independent-bit Hamming distance has mean5120 and standard deviation sqrt(2560)=50.59644. Under the explicit random-oracle assumption, the one-sided Hoeffding expression exp(−5120τ²) follows by substituting a deviation of τD/2. This does not prove retrieval recall for bundled text, where bits and vectors are correlated.
- **XOR:** `(A xor R) xor R=A`, and binding both vectors with R preserves Hamming distance. Each implemented fixed-shift word/bit permutation is bijective and has the provided inverse. `permute(0)` rotates each word by1; it is not the identity or a power-family homomorphism. Chapter04 explains this; other documents' Π^k notation should not be treated as such a contract. Indexed permutations repeat after lcm(160,63)=10,080 positions, so unlimited collision-free ordering is not provided.
- **Spectral projection:** 512 disjoint 20-bit blocks each influence one majority output bit, yielding the unnormalized Hamming 1-Lipschitz bound. It says nothing about a lower bound or injectivity; many dissimilar inputs share the same sketch. Rotational-XOR `hv_digest_u64` is expressly noncryptographic. The dedup128 fingerprint is stronger than that linear sketch but finite and noncryptographic; its comment claiming collision-free deduplication cannot literally hold for 2^10240 possible inputs.
- **IFS:** The affine map halves both L∞ and L2 distance. A common suffix of K **projected corners** bounds L∞ distance by 2^(1−K), L2 by 2^(3−K). The converse does not follow. The harness obtains `shared_suffix_depth=1` for coordinates all−0.5 versus all+0.5, while `verified_suffix_depth=0`. The inverse check usefully rejects that case but cannot recover original state identity from a lossy16-bit corner projection. Two distinct vectors differing by one bit can have the same corner, then identical coordinates. Finite float/quantized storage adds a depth horizon.
- **Bundling:** i32 counters remove the prior i16 saturation at512 weight64 votes; current regression covers600 versus520 and601 opposing votes. Saturating i32 remains finite, not an unlimited exact majority counter. Scalar per-bit work remains; a new performance conclusion needs profiling rather than assuming the old87.5% profile is current.
- **Surprise:** Current implementation is hand-tuned novelty/byte-entropy/diagnostic/exit-code gating with momentum and idle decay. The primary [Titans paper](https://arxiv.org/abs/2501.00663v1) describes learned memory; [Infini-attention v2](https://arxiv.org/abs/2404.07143v2) describes compressive memory inside Transformer attention. Their model results do not validate this application's thresholds. The current subsystem document appropriately says Titans-inspired; older chapters/code labels should be read as analogy.

## ICX-CODEX-003 — HDC snapshots accept nonfinite and inconsistent cached state

- **Category:** robustness / validation defect. **Severity:** Low. **Confidence:** High. **Revision:** above.
- **Location:** `icx-hdc/src/lib.rs::decode_snapshot`, `snapshot.rs::Reader::{f32,f64}`, `fractal.rs::read_snapshot`, `surprise.rs::read_snapshot`, `hv.rs::HdcBundler::read_snapshot`.
- **Prerequisites:** A caller supplies crafted snapshot bytes to the public decoder, or authenticated/caller-created state becomes internally inconsistent. The normal workspace seal may prevent an outside attacker from replacing an authenticated snapshot; this report does not claim a seal bypass.
- **Proof:** Encode a one-entry index with subject `x`; replace4 bytes at offset1445 (8magic+8count+8id+8subjectLen+1subject+1280HV+128coords+4depth) with f32 NaN. Decode returns `Ok`. The decoded surprise is used in `combined_score`; NaN reaches `partial_cmp`, which falls back to Equal. Decoder also trusts serialized summaries/Bloom filters/readout separately from their underlying state instead of deriving/checking them.
- **Impact:** Corrupted/crafted state can be admitted and silently alter ranking or future gating. This contradicts the broader claim that internally inconsistent snapshots are rejected, although length/truncation checks do work.
- **Counterevidence:** Magic, collection bounds, string size, UTF-8, truncation and trailing bytes are checked; vector initial capacities are capped at4096 instead of blindly allocating the advertised16Mi items. Normal encoded snapshots round-trip. No memory-safety or unauthenticated workspace exploit is demonstrated.
- **Recommendation:** Validate finite/bounded coordinates, surprise, momentum, eta and alpha; verify/derive cached summaries and bundler readout; reject archive/index inconsistencies. Keep current format compatibility for valid records and document explicit error behavior for formerly accepted corrupt ones.
- **Acceptance:** Mutate every serialized numeric scalar to NaN/±infinity/out-of-range values and cached fields to contradictory data, expecting `SnapshotError::Inconsistent`. Verify a valid snapshot preserves queries and future observations byte-for-byte. Include authenticated-store recovery behavior separately.
- **Prior:** Related ICX-AUD-001 snapshot optimization and ICX-AUD-039 test quality; new validation gap, not a regression established against an earlier revision.

## ICX-CODEX-004 — Recall remediation fails on varied-content corpus growth

- **Category:** retrieval correctness / unsupported generalization. **Severity:** High for the advertised context-retrieval function. **Confidence:** High for this fixture; general deployment recall is unmeasured. **Revision:** above.
- **Location:** `icx-hdc/src/lib.rs`, `content_hits`, `score_entry`, `query_associative`; `crates/icx-hdc/tests/recall_v110.rs`; `docs/hyperdimensional-and-fractal-index.md` §2.2.
- **Prerequisites:** Ordinary120-token artifacts with varied vocabulary and a rare identifier in content, queried by that identifier. No malicious input or credentials needed.
- **Proof:** [Full results](evidence/math-recall.csv) use sizes100/1000/10000, seeds1/17/97, repetitive20-word and varied100k-word vocabularies. Subjects omit needles;100 evenly spaced needles queried per dataset. Exact-token lexical search returns the correct document for all cases. Requesting allN production hits forces exhaustive composite scoring and substantially improves recall, isolating candidate loss from scoring failure.
- **Independent recheck:** The root auditor rebuilt a separate standalone harness and reproduced the varied10k seed1 results exactly: @1=.16, @5=.33, exhaustive@5=.97, lexical@1=1.0. See [recheck procedure](evidence/recheck-retrieval.py) and [results](evidence/retrieval-recheck.json). Shared-host timing is excluded from the correctness comparison.

| Corpus size | Repetitive recall@1 / @5 (range, three seeds) | Varied recall@1 / @5 | Varied exhaustive composite @5 | Lexical @1 |
| --- | --- | --- | --- | --- |
| 100 | 100% /100% | 40–55% /94–97% | 96–97% |100% |
| 1,000 | 100% /100% | 29–39% /58–68% | 98–99% |100% |
| 10,000 | 99–100% /99–100% | 13–20% /28–36% | 97–99% |100% |

A further [nine-row multilingual extension](evidence/math-recall-multilingual.csv) uses Chinese, Cyrillic, Greek, Japanese, Arabic and French word prefixes with Chinese-prefixed unique needles and the same three sizes/seeds/baselines. At10k, recall@1 is15–19%, recall@5 is25–38%, exhaustive composite@5 is96–97%, and lexical@1 is100%. At1k multilingual@5 is62–72%; at100 it is91–96%. These synthetic strings test Unicode tokenization/routing and distractor growth; they are not a natural-language semantic retrieval evaluation.

- **Mechanism:** The256-bit content Bloom filter stores ~124 subject/content tokens in the varied corpus. Its expected false-positive rate is roughly `(1-exp(-3m/256))^3`, about45% at m=124; query membership is therefore not an exact token hit. False positives receive the same1200 coarse bonus as real matches. Only about400 of10k entries survive at k5; the final stage again gives unverified Bloom matches up to0.5 extra similarity. Both prefilter and score can misrank. The independent formula is an approximation because hash positions/words are not perfectly independent; empirical recall is the evidence.
- **Impact:** Correct artifacts remain byte-exact in CAS yet are frequently absent from recall. A passing300-document20-word regression cannot justify a general claim that corpus-growth recall collapse is fixed.
- **Counterevidence:** The remediation greatly improves repetitive logs; those results reproduce at10k and three seeds. HDC-only ranking is not expected to guarantee arbitrary lexical lookup. Exact `grep` remains available, and a user retaining the handle can dereference directly. These are valuable fallbacks but do not satisfy automatic recall's stated purpose.
- **Recommendation:** Add exact rare-token postings or confirm Bloom candidates against stored terms; distinguish lexical and HDC evidence in scores. Measure recall alongside latency, with varied content, identifiers after the encoder cap, Unicode, distractors and multi-term queries. Increase candidate recall only with measured memory/latency tradeoffs.
- **Acceptance:** Pin this deterministic fixture and publish @1/@5 at all three sizes/seeds versus lexical and exhaustive baselines. Set an explicit product threshold before claiming remediation; the prior audit's suggested≥90%@5 at10k is not met here. Validate prompt-tail integration separately because this harness tests the index, not live model effectiveness.
- **Prior:** ICX-AUD-049 **partially fixed**, rather than fully fixed. ICX-AUD-042 per-artifact dedup is implemented after ranking, but repeated entries can still consume candidate capacity before dedup; that latter lead was not independently reproduced and remains an enhancement experiment.

## Measured costs and optimization leads

The matrix ran a portable release build (no explicit target-cpu=native flag) concurrently with other audit jobs. p50 production-query time ranged20.9–31.9µs at100 entries,110.6–3005.3µs at1k, and2893.1–4266.6µs at10k. These are observations under contention, not independent SLO certification. Use the raw CSV for p95 values. No10-run warmup/native comparison was performed in this workstream.

Serialized index sizes:208,622 bytes at100;1,705,322 at1k;16,681,322 at10k. This is approximately1,664 bytes per additional event in this fixture and excludes original text retained by the harness. RSS and real disk-store amplification require separate measurement. Neither hash-based retrieval nor CAS is literally bounded-memory across unlimited history.

Linux `/proc/self/status` [RSS observations](evidence/math-recall-rss.log) were added for the multilingual extension: first-seed VmRSS grew from4,232KiB at100 to8,196KiB at1k and51,712KiB at10k, with process VmHWM65,984KiB. Later seeds reused the allocator and retained up to62,824KiB, so their HWM is cumulative across datasets, not independent per-corpus peaks. RSS includes the index, original document/needle strings, exhaustive ranking allocations, snapshot temporary allocations and allocator retention; it is **not** index-only memory. This bounds what was actually observed without conflating snapshot size and resident memory.

Source-based optimization candidates: replace repeated O(candidate_cap) min rescans with a bounded heap/selection algorithm; avoid `format!(subject content)`/per-token String allocations on large ingest; distinguish static entry summaries from the actual48-byte `EntrySummary` rather than its stale16-byte comment; consider lexical postings before widening HDC scans. Profile these before production changes. Candidate-cap savings that reduce recall are not accepted optimizations.

## Graph review and limits

`BiTemporalGraph` carries both valid and transaction intervals, but query APIs expose only one `at_ms` and filter `t_valid/t_invalid`; they do not offer a separate as-known-at transaction-time axis. Upsert generally stamps both axes using one timestamp. Describe this as a dual-interval representation with valid-time queries, not a demonstrated full bi-temporal query engine. Temporal source mode (counter vs wall clock) is an adapter concern covered by the broader audit.

CSR snapshot adds forward and reverse weighted adjacency. Each edge endpoint therefore has outgoing mass; the generic PageRank dangling-node concern is not reachable in a normal snapshot built only from nonempty positive-weight edges. Rejecting that lead avoids reporting a formula difference as a defect. Personalized seed matching also permits substring seeds at0.25 weight; the result is heuristic entity expansion, not exact entity identity. Unit tests pin supersession, temporal queries, current stale-failure exclusion, CSR structure and serialization repair. An independent dense Python oracle for a three-node nonuniform weighted graph agrees with production after1,2,6 iterations to about1.12e−7 absolute error ([oracle](evidence/math-ppr-oracle.py), [results](evidence/math-ppr-result.json)). A real-corpus graph contribution ablation was not run. Long histories accumulate historical edges, and snapshot building rescans them.

## Prior reassessment

| Prior ID | Disposition | Evidence and remaining limit |
| --- | --- | --- |
| ICX-AUD-042 | Confirmed fixed for duplicate results | Ranked hits dedup by ArtifactId; candidate-crowding lead remains unverified |
| ICX-AUD-048 | Unverifiable in graph alone | Adapter clock mode determines timestamps; graph itself accepts caller times |
| ICX-AUD-049 | Partially fixed | Repetitive success, varied-corpus collapse004 |
| ICX-AUD-050 | Partially fixed | i32 removes demonstrated i16 saturation; encoding CPU profile not independently refreshed |
| ICX-AUD-051 | Partially fixed in prose | Current subsystem spec says inspired; historical and AHTL prose still contains stronger claims |
| ICX-AUD-052 | Open limitation, partly mitigated | Inverse check rejects opposite orthants, cannot prove original-event suffix identity |
| ICX-AUD-053 | Confirmed fixed as disclosure | hv64 linear collision warning present; no claim hash itself became collision resistant |

## Primary-source ledger

Accessed during this audit (2026-11-21 UTC): [Unicode Braille chart](https://www.unicode.org/charts/PDF/U2800.pdf) redirected to Unicode18.0.0; confirms range and names, not model embeddings. [Titans v1](https://arxiv.org/abs/2501.00663v1), submitted31Dec2024; learned memory inspiration only. [Infini-attention v2](https://arxiv.org/abs/2404.07143v2), revised9Aug2024; compressive attention architecture, not a guarantee for this HDC bundler. [Fractal KV-Cache Archives v2](https://arxiv.org/abs/2607.07144v2), revised5Aug2026; quantized KV symbol archival research, distinct from this lossy HDC→corner projection. No paid model calls or provider tokenizer calls were made. `tiktoken` and `tokenizers` were not installed in the inspected Python environment; token-count/semantic attention assertions remain unverified.
