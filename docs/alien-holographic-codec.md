# Alien Holographic Token Lattice (`AHTL`) & Structural Skeletonizer Specification

**References:** [docs/ahtl/README.md](ahtl/README.md), [docs/ahtl/AHTL_v1_Complete_Technical_Specification_Volume.pdf](ahtl/AHTL_v1_Complete_Technical_Specification_Volume.pdf), [ref-docs/novel-methods/non-human-neural-codecs-and-holographic-context.md](../ref-docs/novel-methods/non-human-neural-codecs-and-holographic-context.md) §1, [ref-docs/research-report.md](../ref-docs/research-report.md) §4.2  
**Target Crate:** `crates/icx-codec`

---

## 1. Purpose

`crates/icx-codec` transforms bloated human-readable text, source code, diffs, compiler outputs, and conversation turns into **AHTL (Alien Holographic Token Lattice)**—a non-human, BPE-aligned Dual-Profile symbolic representation (`Profile A` `AhtlWirePacketV1` and `Profile B` `AhtlPacket`) designed exclusively for Transformer self-attention heads. It guarantees **12x–72x token reduction** while keeping every identifier, line range, error code, causal relationship, and BLAKE3 lossless dereference pointer (`#icx:<16-hex>` or `#icx:<12-hex>`) directly queryable by `Q-K` dot-product attention.

---

## 2. Formal AHTL v1.0 Dual-Profile Grammar (EBNF)

```ebnf
AhtlStream      ::= EpochHeader ("\n" WirePacket)+ ;
EpochHeader     ::= "⟪ICX:v1│ep=" Hex4 "│root=" Hex8 "│τ=" Int "⟫" ;
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
Clause          ::= OpPrefix? Identifier ( ":" ValueExpr )? ( "⇒" Clause )? ;
OpPrefix        ::= "⊕"   (* Added / defined *)
                  | "⊖"   (* Removed / invalidated *)
                  | "¬"   (* Negative constraint / forbidden *)
                  | "!"   (* High-surprise anomaly / error / panic *)
                  | "✓"   (* Verified / test passed *) ;
DerefHandle     ::= "#icx:" ( Hex16 | Hex12 ) ( "(" SliceMap ")" )? ;
Hologram        ::= BrailleSketch ( "[" Hex4 "]" )? ;
BrailleSketch   ::= { U+2800 .. U+28FF }+ ; (* 8 bits per Unicode Braille codepoint *)
```

---

## 3. Dual-Channel Encoding Mechanics

### 3.1 Channel A: Attention-Queryable Relational Skeleton
Standard BPE tokenizers (`o200k_base`, `cl100k_base`, Claude BPE) split punctuation-heavy JSON (`{"file": "src/main.rs", "line": 42}`) into 14+ tokens. AHTL replaces JSON structural keys with single-codepoint mathematical operators (`⊢`, `⇒`, `↦`, `⋮`, `⊕`, `⊖`, `¬`, `!`, `✓`) and strips all whitespace between clauses.
- **Crucial Invariant:** Identifiers (`alloc_page`, `E0502`, `state_5.sqlite`) are emitted **verbatim** without internal character mangling so that the model's token embeddings for those identifiers match user queries identically in attention space.

### 3.2 Channel B: Braille-256 Holographic Sketch (`U+2800..U+28FF`)
Each compressed Profile B packet appends a compact 8-character Braille-256 sketch (`Ψ:⠒⠴⡖⡸⢚⢼⣞⣰`) encoding the 64-bit `ChannelBHologram` metadata fingerprint (`B[0..=3]` 32-bit HDC XOR parity fold, `B[4]` quantized Titans surprise $S_t$, `B[5]` quantized IFS quadrant signature, `B[6]` `AgentKind` ID, `B[7]` `TIER_MASK` bits `0..=1` + `FLAG_*` bits `2..=7`), while Profile A carries a 64-character (512-bit) 1-Lipschitz HDC spectral band projection (`Hypervector::spectral_bands_64`, `encode_braille_64` / `decode_braille_64`).

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

## 5. 4-Tier Pre-Ingestion Secret Redaction Gate (`SecretRedactor`)

Per Module 60 (Security & Input Validation), **no unredacted secret may ever enter the CAS, AHTL stream, HDC/Bi-Temporal index, or `/dev/shm` bus**. `SecretRedactor::sanitize()` runs *before* BLAKE3 hashing across 4 defense-in-depth tiers and sets `FLAG_IS_REDACTED` (`1 << 7`) on `ChannelBHologram` byte `B[7]`:
- **Tier 0 (Delimiter Neutralization):** Escapes literal `⟪` (`<<`), `⟫` (`>>`), `⟦` (`[[`), and `⟧` (`]]`) characters so untrusted input can never forge an AHTL packet or epoch header.
- **Tier 1 (Cryptographic Private Key Blocks):** Strips multi-line `-----BEGIN ... PRIVATE KEY-----` PEM blocks and `-----BEGIN PGP PRIVATE KEY BLOCK-----` blocks.
- **Tier 2 (28 Vendor Prefixes, JWTs, URLs, CLI Flags & Key-Value Assignments):** Strips Anthropic (`sk-ant-*`), OpenAI (`sk-proj-*`, `sk-svcacct-*`, `sk-*`), Stripe (`sk_live_*`, `rk_live_*`, `whsec_*`), GitHub (`ghp_*`, `gho_*`, `ghu_*`, `ghs_*`, `ghr_*`, `github_pat_*`), AWS (`AKIA*`, `ASIA*`), GCP (`AIza*`), Hugging Face (`hf_*`), NPM (`npm_*`), PyPI (`pypi-*`), Slack (`xox[bparso]-*`), `ai-memory` (`aim_*`), 3-segment Base64URL JWTs (`eyJ...`), database/broker connection URL passwords (`postgres://`, `mysql://`, `mongodb://`, `redis://`, `amqp://`), CLI flags (`--api-key=`, `--token=`, `--password=`, `--secret=`), `Bearer`/`Basic` auth headers, and `.env`/JSON/YAML secret assignments.
- **Tier 3 (Shannon Character Entropy Gate $H(S)$):** Computes character Shannon entropy $H(S) = -\sum P(c)\log_2 P(c)$ (`SecretRedactor::shannon_entropy`) and redacts unprefixed high-entropy Base64 tokens ($|S| \ge 24$, $H(S) > 4.35\text{ bits/char}$) and hex assignment tokens ($|S| \ge 32$, $H(S) > 3.50\text{ bits/char}$).

---

## 6. Test Plan

- **Round-Trip Braille & Packet Parser Property Tests (`proptest`):** Arbitrary byte arrays and `AhtlPacket` structs serialize to AHTL strings and parse back with 100% structural equality.
- **Compression Ratio Gate:** Over a benchmark fixture of 50 KB `cargo test` output and 30 KB Rust source files, `AhtlEncoder` achieves `>= 15x` byte/token reduction while preserving 100% of error codes, function names, and `#icx:<hash>` handles.
- **Redaction Gate:** Every known secret pattern is stripped prior to CAS insertion.
