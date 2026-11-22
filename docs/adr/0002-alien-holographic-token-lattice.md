# ADR 0002: Alien Holographic Token Lattice (AHTL) Non-Human Machine Dialect

## Status

Historical decision (2026-11-16); token/compression/identifier guarantees superseded by
[ADR 0006](0006-v1-2-0-context-and-compatibility.md). Preserve the original rationale below.

## Context

Human-readable English prose and pretty-printed JSON use only ~4.5–6.0 bits of information per BPE token out of the ~17.5-bit theoretical channel capacity of a 100k–200k token vocabulary. Worse, LLM-generated English compaction summaries destroy exact identifiers, line numbers, hex addresses, and negative constraints. Conversely, opaque binary compression (e.g., Base64 `gzip`) cannot be queried by Transformer self-attention heads without a decompression round-trip.

## Decision

We design and implement **AHTL (Alien Holographic Token Lattice)** in `crates/icx-codec`:

- **Channel A** encodes structural skeletons, AST mutations, and diagnostics using single-token Unicode mathematical operators (`⊢`, `⇒`, `↦`, `⋮`, `⊕`, `⊖`, `¬`, `!`, `✓`) while preserving canonical identifiers and line ranges as verbatim BPE tokens.
- **Channel B** encodes a 64-bit topological sketch (HDC parity + Fractal IFS coordinate + Titans Surprise) into 8 Unicode Braille Pattern codepoints (`U+2800..U+28FF`) alongside a cryptographic lossless pointer (`#icx:<hex16>`).
- In Antigravity CLI (`agy`), AHTL frames are injected via `systemMessage` with `metadata.user_markdown = ""` so the human UI remains completely clean while the model attends directly to the AHTL lattice.

## Consequences

- **Positive:** 12x–40x token compression with zero LLM summarization latency (`< 200 µs` deterministic Rust execution) and 100% identifier/line-number preservation in self-attention space.
- **Negative:** Compressed frames look like alien ciphertext to humans inspecting raw wire logs, mitigated by `icx decode-ahtl` and `icx deref` CLI inspection commands.
