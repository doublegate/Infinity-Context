//! 10,240-bit Binary/Bipolar Hyperdimensional Computing (`HDC/HRR`) algebra and Infini-attention
//! Delta-Rule majority bundler (`arXiv:2404.07143`).

use core::ops::{BitXor, BitXorAssign};
use std::collections::BTreeSet;

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

impl BitXor for Hypervector {
    type Output = Self;

    #[inline]
    fn bitxor(self, rhs: Self) -> Self::Output {
        self.bind(&rhs)
    }
}

impl BitXor for &Hypervector {
    type Output = Hypervector;

    #[inline]
    fn bitxor(self, rhs: Self) -> Self::Output {
        self.bind(rhs)
    }
}

impl BitXorAssign<&Self> for Hypervector {
    #[inline]
    fn bitxor_assign(&mut self, rhs: &Self) {
        for i in 0..HV_WORDS {
            self.words[i] ^= rhs.words[i];
        }
    }
}

impl BitXorAssign for Hypervector {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Self) {
        *self ^= &rhs;
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

    /// Deterministically generates a 10,240-bit hypervector with injective length-prefixed tuple
    /// framing (`docs/ahtl/04-hyperdimensional-10240-bit-lattice.md` §3.1), preventing collisions
    /// even when user-supplied `domain` strings contain `"::"`.
    pub fn from_seed_framed(domain: &str, symbol: &str) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"ICX_HDC_10240_FRAMED_V1:");
        hasher.update(&(domain.len() as u64).to_le_bytes());
        hasher.update(domain.as_bytes());
        hasher.update(&(symbol.len() as u64).to_le_bytes());
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

    /// Binds a semantic role (`"role"`) to a filler token (`"tok"`) via XOR (`R \oplus F`).
    #[inline]
    pub fn bind_role_filler(role: &str, filler: &str) -> Self {
        let r = Self::from_seed("role", &role.to_ascii_lowercase());
        let f = Self::from_seed("tok", &filler.to_ascii_lowercase());
        r.bind(&f)
    }

    /// Encodes an ordered sequence of hypervectors $(H_0, H_1, \dots, H_{k-1})$ using
    /// non-commutative position-shifted XOR binding:
    /// $$S(H_0, \dots, H_{k-1}) = \bigoplus_{i=0}^{k-1} \Pi_{k-1-i}(H_i)$$
    pub fn encode_sequence(items: &[Self]) -> Self {
        if items.is_empty() {
            return Self::zero();
        }
        let k = items.len();
        let mut acc = Self::zero();
        for (i, item) in items.iter().enumerate() {
            let shift = k - 1 - i;
            let term = if shift == 0 {
                *item
            } else {
                item.permute(shift)
            };
            acc ^= &term;
        }
        acc
    }

    /// Deterministically generates a 10,240-bit hypervector from a 64-bit integer seed using
    /// the bijective Stafford `Mix13` (`SplitMix64`) recurrence (`docs/ahtl/04-hyperdimensional-10240-bit-lattice.md` §3.2).
    #[inline]
    pub const fn from_u64_seed(seed: u64) -> Self {
        let mut words = [0u64; HV_WORDS];
        let mut s = seed;
        let mut i = 0;
        while i < HV_WORDS {
            s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = (s ^ (s >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            words[i] = z ^ (z >> 31);
            i += 1;
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

    /// Unbinds a role or key hypervector from a bound pair or bundle (`(R \oplus F) \oplus R = F`).
    #[inline]
    pub fn unbind(&self, key: &Self) -> Self {
        self.bind(key)
    }

    /// Circular permutation ($\Pi_s$) rotating words and bits for sequence order encoding.
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

    /// Exact inverse permutation ($\Pi_s^{-1}$) satisfying `hv.permute(s).inverse_permute(s) == hv`.
    #[inline]
    pub fn inverse_permute(&self, shift: usize) -> Self {
        let word_shift = shift % HV_WORDS;
        let bit_shift = ((shift % 63) + 1) as u32;
        let mut out = [0u64; HV_WORDS];
        for i in 0..HV_WORDS {
            let src = (i + word_shift) % HV_WORDS;
            out[i] = self.words[src].rotate_right(bit_shift);
        }
        Self { words: out }
    }

    /// Total Hamming weight ($\operatorname{wt}(H) \in [0, 10240]$).
    #[inline]
    pub const fn popcount(&self) -> u32 {
        let mut ones = 0u32;
        let mut i = 0;
        while i < HV_WORDS {
            ones += self.words[i].count_ones();
            i += 1;
        }
        ones
    }

    /// Exact Hamming distance computed via hardware `POPCNT` / auto-vectorized `VPOPCNTDQ`.
    #[inline]
    pub fn hamming_distance(&self, other: &Self) -> u32 {
        self.hamming_distance_simd(other)
    }

    /// Four-way unrolled 160-word `XOR + POPCNT` distance kernel breaking loop-carried dependency
    /// chains across superscalar execution ports (`docs/ahtl/08-v1-reference-implementation-blueprint.md` §3).
    #[inline]
    pub const fn hamming_distance_simd(&self, other: &Self) -> u32 {
        let mut d0 = 0u32;
        let mut d1 = 0u32;
        let mut d2 = 0u32;
        let mut d3 = 0u32;
        let mut i = 0;
        while i < HV_WORDS {
            d0 += (self.words[i] ^ other.words[i]).count_ones();
            d1 += (self.words[i + 1] ^ other.words[i + 1]).count_ones();
            d2 += (self.words[i + 2] ^ other.words[i + 2]).count_ones();
            d3 += (self.words[i + 3] ^ other.words[i + 3]).count_ones();
            i += 4;
        }
        d0 + d1 + d2 + d3
    }

    /// Normalized associative similarity in `[0.0, 1.0]` where `0.0` is orthogonal (`d >= 5120`)
    /// and `1.0` is identical (`d == 0`).
    #[inline]
    pub fn similarity(&self, other: &Self) -> f32 {
        let d = self.hamming_distance(other).min(5120);
        1.0 - ((d as f32) / 5120.0)
    }

    /// Bipolar cosine similarity $\frac{1}{D}\langle \pm 1_A, \pm 1_B \rangle \in [-1.0, +1.0]$
    /// where `1.0` is identical (`d == 0`), `0.0` is orthogonal (`d == 5120`), and `-1.0` is antipodal (`d == 10240`).
    #[inline]
    pub fn cosine_bipolar(&self, other: &Self) -> f32 {
        let d = self.hamming_distance(other) as f32;
        1.0 - ((2.0 * d) / (HV_BITS as f32))
    }

    /// Extracts an unbiased 64-bit sub-hypervector by sampling bit 0 of the first 64 `u64` words,
    /// enabling $< 2\text{ ns}$ L2-cache pre-screening in `HdcAssociativeIndex`.
    #[inline]
    pub const fn sub_hypervector_64(&self) -> u64 {
        let mut out = 0u64;
        let mut i = 0;
        while i < 64 {
            out |= (self.words[i] & 1) << i;
            i += 1;
        }
        out
    }

    /// Computes a non-linear 128-bit fingerprint over all 160 words for collision-free
    /// deduplication in `HdcBundler` (distinguishing structured vectors such as `H` and `!H`
    /// whose linear XOR parity digests coincide).
    #[inline]
    pub const fn dedup_fingerprint_u128(&self) -> u128 {
        let mut h1 = 0xCBF2_9CE4_8422_2325u64;
        let mut h2 = 0x8422_2325_CBF2_9CE4u64;
        let mut i = 0;
        while i < HV_WORDS {
            let w = self.words[i];
            let lane = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            h1 = (h1 ^ w.wrapping_add(lane))
                .wrapping_mul(0x0000_0100_0000_01B3)
                .rotate_left(29);
            h2 = (h2 ^ w.rotate_left(17).wrapping_sub(lane))
                .wrapping_mul(0xBF58_476D_1CE4_E5B9)
                .rotate_right(23);
            i += 1;
        }
        ((h1 as u128) << 64) | (h2 as u128)
    }

    /// Computes the 64-bit rotational XOR-tree parity fold (`hv:<16-hex>` in Profile A).
    #[inline]
    pub fn hv_digest_u64(&self) -> u64 {
        let mut acc = 0u64;
        for (i, &w) in self.words.iter().enumerate() {
            acc ^= w.rotate_left((i as u32) % 64);
        }
        acc
    }

    /// Formats the 64-bit rotational XOR-tree parity digest as 16 lowercase hexadecimal characters.
    #[inline]
    pub fn encode_hv_hex16(&self) -> String {
        format!("{:016x}", self.hv_digest_u64())
    }

    /// Folds the 10,240-bit hypervector into a compact 32-bit parity sketch for Braille-256 Channel B.
    #[inline]
    pub fn sketch_u32(&self) -> u32 {
        let acc = self.hv_digest_u64();
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

    /// Projects the 10,240-bit hypervector onto a 64-band (`512-bit`, `[u8; 64]`) spectral sketch
    /// via 1-Lipschitz disjoint 20-bit sub-block majority voting (`docs/ahtl/02-braille-256-spectral-bijection.md` §§3–4).
    ///
    /// Guarantees the 1-Lipschitz metric bound $d_{\text{spectral}}(P, Q) \le d_H(A, B)$:
    /// flipping any single bit in `self` can alter at most one 20-bit sub-block majority vote.
    pub fn spectral_bands_64(&self) -> [u8; 64] {
        let mut bands = [0u8; 64];
        for (m, band_out) in bands.iter_mut().enumerate() {
            let band_bit_start = m * 160;
            let mut byte_val = 0u8;
            for j in 0..8 {
                let sub_start = band_bit_start + j * 20;
                let mut ones = 0u32;
                for offset in 0..20 {
                    let bit_idx = sub_start + offset;
                    let word_idx = bit_idx / 64;
                    let bit_in_word = bit_idx % 64;
                    ones += ((self.words[word_idx] >> bit_in_word) & 1) as u32;
                }
                if ones >= 10 {
                    byte_val |= 1u8 << j;
                }
            }
            *band_out = byte_val;
        }
        bands
    }
}

/// Infini-attention Delta-Rule (`arXiv:2404.07143`) majority-vote hypervector bundler.
/// Prevents repetitive boilerplate from saturating associative memory.
#[derive(Clone, Debug)]
pub struct HdcBundler {
    acc: [i16; HV_BITS],
    readout: Hypervector,
    count: usize,
    seen_fingerprints: BTreeSet<u128>,
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
            seen_fingerprints: BTreeSet::new(),
        }
    }

    /// Number of hypervector accumulation updates applied to the bundle.
    #[inline]
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Resets the accumulator, readout, and seen-fingerprint set to the empty zero state.
    pub fn reset(&mut self) {
        self.acc = [0i16; HV_BITS];
        self.readout = Hypervector::zero();
        self.count = 0;
        self.seen_fingerprints.clear();
    }

    /// Unbinds a role hypervector from the current bundle readout (`M \oplus R \approx F`).
    #[inline]
    pub fn unbind_query(&self, role: &Hypervector) -> Hypervector {
        self.readout.unbind(role)
    }

    /// Adds a hypervector using the Infini-attention Delta Rule: vectors already strongly present
    /// in the bundle (`similarity >= 0.85` or matching a previously bundled 128-bit fingerprint)
    /// receive zero delta weight so repetitive boilerplate never saturates the accumulator, while
    /// novel/orthogonal vectors receive full weight.
    pub fn add(&mut self, hv: &Hypervector) {
        let fp = hv.dedup_fingerprint_u128();
        if self.seen_fingerprints.contains(&fp) {
            return;
        }
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
        self.seen_fingerprints.insert(fp);
        self.add_weighted(hv, weight);
    }

    /// Adds a hypervector with an explicit integer weight (`1..=64`), breaking exact `0` ties
    /// symmetrically via coordinate parity `((word_idx ^ bit) & 1) == 0` so the thresholded
    /// readout stays balanced around Hamming weight `5,120`.
    pub fn add_weighted(&mut self, hv: &Hypervector, weight: i16) {
        self.seen_fingerprints.insert(hv.dedup_fingerprint_u128());
        let w = weight.clamp(1, 64);
        for word_idx in 0..HV_WORDS {
            let word = hv.words[word_idx];
            let base = word_idx * 64;
            let mut out_word = 0u64;
            for bit in 0..64 {
                let delta = if ((word >> bit) & 1) != 0 { w } else { -w };
                let val = self.acc[base + bit].saturating_add(delta);
                self.acc[base + bit] = val;
                if val > 0 || (val == 0 && ((word_idx ^ bit) & 1) == 0) {
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
        assert!(a.cosine_bipolar(&b).abs() < 0.05);
        assert!((a.cosine_bipolar(&a) - 1.0).abs() < 1e-6);
        let pop = a.popcount();
        assert!(
            (4900..=5340).contains(&pop),
            "expected balanced Hamming weight near 5120, got {pop}"
        );

        let bound = core::ops::BitXor::bitxor(&a, &b);
        assert_eq!(bound, a ^ b);
        assert_eq!(bound.unbind(&b), a, "XOR binding must be self-inverse");
        let mut c = a;
        c ^= &b;
        c ^= b;
        assert_eq!(c, a);

        // Verify permutation isometry, inverse_permute, framed seeds, role-filler, and sequence encoding
        for shift in [0usize, 1, 7, 63, 64, 159, 160, 321] {
            let pa = a.permute(shift);
            let pb = b.permute(shift);
            assert_eq!(pa.hamming_distance(&pb), a.hamming_distance(&b));
            assert_eq!(pa.inverse_permute(shift), a);
        }

        let f1 = Hypervector::from_seed_framed("a::b", "c");
        let f2 = Hypervector::from_seed_framed("a", "b::c");
        assert_ne!(f1, f2);
        assert!(f1.similarity(&f2) < 0.05);

        let rf = Hypervector::bind_role_filler("fn", "alloc_page");
        let role_fn = Hypervector::from_seed("role", "fn");
        let tok_alloc = Hypervector::from_seed("tok", "alloc_page");
        assert_eq!(rf.unbind(&role_fn), tok_alloc);

        assert_eq!(Hypervector::encode_sequence(&[]), Hypervector::zero());
        let seq_ab = Hypervector::encode_sequence(&[a, b]);
        let seq_ba = Hypervector::encode_sequence(&[b, a]);
        assert_ne!(seq_ab, seq_ba, "sequence encoding must be non-commutative");
        assert!(seq_ab.similarity(&seq_ba) < 0.05);
    }

    #[test]
    fn delta_rule_bundler_prevents_boilerplate_saturation() {
        let mut bundler = HdcBundler::default();
        let boilerplate = Hypervector::from_seed("log", "test_ok_boilerplate");
        let anomaly = Hypervector::from_seed("err", "panic_align_offset_412");

        // 100 identical boilerplate vectors followed by 1 novel anomaly vector via `add`
        for _ in 0..100 {
            bundler.add(&boilerplate);
        }
        bundler.add(&anomaly);
        assert_eq!(bundler.count(), 2);

        let memory = bundler.finalize();
        assert!(
            memory.similarity(&anomaly) > 0.25,
            "anomaly must remain retrievable despite 100x boilerplate repetition"
        );
        assert!(
            memory.similarity(&boilerplate) > 0.25,
            "boilerplate must also remain retrievable in balanced superposition"
        );

        // Also verify reverse order (1 anomaly followed by 100 identical boilerplate vectors)
        bundler.reset();
        assert_eq!(bundler.count(), 0);
        let role_err = Hypervector::from_seed("role", "err");
        let bound_anomaly = role_err.bind(&anomaly);
        bundler.add(&bound_anomaly);
        for _ in 0..100 {
            bundler.add(&boilerplate);
        }
        let unbound = bundler.unbind_query(&role_err);
        assert!(
            unbound.similarity(&anomaly) > 0.25,
            "role-filler unbinding from bundle must recover anomaly after 100x trailing boilerplate"
        );

        // Verify that a vector and its bitwise complement (which share hv_digest_u64) do not collide
        bundler.reset();
        let all_zero = Hypervector::zero();
        let all_ones = Hypervector {
            words: [u64::MAX; HV_WORDS],
        };
        assert_eq!(all_zero.hv_digest_u64(), all_ones.hv_digest_u64());
        assert_ne!(
            all_zero.dedup_fingerprint_u128(),
            all_ones.dedup_fingerprint_u128()
        );
        bundler.add(&all_zero);
        bundler.add(&all_ones);
        assert_eq!(
            bundler.count(),
            2,
            "antipodal complement must not be discarded as a duplicate"
        );
    }

    #[test]
    fn splitmix64_seed_and_1_lipschitz_spectral_bands_64() {
        let a = Hypervector::from_u64_seed(42);
        let b = Hypervector::from_u64_seed(43);
        assert!(a.similarity(&b) < 0.05);
        assert_eq!(a.encode_hv_hex16().len(), 16);

        let bands_a = a.spectral_bands_64();
        // Flip k bits in `a` and verify 1-Lipschitz inequality: d_spectral(P, Q) <= d_H(a, mutated)
        let mut mutated = a;
        for flip_idx in 0..25usize {
            let word = (flip_idx * 7) % HV_WORDS;
            let bit = (flip_idx * 13) % 64;
            mutated.words[word] ^= 1u64 << bit;

            let d_full = a.hamming_distance(&mutated);
            let bands_m = mutated.spectral_bands_64();
            let d_spectral: u32 = bands_a
                .iter()
                .zip(bands_m.iter())
                .map(|(&x, &y)| (x ^ y).count_ones())
                .sum();
            assert!(
                d_spectral <= d_full,
                "1-Lipschitz violation: d_spectral ({d_spectral}) > d_full ({d_full})"
            );
        }
    }
}
