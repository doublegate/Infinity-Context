# Sprint 2.1: AHTL Grammar & Braille-256 Sketch Codec

**Phase:** [Phase 2 — Alien Holographic Token Lattice (`AHTL`) & Structural Skeletonizers](overview.md)  
**Target Crate:** `crates/icx-codec`, `crates/icx-hdc`  
**Specifications:** [docs/alien-holographic-codec.md](../../docs/alien-holographic-codec.md) §§1–3, [docs/ahtl/01-formal-grammar-and-packet-specification.md](../../docs/ahtl/01-formal-grammar-and-packet-specification.md), [docs/ahtl/02-braille-256-spectral-bijection.md](../../docs/ahtl/02-braille-256-spectral-bijection.md)

---

## Tickets

### `T-21-001` — Unicode Braille-256 (`U+2800..U+28FF`) Bijection & Sketch Encoder
- **File:** `crates/icx-codec/src/braille.rs`, `crates/icx-hdc/src/hv.rs`, `crates/icx-hdc/src/fractal.rs`
- **Tasks:**
  - [x] Verify `byte_to_braille(u8) -> char`, `braille_to_byte(char) -> Option<u8>`, and closed-form 3-byte UTF-8 helpers `byte_to_braille_utf8(u8) -> [u8; 3]` and `braille_utf8_to_byte([u8; 3]) -> Option<u8>` across all 256 byte values (`0x00..=0xFF`).
  - [x] Implement `encode_braille_sketch(&[u8; 8]) -> String` and `decode_braille_sketch(&str) -> Option<[u8; 8]>` (`Profile B`), plus `encode_braille_64(&[u8; 64]) -> String` and `decode_braille_64(&str) -> Option<[u8; 64]>` (`Profile A`), and structured `ChannelBHologram` (`B[0..7]`).
  - [x] Implement `sketch_hamming_distance(a: &[u8; 8], b: &[u8; 8]) -> u32`, `spectral_hamming_distance_64(a: &[u8; 64], b: &[u8; 64]) -> u32`, and 1-Lipschitz 64-band spectral projection `Hypervector::spectral_bands_64(&self) -> [u8; 64]` (plus `FractalCoord::encode_fc_hex64` and `Hypervector::encode_hv_hex16`).
- **Acceptance Criteria:**
  - [x] `cargo test -p icx-codec braille` and `cargo test -p icx-hdc` pass with `10,000` random 64-bit and 512-bit round-trip vectors and 1-Lipschitz bound verification.

### `T-21-002` — Canonical `AhtlPacket` Serializer & Parser
- **File:** `crates/icx-codec/src/ahtl.rs`, `crates/icx-codec/src/lib.rs`, `crates/icx-cli/src/main.rs`
- **Tasks:**
  - [x] Implement `PacketKind` (`Δ`, `Ψ`, `Ω`, `Θ`, `Φ`), `ClauseOp` (`⊕`, `⊖`, `¬`, `!`, `✓`, `Neutral`), `AhtlClause` (including causal implication chains `⇒`), `AhtlPacket` (`Profile B` with `slice_hint`, `hologram_tag`, and `#L` subject escaping), `AhtlWirePacketV1` (`Profile A`), `EpochHeader`, `WirePacket`, `AhtlStream`, and normative `ContextCodec` trait.
  - [x] Implement `AhtlPacket::render(&self) -> String`, `AhtlPacket::parse(encoded: &str) -> Result<Self, CodecError>`, `AhtlWirePacketV1::render(&self) -> String`, and `AhtlWirePacketV1::parse(input: &str) -> Result<Self, CodecError>`.
  - [x] Handle optional `#L<start>-<end>` line ranges, optional `↦#icx:<12|16-hex>` dereference handles with `(slice_hint)`, and malformed-input error cases (`CodecError`).
- **Acceptance Criteria:**
  - [x] `cargo test -p icx-codec ahtl` passes round-trip serialization/parsing and malformed-packet rejection tests.
