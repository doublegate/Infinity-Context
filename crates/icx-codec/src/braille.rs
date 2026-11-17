//! Unicode Braille-256 (`U+2800..U+28FF`) bijection for encoding 64-bit (`8-byte` Profile B)
//! and 512-bit (`64-byte` Profile A) topological holograms into BPE-isolated codepoints.
//!
//! Reference: `docs/ahtl/02-braille-256-spectral-bijection.md`.

/// Structural flag bit 0 (`0x01`): Packet contains a compiler/runtime error diagnostic.
pub const FLAG_HAS_ERROR: u8 = 1 << 0;
/// Structural flag bit 1 (`0x02`): Packet contains a panic / assertion / crash event.
pub const FLAG_HAS_PANIC: u8 = 1 << 1;
/// Structural flag bit 2 (`0x04`): Packet encodes a unified `git diff` or file mutation.
pub const FLAG_HAS_DIFF: u8 = 1 << 2;
/// Structural flag bit 3 (`0x08`): Packet represents a folded sub-trajectory branch (`Φ`) or stub.
pub const FLAG_IS_FOLDED: u8 = 1 << 3;
/// Structural flag bit 4 (`0x10`): Packet is sealed inside an immutable `CacheEpoch` or cold beacon.
pub const FLAG_IS_EPOCH: u8 = 1 << 4;
/// Structural flag bit 5 (`0x20`): Packet contains causal implication chains (`⇒`).
pub const FLAG_HAS_CAUSAL: u8 = 1 << 5;
/// Structural flag bit 6 (`0x40`): Pre-ingestion `SecretRedactor` scrubbed one or more secrets.
pub const FLAG_IS_REDACTED: u8 = 1 << 6;
/// Structural flag bit 7 (`0x80`): Packet is pinned in high-resolution active memory (`S_eff >= 0.70`).
pub const FLAG_IS_PINNED: u8 = 1 << 7;

/// Structured 64-bit (`8-byte`) Channel B hologram payload (`B[0..7]`) for `AHTL` Profile B
/// (`docs/ahtl/02-braille-256-spectral-bijection.md` §4.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelBHologram {
    /// `B[0..3]`: 32-bit rotational XOR parity fold of the 10,240-bit `Hypervector` (`sketch_u32`).
    pub hdc_sketch_u32: u32,
    /// `B[4..5]`: 16-bit Fractal Chaos-Game orthant signature (`quadrant_signature_u16`).
    pub fractal_quadrant_u16: u16,
    /// `B[6]`: 8-bit quantized Titans effective surprise (`round(255 * S_eff)`).
    pub surprise_q8: u8,
    /// `B[7]`: 8-bit structural flag bitfield (`FLAG_*` constants).
    pub structural_flags: u8,
}

impl ChannelBHologram {
    /// Returns `true` if all bits in `flag` are set in `self.structural_flags`.
    #[inline]
    pub const fn has_flag(self, flag: u8) -> bool {
        (self.structural_flags & flag) == flag
    }
    /// Packs the structured fields into a canonical big-endian 8-byte array `B[0..7]`.
    #[inline]
    pub const fn to_bytes(self) -> [u8; 8] {
        let s = self.hdc_sketch_u32.to_be_bytes();
        let q = self.fractal_quadrant_u16.to_be_bytes();
        [
            s[0],
            s[1],
            s[2],
            s[3],
            q[0],
            q[1],
            self.surprise_q8,
            self.structural_flags,
        ]
    }

    /// Unpacks a canonical big-endian 8-byte Channel B array `B[0..7]` into structured fields.
    #[inline]
    pub const fn from_bytes(b: [u8; 8]) -> Self {
        let hdc_sketch_u32 = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        let fractal_quadrant_u16 = u16::from_be_bytes([b[4], b[5]]);
        Self {
            hdc_sketch_u32,
            fractal_quadrant_u16,
            surprise_q8: b[6],
            structural_flags: b[7],
        }
    }
}

/// Maps an 8-bit byte `b` (`0..=255`) to its corresponding Unicode Braille Pattern (`U+2800..U+28FF`).
#[inline]
pub const fn byte_to_braille(b: u8) -> char {
    match char::from_u32(0x2800 + (b as u32)) {
        Some(c) => c,
        None => '\u{2800}',
    }
}

/// Maps a Unicode Braille Pattern character (`U+2800..U+28FF`) back to its 8-bit byte value.
#[inline]
pub const fn braille_to_byte(c: char) -> Option<u8> {
    let code = c as u32;
    if code >= 0x2800 && code <= 0x28FF {
        Some((code - 0x2800) as u8)
    } else {
        None
    }
}

/// Closed-form branchless 3-byte UTF-8 encoder for a single Braille-256 byte `b`:
/// `UTF8(U+2800 + b) = [0xE2, 0xA0 | (b >> 6), 0x80 | (b & 0x3F)]`.
#[inline]
pub const fn byte_to_braille_utf8(b: u8) -> [u8; 3] {
    [0xE2, 0xA0 | (b >> 6), 0x80 | (b & 0x3F)]
}

/// Closed-form branchless 3-byte UTF-8 decoder for a single Braille-256 codepoint (`E2 A0..=A3 80..=BF`).
#[inline]
pub const fn braille_utf8_to_byte(utf8: [u8; 3]) -> Option<u8> {
    if utf8[0] == 0xE2 && (utf8[1] & 0xFC) == 0xA0 && (utf8[2] & 0xC0) == 0x80 {
        Some(((utf8[1] & 0x03) << 6) | (utf8[2] & 0x3F))
    } else {
        None
    }
}

/// Encodes an 8-byte topological sketch into an 8-character (`24-byte UTF-8`) Braille-256 string.
pub fn encode_braille_sketch(bytes: &[u8; 8]) -> String {
    let mut s = String::with_capacity(24);
    for &b in bytes {
        s.push(byte_to_braille(b));
    }
    s
}

/// Decodes an 8-character (`24-byte UTF-8`) Braille-256 string back into an 8-byte sketch.
pub fn decode_braille_sketch(s: &str) -> Option<[u8; 8]> {
    let raw = s.as_bytes();
    if raw.len() != 24 {
        return None;
    }
    let mut out = [0u8; 8];
    for (i, chunk) in raw.as_chunks::<3>().0.iter().enumerate() {
        out[i] = braille_utf8_to_byte(*chunk)?;
    }
    Some(out)
}

/// Computes the exact 64-bit Hamming distance between two 8-byte Braille sketches via `POPCNT`.
#[inline]
pub const fn sketch_hamming_distance(a: &[u8; 8], b: &[u8; 8]) -> u32 {
    let wa = u64::from_le_bytes(*a);
    let wb = u64::from_le_bytes(*b);
    (wa ^ wb).count_ones()
}

/// Encodes a 64-byte (`512-bit`) Profile A spectral projection into a 64-character (`192-byte UTF-8`)
/// Braille-256 string.
pub fn encode_braille_64(bytes: &[u8; 64]) -> String {
    let mut s = String::with_capacity(192);
    for &b in bytes {
        s.push(byte_to_braille(b));
    }
    s
}

/// Decodes a 64-character (`192-byte UTF-8`) Braille-256 string back into a 64-byte spectral array.
pub fn decode_braille_64(s: &str) -> Option<[u8; 64]> {
    let raw = s.as_bytes();
    if raw.len() != 192 {
        return None;
    }
    let mut out = [0u8; 64];
    for (i, chunk) in raw.as_chunks::<3>().0.iter().enumerate() {
        out[i] = braille_utf8_to_byte(*chunk)?;
    }
    Some(out)
}

/// Computes the exact 512-bit Hamming distance between two 64-byte Profile A spectral sketches.
#[inline]
pub fn spectral_hamming_distance_64(a: &[u8; 64], b: &[u8; 64]) -> u32 {
    let mut dist = 0u32;
    for (ca, cb) in a.as_chunks::<8>().0.iter().zip(b.as_chunks::<8>().0.iter()) {
        let wa = u64::from_le_bytes(*ca);
        let wb = u64::from_le_bytes(*cb);
        dist += (wa ^ wb).count_ones();
    }
    dist
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn braille_256_bijection_covers_all_bytes_and_utf8_triples() {
        for b in 0u8..=255u8 {
            let ch = byte_to_braille(b);
            assert_eq!(braille_to_byte(ch), Some(b));

            let utf8 = byte_to_braille_utf8(b);
            let mut ch_buf = [0u8; 4];
            let encoded_str = ch.encode_utf8(&mut ch_buf);
            assert_eq!(encoded_str.as_bytes(), &utf8);
            assert_eq!(braille_utf8_to_byte(utf8), Some(b));
        }
        assert_eq!(braille_to_byte('A'), None);
        assert_eq!(braille_utf8_to_byte([0xE1, 0xA0, 0x80]), None);
        assert_eq!(braille_utf8_to_byte([0xE2, 0xA4, 0x80]), None);
        assert_eq!(braille_utf8_to_byte([0xE2, 0xA0, 0x40]), None);

        let sample = [0x00, 0x42, 0x7F, 0x80, 0xAA, 0xCC, 0xFE, 0xFF];
        let enc = encode_braille_sketch(&sample);
        assert_eq!(enc.chars().count(), 8);
        assert_eq!(enc.len(), 24);
        assert_eq!(decode_braille_sketch(&enc), Some(sample));
        assert_eq!(decode_braille_sketch("⠀⠀"), None);
        assert_eq!(decode_braille_sketch("⠀⠀⠀⠀⠀⠀⠀⠀⠀"), None);
        assert_eq!(decode_braille_sketch("AAAAAAAAAAAAAAAAAAAAAAAA"), None);
    }

    #[test]
    fn braille_10000_random_8byte_and_64byte_round_trip_and_hamming_isometry() {
        let mut rng = 0xCAFE_BABE_1234_5678u64;
        let mut next_u64 = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };

        for _ in 0..10_000 {
            let w1 = next_u64();
            let w2 = next_u64();
            let b1 = w1.to_le_bytes();
            let b2 = w2.to_le_bytes();
            let s1 = encode_braille_sketch(&b1);
            let s2 = encode_braille_sketch(&b2);
            let d1 = decode_braille_sketch(&s1).expect("valid 8-byte braille");
            let d2 = decode_braille_sketch(&s2).expect("valid 8-byte braille");
            assert_eq!(d1, b1);
            assert_eq!(d2, b2);
            assert_eq!(sketch_hamming_distance(&d1, &d2), (w1 ^ w2).count_ones());
        }

        for _ in 0..500 {
            let mut arr1 = [0u8; 64];
            let mut arr2 = [0u8; 64];
            let mut expected_dist = 0u32;
            for chunk_idx in 0..8 {
                let u1 = next_u64();
                let u2 = next_u64();
                expected_dist += (u1 ^ u2).count_ones();
                arr1[chunk_idx * 8..(chunk_idx + 1) * 8].copy_from_slice(&u1.to_le_bytes());
                arr2[chunk_idx * 8..(chunk_idx + 1) * 8].copy_from_slice(&u2.to_le_bytes());
            }
            let enc1 = encode_braille_64(&arr1);
            assert_eq!(enc1.len(), 192);
            assert_eq!(enc1.chars().count(), 64);
            let dec1 = decode_braille_64(&enc1).expect("valid 64-byte braille");
            assert_eq!(dec1, arr1);
            assert_eq!(spectral_hamming_distance_64(&dec1, &arr2), expected_dist);
        }
        assert_eq!(decode_braille_64("short"), None);
        assert_eq!(decode_braille_64(&"A".repeat(192)), None);
    }

    #[test]
    fn channel_b_hologram_packs_and_unpacks_losslessly() {
        let holo = ChannelBHologram {
            hdc_sketch_u32: 0xDEAD_BEEF,
            fractal_quadrant_u16: 0xCAFE,
            surprise_q8: 214,
            structural_flags: FLAG_HAS_ERROR | FLAG_HAS_PANIC | FLAG_IS_PINNED,
        };
        let raw = holo.to_bytes();
        let unpacked = ChannelBHologram::from_bytes(raw);
        assert_eq!(unpacked, holo);
        assert!(unpacked.has_flag(FLAG_IS_PINNED));
        assert!(unpacked.has_flag(FLAG_HAS_ERROR));
        assert!(!unpacked.has_flag(FLAG_IS_REDACTED));
    }
}
