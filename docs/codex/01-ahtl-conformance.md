# AHTL conformance and redaction

Audited revision: `a996d59f1eb468e7c386747e15f4c7e58d08410e`. This is an independent audit; no production fixes were made. Findings below distinguish reachable ingestion failures from public-library codec failures. Run the [isolated Rust harness](evidence/codec-probe.rs) using the [procedure](evidence/codec-procedure.md).

## PDF provenance and inspection

`docs/ahtl/AHTL_v1_Complete_Technical_Specification_Volume.pdf` is 19,921,768 bytes, PDF 1.7, 58 A4 pages, produced by WeasyPrint 68.1. SHA-256: `6219af31092df75c8aef33c99dfeac23ad258fb6c9a7a970e6e30e8288ab4abd`. The source PDF was preserved.

All 58 pages were text-extracted with `pdftotext -layout`; the [page ledger](evidence/codec-pdf-pages.csv) records the nonempty page inventory and chapter mapping. This does **not** mean every statement on every page was independently verified. Detailed comparison covered grammar, test vectors, binary layout, contraction and Hamming proofs, redaction, and reference implementation. Pages 30 and 35 were rendered at 1,500 pixels and visually inspected: the block-majority and contraction equations and the HDC basis code are legible; image/table layout preserves the relevant meaning. Other diagrams have text/source review rather than comprehensive visual inspection.

Root additionally rendered and inspected four contact sheets covering all 58 pages for layout and page continuity. No blank or obviously clipped page was observed at that overview scale. Small labels, individual table cells and equations were not all legible in those thumbnails; this is not a replacement for the two full-size checks or semantic conformance review. Contact sheets remain in `/tmp/Infinity-Context/pdf-layout-review/` to avoid duplicating the reference publication in audit deliverables.

The PDF now carries v1.1.0 on its cover and Chapter 08 has the current six-section struct and `splitn(6)` parser. Prior ICX-AUD-030 is confirmed fixed for its specific stale-struct defect. Residual claims remain inconsistent; this is not evidence that every prior PDF problem was fixed.

## Normative conformance matrix

| Requirement / source | Implementation / independent check | Result |
| --- | --- | --- |
| Chapter 01 / PDF pp.8–15: outer delimiters, Profile A field widths, 8/64 Braille glyphs | `ahtl.rs`, `braille.rs`; Unicode U+2800–U+28FF chart | Correct fixed-width codepoint/UTF-8 mapping; public fields permit noncanonical construction |
| Chapter 01 §2: LegendHeader and repeated EpochFrames | `AhtlStream::parse_stream` accepts one optional epoch header and then packets only | Specification/API mismatch: legend and a second epoch header are rejected |
| Chapter 01 §2: all SafeChar exclusions in subjects and clause values | `escape_subject`, `escape_clause_field` | Raw CR/LF, `│`, `⟪⟫` can remain in subjects; `│`, `⟪⟫` remain in clauses. Stream round trip may still work; this is conformance, not demonstrated injection |
| Chapter 01: lowercase canonical field hex; optional SliceMap | Parsers accept and normalize uppercase; hint validated structurally | Uppercase behavior documented; SliceMap range semantics and numeric overflow are deferred to consumers |
| Chapter 01: bounded clauses | Iterative `splitn(64,'⇒')` | Prior stack exhaustion fixed for parsed text; API can still manually construct a deeper boxed chain |
| Chapter 01 §6.1 / PDF p.15: BLAKE3 canonical test vector | Python `blake3` independent binding hashes `raw_payload_bytes` | Digest recomputed in evidence; compare exact output, not the codec's own assertion |
| Chapter 01 §6.2, Chapter 03 §4 / PDF pp.15,32: `fc` is Q1.14 | `FractalCoord::encode_fc_hex64` uses `to_q15` | **ICX-CODEX-005**, format mismatch |
| Chapter 02 / PDF pp.16–28: byte↔Braille bijection and all 256 glyph table | `byte_to_braille`, closed-form UTF-8, full byte unit test | Mathematical bijection confirmed; does not establish token count or embedding orthogonality |
| Chapter 02 §4.2: 512 disjoint 20-bit majority blocks | `spectral_bands_64` | The unnormalized Hamming 1-Lipschitz bound follows because each input bit influences one output bit; not an injective encoding |
| Chapter 02 §4.2: full scan always evaluated, no false-negative candidate pruning | `query_associative`, Stage 1 candidate cap | False as a shipped algorithm description; see ICX-CODEX-004 |
| Chapter 04 / PDF pp.34–38: i16 bundler and nonnegative tie threshold | Current i32 counters and coordinate-parity ties | Stale blueprint equations, even though current subsystem spec describes i32 correctly |
| Chapter 05 / PDF pp.39–43: multi-line JSON/YAML secret values | `SecretRedactor` continuation recognizes only next scalar | **ICX-CODEX-001** |
| Chapter 05: cryptographic replacement marker has zero mutual information | Category, location and occurrence remain observable | Conditional claim needs a threat/distribution model; constant replacement alone proves removal of matched value bytes, not unconditional zero mutual information |
| Chapter 07 / PDF pp.49–52: exact identifier retention, embedding orthogonality, zero hallucination, 18.4–72.1x SNR | No tokenizer/model oracle; capped line scanner and heuristic routing | Unsupported model-effectiveness claims; token spelling is not a proof about contextual keys/queries or model tool selection |
| Chapter 08 / PDF pp.53–58: six-section Profile A struct | Current `ahtl.rs` | Struct/parser arity corrected; raw skeleton is not safely framed under all public API inputs |

## ICX-CODEX-001 — YAML scalar credentials survive ingestion redaction

- **Category:** security / secret exposure. **Severity:** Medium. **Confidence:** High. **Revision:** audited HEAD above.
- **Location:** `crates/icx-codec/src/redact.rs`, `SecretRedactor::redact`, `ends_with_sensitive_key`, `redact_value_span`; `docs/alien-holographic-codec.md` §5 and Chapter 05 §3.2.
- **Prerequisites and boundary:** A caller supplies YAML containing a sensitive key with a literal (`|`) or folded (`>`) block value to the normal pre-ingestion redactor. The credential need not use a recognizable vendor prefix or pass an entropy threshold. The trust boundary is untrusted tool/file content → redacted storage/model output. This is local credential exposure, not demonstrated remote exfiltration.
- **Proof:** `SecretRedactor::redact("password: |\n  lowentropyfirst\n  lowentropysecond\npublic: ok\n")` returns the entire string unchanged. For `password: >`, only `>` becomes `[REDACTED:CONFIG_SECRET]`; both secret lines remain. The scalar's bytes survive the very gate that is advertised to remove multi-line YAML secret values.
- **Second continuation-coverage gap:** `password:\n  user=correcthorse\npublic: ok\n` is valid YAML with password scalar `user=correcthorse` (independently parsed using PyYAML). `ContinuationLine::classify` treats identifier-shaped `key=value` text as a nested structure and stops pending sensitive-value redaction, even though YAML does not use `=` to introduce a nested mapping. Both redaction passes leave this scalar unchanged. The real CLI also stores and recovers it unchanged: [harness](evidence/codec-redact-cli.py), [results](evidence/codec-redact-cli-result.json). This lead came from investigating the distinct idempotence failure 006; unlike the fuzzer assertion alone, this fixture demonstrates actual secret-coverage failure.
- **Independent end-to-end recheck:** The root auditor ran real `icx init`, `encode-ahtl`, and `deref --raw` with isolated config/state/runtime/workspace paths. Both synthetic secret lines survived CAS and the public field remained. See [recheck harness](evidence/recheck-yaml-cli.py) and [command results](evidence/yaml-cli-result.json). This establishes current CLI ingestion reachability beyond the library probe.
- **Impact:** Sensitive block values can enter CAS, index-derived output, and downstream context. Model-facing delimiter escaping does not remove them.
- **Counterevidence:** Inline key/value, a value on the immediately following line, many vendor patterns, and PEM blocks have dedicated protections. Documentation correctly admits unknown patterns can pass. This finding is narrower: common YAML syntax under an explicitly sensitive key is advertised as supported but is not recognized.
- **Recommendation:** Track YAML block indicators and indentation, scrubbing the complete scalar through dedent, including `|-`, `|+`, `>2`, comments and blank lines. If syntax-aware parsing is too costly, fail closed for sensitive keys with these indicators while preserving subsequent nonsensitive keys. Keep storage and model-facing paths consistent.
- **Acceptance:** Add fixtures for literal/folded/chomped/indented scalars with CRLF, comments, empty lines and following public fields. Assert no scalar secret bytes survive both `redact` and end-to-end ingest/deref. Confirm subsequent public data is retained.
- **Additional acceptance:** Include YAML plain scalars containing `=` on the line after sensitive keys and distinguish them from real nested `key: value` mappings. Avoid a one-language heuristic silently changing another format's value semantics.
- **Prior:** ICX-AUD-015 is **partially fixed**, not fully fixed for advertised multi-line YAML coverage. ICX-AUD-016's same-line PEM swallowing fix remains independently supported.

## ICX-CODEX-002 — Profile A public skeleton field can inject extra stream packets

- **Category:** codec integrity / framing. **Severity:** Low. **Confidence:** High. **Revision:** audited HEAD above.
- **Location:** `crates/icx-codec/src/ahtl.rs`, `AhtlWirePacketV1::{render,parse}`, `AhtlStream::parse_stream`.
- **Prerequisites:** A library consumer places untrusted text directly in the public `skeleton_body` field, or parses and rerenders a standalone Profile A packet. No existing hook path accepting this field directly was established.
- **Proof:** Start with valid Profile A fields, set `skeleton_body` to `k:v⟧\n⟦Ω:forged⊢admin:true⋮Ψ:⠀⠀⠀⠀⠀⠀⠀⠀`, and render. `AhtlStream::parse_stream` returns **two** packets. `render` inserts the skeleton verbatim, whereas stream parsing terminates on the first `⟧`.
- **Impact:** Library/relay callers cannot assume one typed packet renders to one stream packet. A second packet can assert an invariant it was never separately authorized to create. This does not establish bypass of current MMU Tier-0 sanitization or execution policy.
- **Counterevidence:** Promotion from Profile B builds the skeleton using escaped clause renderers, and normal skeletonization sanitizes untrusted text. Existing SliceMap forgery remediation is effective at that specific field. These controls limit current reachability and justify Low severity.
- **Recommendation:** Validate all Profile A components at construction and render, or encode skeleton delimiters bijectively and reject noncanonical standalone input. Prefer a typed skeleton over an unrestricted public String. Describe compatibility for formerly accepted raw skeletons.
- **Acceptance:** Arbitrary hostile Profile A skeleton property test must yield exactly one packet or an explicit error; standalone parse→render→stream must preserve packet cardinality. Include literal `%` and nested delimiter combinations.
- **Prior:** Related to ICX-AUD-025; that slice-specific defect is fixed, but equivalent framing coverage was not generalized.

## ICX-CODEX-005 — Fractal wire codec disagrees with normative Q1.14 scale

- **Category:** specification / interoperability defect. **Severity:** Medium. **Confidence:** High. **Revision:** audited HEAD above.
- **Location:** `crates/icx-hdc/src/fractal.rs`, `encode_fc_hex64`, `decode_fc_hex64`, `to_q15`; Chapter 01 `FractalCoordField`, Chapter 03 §4, canonical Profile A test vector.
- **Prerequisites:** An independent consumer follows the normative Q1.14 wire description or exchanges coordinates with `FractalCoord` wire methods.
- **Proof:** Coordinate +0.5 encodes as `4000` (round(0.5×32767)); normative Q1.14 requires `2000` (0.5×16384). Decoding canonical `2000` yields approximately 0.25000763 rather than +0.5. Current Profile A parser validates hex length, not scale; internal same-codec round trips cannot catch this differential.
- **Impact:** Cross-implementation distances and coordinate magnitudes disagree by approximately two. Most signs still agree, so Profile A→B orthant projection alone can conceal the defect.
- **Counterevidence:** Both Q15 and Q1.14 conversion APIs exist; encode/decode are internally consistent within Q15 quantization error, and the bus separately uses Q15. The problem is specifically the unversioned `fc` wire contract, not Q15 arithmetic itself.
- **Recommendation:** Choose one normative scale for Profile A, version or migrate existing archived coordinates, and keep bus serialization explicitly separate. If Q1.14 remains normative, use `to_q1_14/from_q1_14` in `fc` wire methods. Do not silently reinterpret existing records.
- **Acceptance:** Independent literal vectors for −1, −0.5, 0, +0.5, +1 must assert exact wire words and decoded magnitudes; run vectors in both implementations and test old-version migration.
- **Prior:** ICX-AUD-031 numeric consistency is still partial in this area; prior G12 relabeling the example as Q1.14 did not synchronize the encoder.

## Prior findings and rejected leads

| Prior ID | Independent disposition | Basis |
| --- | --- | --- |
| ICX-AUD-015 | Partially fixed | URL/vendor/next-line fixtures added; YAML block scalars fail (001) |
| ICX-AUD-016 | Confirmed fixed for reported inline-PEM bug | Inline spans replace start..end and continue; runaway bounds remain documented |
| ICX-AUD-024 | Confirmed fixed | Hex epoch formatting, 4–8 digit parser and DAG round-trip test |
| ICX-AUD-025 | Confirmed fixed at slice_hint | Invalid hints not rendered and rejected on parse; Profile A skeleton separate defect 002 |
| ICX-AUD-026 | Confirmed fixed for parser input | Iterative max 64 materialized links; no claim on arbitrary manually built/serde-recursive trees |
| ICX-AUD-030 | Confirmed fixed for stale Chapter 08 struct | PDF now has current fields and six-way split |
| ICX-AUD-031 | Partially fixed | Q1.14/Q15 mismatch 005; current core spec improved but AHTL chapters remain stale |
| ICX-AUD-032 | Partially fixed via supplement | Historical exactness review unchanged; corrections file supplies arithmetic and provenance caveats |
| ICX-AUD-039 | Partially fixed | Properties/fuzz targets exist; Profile A hostile fields and YAML scalar syntax absent from shown tests |

Rejected as findings: uppercase hex is explicitly accepted; invalid SliceMap omission is documented; `permute(0)` not being identity is explicitly explained in Chapter 04; Braille's 3-byte storage cost is intentional. None is evidence of memory unsafety. Regex/prefix source skeletonization is deliberately lossy; its limits must qualify the stronger 100%-identifier-retention prose.

## ICX-CODEX-006 — Redaction changes again when a previously redacted payload is processed

- **Category:** correctness / output stability. **Severity:** Low. **Confidence:** High. **Revision:** audited HEAD above.
- **Location:** `crates/icx-codec/src/redact.rs`, `SecretRedactor::redact`, `ContinuationLine::classify`, vendor prefix replacement in `redact_line`; `fuzz/fuzz_targets/redact.rs:10`.
- **Discovery and prerequisites:** The redactor's own fuzz target asserts `R(R(s)) == R(s)`. It found a synthetic 153-byte valid UTF-8 string after about 540,000 executions. A caller only needs to process an already redacted string again, a normal possibility in ingest/sanitize/deref layers. No external credentials or unusual memory behavior are needed.
- **Reproduction:** [Minimizer](evidence/codec-redact-minimize.py), [raw escaped evidence](evidence/codec-redact-idempotence.json), [22-byte minimized binary](evidence/codec-redact-minimal.bin), [readable fixture](evidence/codec-redact-readable.txt). The minimum-by-deletion input is `TOKEN:\nPsk-ssVk-kP-P=S`; pass 1 yields `TOKEN:\nP[REDACTED:API_KEY]=S`, pass 2 yields `TOKEN:\n[REDACTED:SECRET]`. A readable equivalent is `password:\n  ask-123456789=x\n`. The independent state-debug auditor reproduces the same mechanism with `TOKEN:\nPsk-OyyyTOKEN:` ([source](evidence/state-debug-redact.rs), [output](evidence/state-debug-redact.log)).
- **Artifact integrity:** Original SHA-256 `565d4de317ac8f92eae0c6f1df205d47ec8a00a6b4a0c4331df645fe59921230`;22-byte deletion-minimal SHA-256 `6881bcdd47c3972c7e4f56380851c51981bb59482bf0c1072e29563026c8fdfa`; readable 28-byte fixture SHA-256 `e59908f4370963a497cb4f6f4634a3cb48d7d3b8252684091ff1e0418f8f377e`. The JSON preserves original bytes as hex; original fuzzer artifact remains in isolated scratch storage. Minimality means no further individual-byte deletion preserves this failure, not a proof of a globally shortest synthesized input.
- **Cause:** Pending sensitive-value continuation is classified from the **original** line. A nested-structure-looking identifier may contain a vendor-token substring; replacing that substring inserts `[REDACTED:...]` punctuation. On a later pass, the mutated line no longer looks like an identifier-shaped structure and is treated as the pending secret scalar, so the entire line is removed. Prefix scanning and continuation recognition are not compositionally stable.
- **Impact:** Additional content is removed on subsequent passes. Redaction output, downstream packet hashes and stored/model-facing views can differ depending on how many times the gate runs. The reported fuzz process terminates because the **test assertion** fails; the production redactor returns normally for these inputs. No production crash, memory-safety violation or secret exfiltration is established by this idempotence fixture. The separately confirmed YAML equals-scalar leakage is merged into 001, not counted twice here.
- **Counterevidence:** Existing random hostile-text properties and many normal fixtures pass, and the embedded vendor substring is removed on the first pass. The problem is output stability and unintended further erasure, not proof that the matched vendor credential survives.
- **Recommendation:** Make continuation/structure decisions stable across vendor replacement, using typed format/context state or a canonical redacted representation. Do not blindly loop until convergence: that can hide the original semantic classification error and over-redact legitimate structures. Preserve the arbitrary-input idempotence property as a real gate.
- **Acceptance:** Add exact original/minimized/readable regression vectors and assert first-pass expected semantics, `R(R(s))==R(s)`, and stable ingest→model-view behavior. Retain fixtures with real nested mappings, comments, quoted scalars and `=`-containing YAML values to prevent trading stability for a secret leak. Replay the original fuzz artifact, then resume the campaign with the known failure explicitly reported until a production fix is separately authorized.
- **Prior:** ICX-AUD-039 remains partially fixed: fuzz coverage is valuable and discovered a real violated invariant. ICX-AUD-015 covers adjacent redaction recognition gaps; this idempotence failure is a new finding.
