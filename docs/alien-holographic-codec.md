# Alien Holographic Token Lattice (`AHTL`) & Structural Skeletonizer Specification

**References:** [ref-docs/novel-methods/non-human-neural-codecs-and-holographic-context.md](../ref-docs/novel-methods/non-human-neural-codecs-and-holographic-context.md) §1, [ref-docs/research-report.md](../ref-docs/research-report.md) §4.2  
**Target Crate:** `crates/icx-codec`

---

## 1. Purpose

`crates/icx-codec` transforms bloated human-readable text, source code, diffs, compiler outputs, and conversation turns into **AHTL (Alien Holographic Token Lattice)**—a non-human, BPE-aligned symbolic representation designed exclusively for Transformer self-attention heads. It guarantees **12x–40x token reduction** while keeping every identifier, line range, error code, causal relationship, and BLAKE3 lossless dereference pointer (`#icx:<hex8>`) directly queryable by `Q-K` dot-product attention.

---

## 2. Formal AHTL Grammar (EBNF)

```ebnf
AhtlStream      ::= FrameHeader Packet+ ;
FrameHeader     ::= "⟪ICX:v1" "│ep=" Hex4 "│ root=" Hex8 "│τ=" Int "⟫" ;
Packet          ::= "⟦" PacketKind ":" Subject ( "⊢" ClauseList )? ( "↦" DerefHandle )? ( "⋮Ψ:" Hologram )? "⟧" ;
PacketKind      ::= "Δ"   (* Code/AST/File mutation or read *)
                  | "Ψ"   (* Tool execution / diagnostic stub *)
                  | "Ω"   (* Architectural invariant / hard rule *)
                  | "Θ"   (* Cross-agent telepathy broadcast *)
                  | "Φ"   (* Folded sub-trajectory branch commit *) ;
Subject         ::= PathOrSymbol ( "#" LineRange )? ;
ClauseList      ::= Clause ( "⋮" Clause )* ;
Clause          ::= OpPrefix? Identifier ( ":" ValueExpr )? ( "⇒" Clause )? ;
OpPrefix        ::= "⊕"   (* Added / defined *)
                  | "⊖"   (* Removed / invalidated *)
                  | "¬"   (* Negative constraint / forbidden *)
                  | "!"   (* High-surprise anomaly / error / panic *)
                  | "✓"   (* Verified / test passed *) ;
DerefHandle     ::= "#icx:" Hex16 ( "(" SliceMap ")" )? ;
Hologram        ::= BrailleSketch "[" Hex4 "]" ;
BrailleSketch   ::= { U+2800 .. U+28FF }+ ; (* 8 bits per Unicode Braille codepoint *)
```

---

## 3. Dual-Channel Encoding Mechanics

### 3.1 Channel A: Attention-Queryable Relational Skeleton
Standard BPE tokenizers (`o200k_base`, `cl100k_base`, Claude BPE) split punctuation-heavy JSON (`{"file": "src/main.rs", "line": 42}`) into 14+ tokens. AHTL replaces JSON structural keys with single-codepoint mathematical operators (`⊢`, `⇒`, `↦`, `⋮`, `⊕`, `⊖`, `¬`, `!`, `✓`) and strips all whitespace between clauses.
- **Crucial Invariant:** Identifiers (`alloc_page`, `E0502`, `state_5.sqlite`) are emitted **verbatim** without internal character mangling so that the model's token embeddings for those identifiers match user queries identically in attention space.

### 3.2 Channel B: Braille-256 Holographic Sketch (`U+2800..U+28FF`)
Each compressed packet appends a compact 8-character Braille-256 sketch (`Ψ:⠞⠧...`) encoding the 64-bit topological fingerprint of the raw block:
- Bytes `0..3` (32 bits): Top 32 bits of the 10,240-bit HDC hypervector folded via XOR-tree parity.
- Bytes `4..5` (16 bits): Quantized Fractal Chaos-Game Coordinate (`FCGA` quadrant signature).
- Bytes `6..7` (16 bits): Quantized Titans Surprise score (`u8`) + structural kind bitmask (`u8`).

Encoding byte $b \in [0, 255]$ to Unicode Braille is a branchless constant-time operation:
```rust
#[inline(always)]
pub const fn byte_to_braille(b: u8) -> char {
    // U+2800 is the base of the 256-entry Unicode Braille Patterns block
    char::from_u32(0x2800 + (b as u32)).unwrap()
}

#[inline(always)]
pub const fn braille_to_byte(c: char) -> Option<u8> {
    let code = c as u32;
    if code >= 0x2800 && code <= 0x28FF {
        Some((code - 0x2800) as u8)
    } else {
        None
    }
}
```

---

## 4. Domain-Specific Structural Skeletonizers

Before AHTL packet assembly, `StructuralSkeletonizer` extracts high-entropy structural skeletons from raw text in $O(N)$ single-pass scans:

1. **Source Code Skeletonizer (`Rust`, `TypeScript`, `Python`, `Go`, `C/C++`):**
   - Extracts module/package declarations, `pub fn` / `fn` / `def` / `func` signatures with line ranges, `struct`/`enum`/`trait`/`interface`/`class` definitions, `unsafe` blocks, and `TODO`/`FIXME`/`SAFETY` comments.
   - Elides function bodies into line-range pointers (`#L42-108↦#icx:a1b2c3d4`) unless those lines contain high-surprise diffs or diagnostics.
2. **Compiler & Test Log Skeletonizer (`cargo`, `pytest`, `vitest`, `tsc`, `go test`):**
   - Collapses $N$ passing test lines (`test foo ... ok`) into a single aggregate counter (`✓pass=N`).
   - Preserves 100% of `error[E...]`, `FAILED`, `panicked at`, backtrace frames within workspace paths, and assertion diffs verbatim (`!ERR@...`).
3. **Unified Diff Skeletonizer (`git diff`):**
   - Extracts file path, hunk headers (`@@ -a,b +c,d @@`), removed symbols (`⊖`), added symbols (`⊕`), and stores the full patch in CAS.

---

## 5. Pre-Ingestion Secret Redaction Gate (`SecretRedactor`)

Per Module 60 (Security & Input Validation), **no unredacted secret may ever enter the CAS, AHTL stream, or `/dev/shm` bus**. `SecretRedactor::sanitize()` runs *before* BLAKE3 hashing and replaces matches with `[REDACTED:<kind>:blake3_4B]`:
- API keys & tokens: `sk-[A-Za-z0-9_-]{20,}`, `ghp_[A-Za-z0-9]{36}`, `github_pat_[A-Za-z0-9_]{50,}`, `aim_[A-Za-z0-9]{24,}`, `AKIA[0-9A-Z]{16}`, `xox[baprs]-[A-Za-z0-9-]+`.
- PEM private keys: `-----BEGIN [A-Z ]*PRIVATE KEY----- ... -----END [A-Z ]*PRIVATE KEY-----`.
- Bearer / Authorization headers and `.env` assignments (`*_SECRET=`, `*_TOKEN=`, `*_PASSWORD=`, `*_API_KEY=`).

---

## 6. Test Plan

- **Round-Trip Braille & Packet Parser Property Tests (`proptest`):** Arbitrary byte arrays and `AhtlPacket` structs serialize to AHTL strings and parse back with 100% structural equality.
- **Compression Ratio Gate:** Over a benchmark fixture of 50 KB `cargo test` output and 30 KB Rust source files, `AhtlEncoder` achieves `>= 15x` byte/token reduction while preserving 100% of error codes, function names, and `#icx:<hash>` handles.
- **Redaction Gate:** Every known secret pattern is stripped prior to CAS insertion.
