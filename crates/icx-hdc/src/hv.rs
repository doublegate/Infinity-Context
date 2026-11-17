//! 10,240-bit Binary/Bipolar Hyperdimensional Computing (`HDC/HRR`) algebra and Infini-attention
//! Delta-Rule majority bundler (`arXiv:2404.07143`).

pub const HV_WORDS: usize = 160;
pub const HV_BITS: usize = HV_WORDS * 64; // 10,240 bits

/// 10,240-bit binary hypervector aligned to 64-byte cache lines / AVX-512 ZMM registers.
#[repr(C, align(64))]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Hypervector {
    pub words: [u64; HV_WORDS],
}

impl Default for Hypervector {
    fn default() -> Self {
        Self::zero()
    }
}

impl Hypervector {
    /// Returns an all-zero hypervector.
    #[inline]
    pub const fn zero() -> Self {
        Self {
            words: [0u64; HV_WORDS],
        }
    }

    /// Deterministically generates a quasi-orthogonal 10,240-bit hypervector from `(domain, symbol)`
    /// using `BLAKE3` in Extendable Output Function (`XOF`) mode.
    pub fn from_seed(domain: &str, symbol: &str) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"ICX_HDC_10240_V1:");
        hasher.update(domain.as_bytes());
        hasher.update(b"::");
        hasher.update(symbol.as_bytes());
        let mut xof = hasher.finalize_xof();

        let mut raw = [0u8; HV_WORDS * 8];
        xof.fill(&mut raw);

        let mut words = [0u64; HV_WORDS];
        for (i, chunk) in raw.as_chunks::<8>().0.iter().enumerate() {
            words[i] = u64::from_le_bytes(*chunk);
        }
        Self { words }
    }

    /// Encodes a text string into a composite hypervector by binding token unigrams and
    /// permutation-shifted bigrams.
    pub fn encode_text(text: &str) -> Self {
        let mut bundler = HdcBundler::new();
        let mut prev_hv: Option<Self> = None;

        for raw_tok in
            text.split(|c: char| !c.is_alphanumeric() && c != '_' && c != ':' && c != '/')
        {
            let tok = raw_tok.trim();
            if tok.len() < 2 {
                continue;
            }
            let lower = tok.to_ascii_lowercase();
            let hv = Self::from_seed("tok", &lower);
            bundler.add(&hv);

            if let Some(prev) = prev_hv {
                let bigram = prev.permute(1).bind(&hv);
                bundler.add(&bigram);
            }
            prev_hv = Some(hv);
        }

        bundler.finalize()
    }

    /// Bitwise XOR binding ($\oplus$). Self-inverse and Hamming-distance preserving.
    #[inline]
    pub fn bind(&self, other: &Self) -> Self {
        let mut out = [0u64; HV_WORDS];
        for i in 0..HV_WORDS {
            out[i] = self.words[i] ^ other.words[i];
        }
        Self { words: out }
    }

    /// Circular permutation ($\Pi^k$) rotating words and bits for sequence order encoding.
    #[inline]
    pub fn permute(&self, shift: usize) -> Self {
        let word_shift = shift % HV_WORDS;
        let bit_shift = ((shift % 63) + 1) as u32;
        let mut out = [0u64; HV_WORDS];
        for i in 0..HV_WORDS {
            let src = (i + HV_WORDS - word_shift) % HV_WORDS;
            out[i] = self.words[src].rotate_left(bit_shift);
        }
        Self { words: out }
    }

    /// Exact Hamming distance computed via hardware `POPCNT` / auto-vectorized `VPOPCNTDQ`.
    #[inline]
    pub fn hamming_distance(&self, other: &Self) -> u32 {
        let mut dist = 0u32;
        for i in 0..HV_WORDS {
            dist += (self.words[i] ^ other.words[i]).count_ones();
        }
        dist
    }

    /// Normalized associative similarity in `[0.0, 1.0]` where `0.0` is orthogonal (`d >= 5120`)
    /// and `1.0` is identical (`d == 0`).
    #[inline]
    pub fn similarity(&self, other: &Self) -> f32 {
        let d = self.hamming_distance(other).min(5120);
        1.0 - ((d as f32) / 5120.0)
    }

    /// Folds the 10,240-bit hypervector into a compact 32-bit parity sketch for Braille-256 Channel B.
    #[inline]
    pub fn sketch_u32(&self) -> u32 {
        let mut acc = 0u64;
        for (i, &w) in self.words.iter().enumerate() {
            acc ^= w.rotate_left((i as u32) % 64);
        }
        ((acc >> 32) as u32) ^ (acc as u32)
    }

    /// Extracts a 256-bit (`[u64; 4]`) compact sketch for `/dev/shm` slot headers.
    #[inline]
    pub fn sketch_256(&self) -> [u64; 4] {
        let mut out = [0u64; 4];
        for (i, &w) in self.words.iter().enumerate() {
            out[i % 4] ^= w.rotate_left((i as u32) % 64);
        }
        out
    }
}

/// Infini-attention Delta-Rule (`arXiv:2404.07143`) majority-vote hypervector bundler.
/// Prevents repetitive boilerplate from saturating associative memory.
#[derive(Clone, Debug)]
pub struct HdcBundler {
    acc: [i16; HV_BITS],
    readout: Hypervector,
    count: usize,
}

impl Default for HdcBundler {
    fn default() -> Self {
        Self::new()
    }
}

impl HdcBundler {
    /// Creates a new empty `HdcBundler`.
    pub fn new() -> Self {
        Self {
            acc: [0i16; HV_BITS],
            readout: Hypervector::zero(),
            count: 0,
        }
    }

    /// Adds a hypervector using the Infini-attention Delta Rule: vectors already strongly present
    /// in the bundle (`similarity >= 0.85`) receive zero delta weight so repetitive boilerplate
    /// never saturates the accumulator, while novel/orthogonal vectors receive full weight.
    pub fn add(&mut self, hv: &Hypervector) {
        let weight: i16 = if self.count == 0 {
            16
        } else {
            let sim = self.readout.similarity(hv);
            if sim >= 0.85 {
                return;
            }
            let novelty = (1.0 - sim).clamp(0.0, 1.0);
            ((novelty * 16.0).round() as i16).max(1)
        };
        self.add_weighted(hv, weight);
    }

    /// Adds a hypervector with an explicit integer weight (`1..=64`).
    pub fn add_weighted(&mut self, hv: &Hypervector, weight: i16) {
        let w = weight.clamp(1, 64);
        for word_idx in 0..HV_WORDS {
            let word = hv.words[word_idx];
            let base = word_idx * 64;
            let mut out_word = 0u64;
            for bit in 0..64 {
                let delta = if ((word >> bit) & 1) != 0 { w } else { -w };
                let val = self.acc[base + bit].saturating_add(delta);
                self.acc[base + bit] = val;
                if val >= 0 {
                    out_word |= 1u64 << bit;
                }
            }
            self.readout.words[word_idx] = out_word;
        }
        self.count += 1;
    }

    /// Returns the current thresholded 10,240-bit `Hypervector` readout.
    #[inline]
    pub const fn finalize(&self) -> Hypervector {
        self.readout
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hypervector_orthogonality_and_xor_self_inverse() {
        let a = Hypervector::from_seed("file", "crates/icx-core/src/dag.rs");
        let b = Hypervector::from_seed("file", "crates/icx-hdc/src/hv.rs");
        assert!(
            a.similarity(&b) < 0.05,
            "unrelated seeds must be quasi-orthogonal"
        );
        let bound = a.bind(&b);
        assert_eq!(bound.bind(&b), a, "XOR binding must be self-inverse");
    }

    #[test]
    fn delta_rule_bundler_prevents_boilerplate_saturation() {
        let mut bundler = HdcBundler::new();
        let boilerplate = Hypervector::from_seed("log", "test_ok_boilerplate");
        let anomaly = Hypervector::from_seed("err", "panic_align_offset_412");

        for _ in 0..50 {
            bundler.add(&boilerplate);
        }
        bundler.add_weighted(&anomaly, 48);

        let memory = bundler.finalize();
        assert!(
            memory.similarity(&anomaly) > 0.25,
            "anomaly must remain retrievable despite 50x boilerplate repetition"
        );
    }
}
