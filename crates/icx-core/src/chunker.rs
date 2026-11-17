//! Gear-hash Content-Defined Chunking (`FastCDC`) for deduplicating shared byte spans.

/// Configuration for Content-Defined Chunking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// Splits `data` into content-defined byte range slices `(offset, length)`.
pub fn chunk_bytes(data: &[u8], cfg: ChunkerConfig) -> Vec<(usize, usize)> {
    if data.is_empty() {
        return Vec::new();
    }
    let min_size = cfg.min_size.max(64);
    let max_size = cfg.max_size.max(min_size + 64);
    let bits = cfg.avg_size.max(128).ilog2();
    let mask: u64 = (1u64 << bits) - 1;

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
        let mut pos = offset + min_size;
        let mut fp: u64 = 0;

        while pos < limit {
            fp = (fp << 1).wrapping_add(GEAR_TABLE[data[pos] as usize]);
            if (fp & mask) == 0 {
                pos += 1;
                break;
            }
            pos += 1;
        }

        let len = pos - offset;
        chunks.push((offset, len));
        offset = pos;
    }

    chunks
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
}
