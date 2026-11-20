# Alien Holographic Token Lattice (`AHTL`) & Structural Skeletonizer Specification

**Applies to:** `icx` v1.1.0  
**References:** [docs/ahtl/README.md](ahtl/README.md), [docs/ahtl/AHTL_v1_Complete_Technical_Specification_Volume.pdf](ahtl/AHTL_v1_Complete_Technical_Specification_Volume.pdf), [ref-docs/novel-methods/non-human-neural-codecs-and-holographic-context.md](../ref-docs/novel-methods/non-human-neural-codecs-and-holographic-context.md) §1, [ref-docs/research-report.md](../ref-docs/research-report.md) §4.2  
**Target Crate:** `crates/icx-codec`

---

## 1. Purpose

`crates/icx-codec` transforms bloated human-readable text, source code, diffs, compiler outputs, and conversation turns into **AHTL (Alien Holographic Token Lattice)**—a non-human, BPE-aligned Dual-Profile symbolic representation (`Profile A` `AhtlWirePacketV1` and `Profile B` `AhtlPacket`) intended to be read by the model rather than by people. A packet is a structural skeleton (identifiers, line ranges, error codes, failing tests, causal clauses) plus a dereference pointer (`#icx:<16-hex>` or `#icx:<12-hex>`) to the stored bytes. It is not a lossless encoding: the full text is recovered by dereferencing the pointer. The size reduction depends on the input; `icx benchmark` measures 214x on a repetitive 1,200-line test log and 159.8x on a varied log, both synthetic ([docs/performance.md](performance.md)).

---

## 2. Formal AHTL v1.0 Dual-Profile Grammar (EBNF)

```ebnf
AhtlStream      ::= EpochHeader ("\n" WirePacket)+ ;
EpochHeader     ::= "⟪ICX:v1│ep=" Hex4to8 "│root=" ( Hex8 | Hex16 ) "│" EpochCount "⟫" ;
EpochCount      ::= ( "τ=" | "nodes=" | "folded=" ) UInt32 ;  (* epoch id is hexadecimal: ep=000a is 10 *)
WirePacket      ::= ProfileAPacket | ProfileBPacket ;
ProfileAPacket  ::= "⟦AHTL v1│" VirtualHandle "│" FractalCoord "│" HdcDigest
                    "│" BrailleSpectrum64 "│" SkeletonPayload "⟧" ;
ProfileBPacket  ::= "⟦" PacketKind ":" Subject ( "⊢" ClauseList )? ( "↦" DerefHandle )? ( "⋮Ψ:" Hologram )? "⟧" ;
PacketKind      ::= "Δ"   (* Code/AST/File mutation or read *)
                  | "Ψ"   (* Tool execution / diagnostic stub *)
                  | "Ω"   (* Architectural invariant / hard rule *)
                  | "Θ"   (* Cross-agent telepathy broadcast *)
                  | "Φ"   (* Folded sub-trajectory branch commit *) ;
Subject         ::= PathOrSymbol ( "#L" LineRange )? ;
ClauseList      ::= Clause ( "⋮" Clause )* ;
Clause          ::= OpPrefix? Identifier ( ":" ValueExpr )? ( "⇒" Clause )? ;  (* at most 64 "⇒" links *)
OpPrefix        ::= "⊕"   (* Added / defined *)
                  | "⊖"   (* Removed / invalidated *)
                  | "¬"   (* Negative constraint / forbidden *)
                  | "!"   (* High-surprise anomaly / error / panic *)
                  | "✓"   (* Verified / test passed *) ;
DerefHandle     ::= "#icx:" ( Hex16 | Hex12 ) ( "(" SliceMap ")" )? ;
SliceMap        ::= "L" UInt "-" UInt | "B" UInt ":" UInt ;
Hologram        ::= BrailleSketch ( "[" Hex4 "]" )? ;
BrailleSketch   ::= { U+2800 .. U+28FF }+ ; (* 8 bits per Unicode Braille codepoint *)
```

### 2.1 Parser Bounds and Validation (v1.1.0)

- **Epoch ids** are `u32`, written and parsed as 4 to 8 hexadecimal digits everywhere (seal, fold,
  retarget). v1.0.0 wrote some headers in decimal, so epoch 10 read back as 16 (ICX-AUD-024).
- **Slice hints** must match `SliceMap` (`is_valid_slice_map`). An invalid `slice_hint` is never
  rendered and is rejected when parsing, which prevents a hint from injecting packet structure
  into a stream (ICX-AUD-025).
- **Clause chains** are parsed iteratively and materialize at most `MAX_CLAUSE_CHAIN = 64` `⇒`
  links, so arbitrarily long chains cannot exhaust the stack (ICX-AUD-026).
- **`hv:` digests** (`hv_digest_u64`, Profile A `HdcDigest`) are a linear, non-cryptographic sketch
  of the hypervector. They are not an identity and must not be used for integrity (ICX-AUD-053).

---

## 3. Dual-Channel Encoding Mechanics

### 3.1 Channel A: Attention-Queryable Relational Skeleton

Standard BPE tokenizers (`o200k_base`, `cl100k_base`, Claude BPE) split punctuation-heavy JSON (`{"file": "src/main.rs", "line": 42}`) into 14+ tokens. AHTL replaces JSON structural keys with single-codepoint mathematical operators (`⊢`, `⇒`, `↦`, `⋮`, `⊕`, `⊖`, `¬`, `!`, `✓`) and strips all whitespace between clauses.

- **Crucial Invariant:** Identifiers (`alloc_page`, `E0502`, `state_5.sqlite`) are emitted **verbatim** without internal character mangling so that the model's token embeddings for those identifiers match user queries identically in attention space.

### 3.2 Channel B: Braille-256 Holographic Sketch (`U+2800..U+28FF`)

Each compressed Profile B packet appends a compact 8-character Braille-256 sketch (`Ψ:⠒⠴⡖⡸⢚⢼⣞⣰`) encoding the 64-bit `ChannelBHologram` metadata fingerprint (`B[0..=3]` 32-bit big-endian HDC XOR parity fold, `B[4..=5]` 16-bit big-endian quantized IFS quadrant signature, `B[6]` 8-bit quantized surprise $S_t$ from the Titans-inspired gate, `B[7]` flags byte: bits `0..=1` retention tier (`TIER_MASK`), bit 2 `FLAG_HAS_ERROR`, bit 3 `FLAG_HAS_PANIC`, bit 4 `FLAG_HAS_DIFF`, bit 5 `FLAG_IS_FOLDED`, bit 6 `FLAG_IS_EPOCH`, bit 7 `FLAG_IS_REDACTED`), while Profile A carries a 64-character (512-bit) 1-Lipschitz HDC spectral band projection (`Hypervector::spectral_bands_64`, `encode_braille_64` / `decode_braille_64`).

Encoding byte $b \in [0, 255]$ to Unicode Braille (`U+2800 + b`) and its exact 3-byte UTF-8 triple (`[0xE2, 0xA0 | (b >> 6), 0x80 | (b & 0x3F)]`) is a branchless constant-time bijection (`byte_to_braille`, `braille_to_byte`, `byte_to_braille_utf8`, `braille_utf8_to_byte`).

---

## 4. Domain-Specific Structural Skeletonizers (`StructuralSkeletonizer`)

Before AHTL packet assembly, `StructuralSkeletonizer` extracts high-entropy structural skeletons from raw text in $O(N)$ single-pass scans:

1. **Source Code Skeletonizer (`Rust`, `TypeScript/JavaScript`, `Python`, `Go`, `C/C++`):**
   - Extracts module/package headers (`mod@L<n>`), folds contiguous header imports (`imports:L<a>-<b>(N)` including Go `import (...)` blocks), and extracts `fn` / `def` / `func` / `struct` / `enum` / `trait` / `impl` / `class` / `interface` / `type` signatures with quote-, Rust-lifetime- (`'a`), and comment-aware `L<start>-<end>` line spans, plus `TODO`/`FIXME`/`SAFETY` notes.
2. **Compiler & Test Log Skeletonizer (`cargo test`/`clippy`, `pytest`, `vitest`/`jest`, `tsc`, `go test`):**
   - Collapses $N$ passing test lines into a single aggregate counter (`✓pass:N_tests`).
   - Preserves 100% of failing test names (`!FAIL:<test_name>`, deduplicated across summary blocks), `error[E...]`, `FAILED`, `panicked at`, and panic locations (`!PANIC@...`).
3. **Unified Diff Skeletonizer (`git diff`):**
   - Pairs adjacent `--- a/<path>` / `+++ b/<path>` headers (`file:<path>`), extracts hunk headers (`@@ -a,b +c,d @@`), removed lines (`⊖`), and added lines (`⊕`), and stores the full patch in CAS.

---

## 5. Secret Redaction (`SecretRedactor`)

Per Module 60, secrets are removed before content enters the CAS, the HDC and bi-temporal
indexes, or the telepathy ring. v1.1.0 separates two operations (ICX-AUD-021):

- **`SecretRedactor::redact` (storage).** Removes secrets and preserves every other byte: line
  endings (including CRLF), trailing newlines and literal delimiters. For secret-free input the
  output equals the input, which is what makes CAS dereferencing lossless. Redacted artifacts set
  `FLAG_IS_REDACTED` (bit 7 of `B[7]`).
- **`SecretRedactor::sanitize` (model-facing text).** `redact` followed by Tier-0 delimiter
  escaping. Used for packets, skeletons, tail slots and MCP/CLI deref output, never for stored
  bytes.

Tiers:

- **Tier 0 (delimiter neutralization, `sanitize` only):** escapes literal `⟪` (`<<`), `⟫` (`>>`),
  `⟦` (`[[`) and `⟧` (`]]`) so untrusted text cannot imitate an AHTL packet or epoch header. Text
  that is byte-identical to a block `icx` emitted is recognized by digest, not by prefix
  ([docs/security-and-threat-model.md](security-and-threat-model.md) §4.3).
- **Tier 1 (private-key blocks):** multi-line `-----BEGIN ... PRIVATE KEY-----` and PGP private key
  blocks; single-line (JSON-escaped) blocks are redacted inline; an unterminated block stops
  consuming after 200 lines (ICX-AUD-016).
- **Tier 2 (known formats and key names):** vendor token prefixes (Anthropic, OpenAI, Stripe,
  GitHub, GitLab, AWS, GCP, Hugging Face, NPM, PyPI, Slack bot/app tokens and webhooks, SendGrid,
  DigitalOcean, Shopify, Twilio, `ai-memory`), 3-segment JWTs, URL user-info passwords for every
  scheme, CLI secret flags (`--api-key=`, `--token=`, `--password=`, `--secret=`), `Bearer` and
  `Basic` headers, and `.env`/JSON/YAML assignments to sensitive key names (including multi-line
  JSON/YAML values after a sensitive key) (ICX-AUD-015).
- **Tier 3 (entropy gate):** unprefixed high-entropy Base64 tokens ($|S| \ge 24$,
  $H(S) > 4.35$ bits/char) and hex assignment values ($|S| \ge 32$, $H(S) > 3.50$ bits/char).

Redaction is pattern-based. Secrets in unrecognized formats can pass through, and high-entropy
non-secrets can be removed; see [docs/security-and-threat-model.md](security-and-threat-model.md)
§5.

---

## 6. Test Plan

- **Property tests (`proptest`, `crates/icx-codec/tests/properties.rs`):** `redact` is the
  identity on secret-free text and idempotent; `sanitize` removes every delimiter; packets built
  from hostile field text render to exactly one packet and round-trip; clause parsing is total.
- **Codec regressions (`crates/icx-codec/tests/codec_v110.rs`):** hex epoch ids, `SliceMap`
  validation, bounded clause chains.
- **Redaction fixtures (`crates/icx-codec/tests/redact_v110.rs`):** URL user-info for every scheme,
  additional vendor formats and key names, multi-line JSON/YAML values, single-line and
  unterminated PEM blocks, losslessness on secret-free input.
- **Compression tests (`crates/icx-codec/src/skeleton.rs`):** `>= 25x` on a synthetic passing
  test log and `>= 15x` on a synthetic pytest log, with failing test names, error codes and handles
  preserved.
- **Fuzzing (`fuzz/fuzz_targets/ahtl_stream.rs`, `redact.rs`):** the stream parser and the
  redactor must not panic on arbitrary input.
