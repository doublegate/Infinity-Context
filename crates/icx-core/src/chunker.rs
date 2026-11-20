//! Normalized Two-Threshold Gear-Hash Content-Defined Chunking (`FastCDC`) for deduplicating
//! shared byte spans and computing streaming 1-based line-offset spans.

use serde::{Deserialize, Serialize};

/// Configuration for Content-Defined Chunking.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkerConfig {
    pub min_size: usize,
    pub avg_size: usize,
    pub max_size: usize,
}

impl Default for ChunkerConfig {
    fn default() -> Self {
        Self {
            min_size: 2048,
            avg_size: 8192,
            max_size: 65536,
        }
    }
}

/// A content-defined chunk span paired with its 1-based inclusive line numbers `[start_line, end_line]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkLineSpan {
    pub offset: usize,
    pub len: usize,
    pub start_line: u32,
    pub end_line: u32,
}

/// Deterministic 256-entry gear table generated via SplitMix64 from a fixed seed.
const fn build_gear_table() -> [u64; 256] {
    let mut table = [0u64; 256];
    let mut state: u64 = 0x4943_585F_4641_5354; // "ICX_FAST"
    let mut i = 0;
    while i < 256 {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        table[i] = z ^ (z >> 31);
        i += 1;
    }
    table
}

const GEAR_TABLE: [u64; 256] = build_gear_table();

/// FastCDC-style mask with `ones` set bits spread evenly over bit positions 63..=16
/// (Xia et al., USENIX ATC '16). With Gear accumulation `fp = (fp << 1) + G[b]`, bit `k` depends
/// on the last `k + 1` bytes, so low-bit masks only see ~14 bytes of context; spread high bits
/// widen the effective window to ~48 bytes. Measured on this repository's sources: mean chunk
/// 13.2 KiB -> 10.6 KiB (target 8 KiB), size s.d. 8.0 -> 5.1 KiB, bytes still deduplicated after a
/// 1-byte insertion 99.33% -> 99.60% (ICX-AUD-027).
const fn spread_mask(ones: u32) -> u64 {
    let ones = if ones == 0 { 1 } else { ones };
    let mut mask = 0u64;
    let mut k = 0;
    while k < ones {
        mask |= 1u64 << (63 - (k * 47 / ones));
        k += 1;
    }
    mask
}

/// Splits `data` into content-defined byte range slices `(offset, length)` using Normalized FastCDC
/// (stricter mask `mask_s` before `avg_size`, relaxed mask `mask_l` from `avg_size` to `max_size`).
pub fn chunk_bytes(data: &[u8], cfg: ChunkerConfig) -> Vec<(usize, usize)> {
    if data.is_empty() {
        return Vec::new();
    }
    let min_size = cfg.min_size.max(64);
    let max_size = cfg.max_size.max(min_size + 64);
    let avg_size = cfg.avg_size.clamp(min_size, max_size);
    let bits = avg_size.max(128).ilog2();
    let mask_s: u64 = spread_mask((bits + 1).min(47));
    let mask_l: u64 = spread_mask(bits.saturating_sub(1).max(1));

    let mut chunks = Vec::new();
    let mut offset = 0usize;
    let total = data.len();

    while offset < total {
        let remaining = total - offset;
        if remaining <= min_size {
            chunks.push((offset, remaining));
            break;
        }

        let limit = offset + remaining.min(max_size);
        let normal_pivot = (offset + avg_size).min(limit);
        let mut pos = offset + min_size;
        let mut fp: u64 = 0;
        let mut cut_found = false;

        // Phase 1: [min_size .. avg_size) with stricter mask_s (bits + 1 ones)
        while pos < normal_pivot {
            fp = (fp << 1).wrapping_add(GEAR_TABLE[data[pos] as usize]);
            if (fp & mask_s) == 0 {
                pos += 1;
                cut_found = true;
                break;
            }
            pos += 1;
        }

        // Phase 2: [avg_size .. max_size) with relaxed mask_l (bits - 1 ones)
        if !cut_found {
            while pos < limit {
                fp = (fp << 1).wrapping_add(GEAR_TABLE[data[pos] as usize]);
                if (fp & mask_l) == 0 {
                    pos += 1;
                    break;
                }
                pos += 1;
            }
        }

        let len = pos - offset;
        chunks.push((offset, len));
        offset = pos;
    }

    chunks
}

/// Computes `(start_line, end_line, next_current_line)` for a single chunk byte slice.
#[inline]
pub(crate) fn advance_chunk_line_span(slice: &[u8], current_line: u32) -> (u32, u32, u32) {
    let start_line = current_line;
    let newlines = slice.iter().filter(|&&b| b == b'\n').count() as u32;
    let next_line = current_line.saturating_add(newlines);
    let end_line = if slice.last() == Some(&b'\n') {
        next_line.saturating_sub(1).max(start_line)
    } else {
        next_line
    };
    (start_line, end_line, next_line)
}

/// Splits `data` using Normalized FastCDC (`chunk_bytes`) and indexes the exact 1-based
/// `[start_line, end_line]` boundaries across the resulting chunk spans.
pub fn chunk_bytes_with_lines(data: &[u8], cfg: ChunkerConfig) -> Vec<ChunkLineSpan> {
    let spans = chunk_bytes(data, cfg);
    let mut out = Vec::with_capacity(spans.len());
    let mut current_line: u32 = 1;

    for (offset, len) in spans {
        let slice = &data[offset..offset + len];
        let (start_line, end_line, next_line) = advance_chunk_line_span(slice, current_line);
        current_line = next_line;
        out.push(ChunkLineSpan {
            offset,
            len,
            start_line,
            end_line,
        });
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fastcdc_reconstructs_exact_bytes_and_realigns_after_insert() {
        let mut payload = Vec::with_capacity(64 * 1024);
        for i in 0..8192u32 {
            payload.extend_from_slice(&i.to_le_bytes());
            payload.extend_from_slice(b"line_entry\n");
        }
        let cfg = ChunkerConfig {
            min_size: 512,
            avg_size: 2048,
            max_size: 8192,
        };
        let spans = chunk_bytes(&payload, cfg);
        assert!(spans.len() > 8);

        let mut rebuilt = Vec::new();
        for &(off, len) in &spans {
            assert!(len <= cfg.max_size);
            rebuilt.extend_from_slice(&payload[off..off + len]);
        }
        assert_eq!(rebuilt, payload);

        // Insert 17 bytes near the beginning and verify trailing chunks realign identically
        let mut modified = Vec::with_capacity(payload.len() + 17);
        modified.extend_from_slice(b"INSERTED_HEADER\n!");
        modified.extend_from_slice(&payload);
        let mod_spans = chunk_bytes(&modified, cfg);

        let orig_last = spans.last().copied().unwrap();
        let mod_last = mod_spans.last().copied().unwrap();
        assert_eq!(
            &payload[orig_last.0..orig_last.0 + orig_last.1],
            &modified[mod_last.0..mod_last.0 + mod_last.1]
        );
    }

    #[test]
    fn chunk_bytes_with_lines_tracks_exact_line_boundaries() {
        let mut text = String::new();
        for i in 1..=500 {
            text.push_str(&format!(
                "line_{i:04}: content payload for testing fastcdc\n"
            ));
        }
        let cfg = ChunkerConfig {
            min_size: 256,
            avg_size: 1024,
            max_size: 4096,
        };
        let line_spans = chunk_bytes_with_lines(text.as_bytes(), cfg);
        assert!(line_spans.len() > 4);
        assert_eq!(line_spans.first().unwrap().start_line, 1);
        assert_eq!(line_spans.last().unwrap().end_line, 500);

        let def_cfg = ChunkerConfig::default();
        assert_eq!(def_cfg.avg_size, 8192);
        assert_eq!(build_gear_table(), GEAR_TABLE);
        assert_eq!(chunk_bytes(&[], def_cfg).len(), 0);
        let small = chunk_bytes(b"short_payload_no_newline", def_cfg);
        assert_eq!(small, vec![(0, 24)]);
        let small_lines = chunk_bytes_with_lines(b"line1\nline2_no_nl", def_cfg);
        assert_eq!(small_lines.len(), 1);
        assert_eq!(small_lines[0].start_line, 1);
        assert_eq!(small_lines[0].end_line, 2);
    }
}
