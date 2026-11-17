# Sprint 1 — AHTL Grammar, Parser & Braille-256 Bijection

**Phase:** Phase 2 — Alien Holographic Token Lattice (`AHTL`)  
**Sprint Goal:** Implement the core `AhtlPacket` serializer, parser, and Braille-256 (`U+2800..U+28FF`) holographic sketch codec in `crates/icx-codec`.  
**Estimated Duration:** 1 week

## Tickets

### T-21-001 — Implement Braille-256 (`U+2800..U+28FF`) Byte-to-Glyph Bijection

**Description:** Implement branchless `byte_to_braille`, `braille_to_byte`, `encode_braille_sketch(&[u8; 8])`, and `decode_braille_sketch(&str)` in `crates/icx-codec/src/braille.rs`.

**Acceptance Criteria:**
- [x] Exhaustive test over `0u8..=255u8` verifies `braille_to_byte(byte_to_braille(b)) == Some(b)`.
- [x] 8-byte sketches round-trip to 8-codepoint Braille strings.

**Dependencies:** T-11-002  
**Reference:** `docs/alien-holographic-codec.md` §3.2  
**Estimated Complexity:** S

---

### T-21-002 — Implement `AhtlPacket`, `AhtlClause` & Canonical Serializer/Parser

**Description:** Implement `AhtlPacket::render()` and `AhtlPacket::parse()` supporting `Δ` (code delta), `Ψ` (tool stub), `Ω` (invariant), `Θ` (telepathy), and `Φ` (folded branch) packets with `⊢`, `⋮`, `↦`, `⊕`, `⊖`, `¬`, `!`, `✓` operators.

**Acceptance Criteria:**
- [x] Every packet kind, line range, clause operator, `#icx:<hex16>` handle, and Braille sketch round-trips losslessly through `render()` -> `parse()`.
- [x] Unit tests cover complex multi-clause packets and malformed input error reporting.

**Dependencies:** T-21-001  
**Reference:** `docs/alien-holographic-codec.md` §2–3; `docs/data-model.md` §3  
**Estimated Complexity:** M
