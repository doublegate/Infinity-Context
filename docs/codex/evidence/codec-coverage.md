# Codec/math coverage and verification limits

This is the workstream's disposition ledger, not a claim every possible defect was excluded. The root coverage ledger may have additional independent readers. Date:2026-11-21 UTC; revision a996d59f1eb468e7c386747e15f4c7e58d08410e.

| Path(s) | Review depth / disposition |
| --- | --- |
| `crates/icx-codec/src/ahtl.rs` | Detailed production parser/renderer/conversion/stream review; tests sampled plus `tests/codec_v110.rs` read; hostile ProfileA probe |
| `crates/icx-codec/src/braille.rs` | Production mapping/layout and full-byte unit assertions read; formula checked against primary Unicode chart |
| `crates/icx-codec/src/lib.rs` | Trait/default implementation read |
| `crates/icx-codec/src/redact.rs` | Production flow and boundary helpers reviewed in chunks; existing large in-module fixture tail sampled, not every fixture independently checked |
| `crates/icx-codec/src/skeleton.rs` | Classifier, log/source/diff extraction, caps, token compaction and brace scanner reviewed; in-module multi-language examples sampled |
| `crates/icx-codec/tests/properties.rs` | Entire property strategy/assertion definitions read; ProfileA fields and YAML block scalar semantics absent |
| `crates/icx-codec/tests/codec_v110.rs` | Entire file read; source supports prior024/025/026 dispositions |
| `crates/icx-codec/tests/redact_v110.rs` | Vendor, key, multi-line and PEM cases read; remainder losslessness assertions sampled |
| `crates/icx-hdc/src/lib.rs` | Entire production index/snapshot/query pipeline read; snapshot and recall unit cases sampled; independent scaling matrix complete |
| `crates/icx-hdc/src/hv.rs` | Seed framing, text encoder, binding/permutation, sketches, bundler and snapshot routines reviewed; tests sampled, no independent BLAKE3 XOF vector suite |
| `crates/icx-hdc/src/fractal.rs` | Entire production coordinate/archive and tests read; affine bounds derived; quantization differential reproduced |
| `crates/icx-hdc/src/snapshot.rs` | Entire bounds/reader/writer file read; valid snapshot NaN mutation accepted |
| `crates/icx-hdc/src/surprise.rs` | Entire gate implementation/tests read; primary-paper distinction checked |
| `crates/icx-hdc/tests/recall_v110.rs` | Entire fixture read; independent matrix extends n300/20word gate to100/1k/10k and varied vocabulary |
| `crates/icx-store/src/lib.rs` | Export-only module inventoried |
| `crates/icx-store/src/graph.rs` | Production edge lifecycle, CSR/PPR, load validation and repair read; tests sampled; independent three-node dense PPR oracle matches1/2/6iterations |
| `docs/ahtl/01-formal-grammar-and-packet-specification.md` | Detailed grammar, budgets, parser and vectors read |
| `docs/ahtl/02-braille-256-spectral-bijection.md` | Narrative/formulas/layout read; huge glyph table only sampled, not all256 table cells checked by hand |
| `docs/ahtl/03-fractal-ifs-chaos-game-coordinates.md` | Detailed contraction/suffix and quantization review |
| `docs/ahtl/04-hyperdimensional-10240-bit-lattice.md` | Detailed HDC formulas and blueprint comparison |
| `docs/ahtl/05-structural-ast-skeletonization-and-redaction.md` | Detailed redaction, extraction, exactness and leakage claims review |
| `docs/ahtl/06-blake3-merkle-cas-and-virtual-handles.md` | Hash/handle/interface claims read; core storage validation owned by another workstream |
| `docs/ahtl/07-transformer-attention-and-bpe-mechanics.md` | Entire prose read; model/token assertions unsupported by workstream's tools |
| `docs/ahtl/08-v1-reference-implementation-blueprint.md` | Entire current blueprint read; PDF current struct comparison |
| `docs/ahtl/README.md` | Narrative/tables/lifecycle read; asset-list links inventoried, binaries not individually visually inspected |
| `docs/ahtl/AHTL_v1_Complete_Technical_Specification_Volume.pdf` | Complete58-page extraction/inventory; target-claim comparison across chapters; pages30/35 visual; every page **not** fully visually reviewed |
| `docs/alien-holographic-codec.md` | Entire subsystem document read |
| `docs/hyperdimensional-and-fractal-index.md` | Entire subsystem document read |
| `ref-docs/ahtl-exactness-review-and-analysis.md` | Historical claims/ENFORCED table, prior risk leads and corrections compared; immutable source retained |
| `ref-docs/novel-methods/non-human-neural-codecs-and-holographic-context.md` | Entire text read; identified unsupported suffix iff/model guarantees and obsolete figures |
| `ref-docs/ahtl-foundations/README.md` | Entire239-line narrative, formulas and asset catalog read |
| `ref-docs/hypercube-and-hamming-geometry/README.md` |366-line text read in two chunks, latter long output partially truncated at bibliography; mathematical sections reviewed; complete reference-by-reference checking not done |
| `ref-docs/corrections-2026-11-18.md` | Read for numeric/bibliographic corrections; does not repair every normative AHTL statement |
| `ref-docs/unicode-braille-patterns/README.md` | Inventoried; not fully read by this workstream; independent official chart consulted |
| `ref-docs/blake3-cryptographic-hash/README.md` | Inventoried; not fully read by this workstream; storage/cryptographic source review delegated elsewhere |
| `docs/claude/01-ahtl-spec-conformance.md`, `03-hdc-fractal-math.md`, `12-v1.1.0-remediation-status.md` | Read as hypotheses; dispositions in reports, never accepted as independent proof |

## Experiment completion

Isolated harness release compilation and all probes completed exit0. The18-row ASCII retrieval matrix and nine-row multilingual extension completed exit0. The latter includes process VmRSS/VmHWM (not index-only memory). The independent Python PPR vector completed exit0 and matches production within f32 error. No production tests were changed. The parent audit runs baseline workspace gates. Fuzzing/mutation is parent-owned; this workstream does not independently claim a15-minute fuzz run or complete mutation coverage. Cold restart scaling, graph ablation, long-document encoder caps, semantic paraphrases, tokenizer counts and live-model recall were not measured here. These remain explicit audit limitations.

## Research boundaries

Primary Unicode chart, Titans, Infini-attention and Fractal KV arXiv metadata were opened using the web tool. Paper full mathematical proofs were not all rederived from full primary text. The algebra and contraction arguments in the report are independently derivable from the code's explicit formulas. Current provider integration/caching research belongs to other workstreams. No archived HTML snapshot was treated as permission or operational guidance.
