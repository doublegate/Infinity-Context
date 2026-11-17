//! Unicode Braille-256 (`U+2800..U+28FF`) bijection for encoding 64-bit topological holograms
//! into 8 BPE-friendly codepoints in Channel B of an AHTL packet.

/// Maps an 8-bit byte `b` (`0..=255`) to its corresponding Unicode Braille Pattern (`U+2800..U+28FF`).
#[inline]
pub const fn byte_to_braille(b: u8) -> char {
    // SAFETY/INVARIANT: 0x2800..=0x28FF are valid Unicode scalar values.
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

/// Encodes an 8-byte topological sketch into an 8-character Braille-256 string.
pub fn encode_braille_sketch(bytes: &[u8; 8]) -> String {
    let mut s = String::with_capacity(24);
    for &b in bytes {
        s.push(byte_to_braille(b));
    }
    s
}

/// Decodes an 8-character Braille-256 string back into an 8-byte sketch.
pub fn decode_braille_sketch(s: &str) -> Option<[u8; 8]> {
    let mut out = [0u8; 8];
    let mut count = 0usize;
    for c in s.chars() {
        if count >= 8 {
            return None;
        }
        out[count] = braille_to_byte(c)?;
        count += 1;
    }
    if count == 8 {
        Some(out)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn braille_256_bijection_covers_all_bytes() {
        for b in 0u8..=255u8 {
            let ch = byte_to_braille(b);
            assert_eq!(braille_to_byte(ch), Some(b));
        }
        let sample = [0x00, 0x42, 0x7F, 0x80, 0xAA, 0xCC, 0xFE, 0xFF];
        let enc = encode_braille_sketch(&sample);
        assert_eq!(enc.chars().count(), 8);
        assert_eq!(decode_braille_sketch(&enc), Some(sample));
    }
}
