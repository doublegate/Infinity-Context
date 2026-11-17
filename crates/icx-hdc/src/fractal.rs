//! Fractal Chaos-Game Trajectory Archive (`FCGA`, `arXiv:2607.07144`).
//!
//! Maps discrete state sequences into a 16-dimensional contractive Iterated Function System (IFS)
//! coordinate in `[-1.0, 1.0]^16` where Chebyshev distance $d_\infty$ encodes the exact shared
//! trailing state suffix length in $O(1)$ time.

use crate::hv::Hypervector;
use icx_core::ArtifactId;
use serde::{Deserialize, Serialize};

/// 16-dimensional Contractive Iterated Function System (IFS) trajectory coordinate.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FractalCoord {
    pub coords: [f64; 16],
    pub depth: u32,
}

impl Default for FractalCoord {
    fn default() -> Self {
        Self {
            coords: [0.0; 16],
            depth: 0,
        }
    }
}

impl FractalCoord {
    /// Extracts the 16-dimensional bipolar corner vertex `v(H) in {-1.0, +1.0}^16` from a `Hypervector`.
    #[inline]
    pub fn corner_vertex(hv: &Hypervector) -> [f64; 16] {
        let mut v = [1.0f64; 16];
        for dim in 0..16 {
            let start = dim * 10;
            let mut ones = 0u32;
            for w in &hv.words[start..start + 10] {
                ones += w.count_ones();
            }
            // 10 words = 640 bits; threshold at 320
            v[dim] = if ones >= 320 { 1.0 } else { -1.0 };
        }
        v
    }

    /// Advances the trajectory coordinate by one contractive IFS transition ($\alpha = 0.5$):
    /// $$z_t = \frac{1}{2} z_{t-1} + \frac{1}{2} v(s_t)$$
    #[inline]
    pub fn step(&self, state_hv: &Hypervector) -> Self {
        let v = Self::corner_vertex(state_hv);
        let mut next = [0.0f64; 16];
        for j in 0..16 {
            next[j] = 0.5 * self.coords[j] + 0.5 * v[j];
        }
        Self {
            coords: next,
            depth: self.depth.saturating_add(1),
        }
    }

    /// Chebyshev ($L_\infty$) distance between two trajectory coordinates.
    #[inline]
    pub fn chebyshev_distance(&self, other: &Self) -> f64 {
        let mut max_diff = 0.0f64;
        for j in 0..16 {
            let d = (self.coords[j] - other.coords[j]).abs();
            if d > max_diff {
                max_diff = d;
            }
        }
        max_diff
    }

    /// Euclidean ($L_2$) distance between two trajectory coordinates (`docs/ahtl/03-fractal-ifs-chaos-game-coordinates.md` §3.2):
    /// satisfies $d_2(\vec{z}^{(A)}, \vec{z}^{(B)}) \le 4 \cdot d_\infty(\vec{z}^{(A)}, \vec{z}^{(B)}) \le 2^{3 - K}$ for a shared $K$-step suffix.
    #[inline]
    pub fn euclidean_distance(&self, other: &Self) -> f64 {
        let mut sum_sq = 0.0f64;
        for j in 0..16 {
            let d = self.coords[j] - other.coords[j];
            sum_sq += d * d;
        }
        sum_sq.sqrt()
    }

    /// Computes the number of shared trailing state transitions (`0..=32`) between two trajectories
    /// in $O(1)$ time via the contractive IFS suffix theorem on `[-1.0, 1.0]^16` (diameter 2.0):
    /// $$L_{\text{shared}} = \lfloor -\log_2(\max(d_\infty(z_a, z_b) / 2, 2^{-32})) \rfloor$$
    #[inline]
    pub fn shared_suffix_depth(&self, other: &Self) -> u32 {
        let d_norm = self.chebyshev_distance(other) * 0.5;
        if d_norm <= 0.0 {
            return self.depth.min(other.depth).min(32);
        }
        if d_norm >= 1.0 {
            return 0;
        }
        let k = (-d_norm.log2()).floor() as u32;
        k.min(self.depth).min(other.depth).min(32)
    }

    /// Quantizes the Fractal coordinate into a 16-bit quadrant signature for Braille-256 Channel B.
    #[inline]
    pub fn quadrant_signature_u16(&self) -> u16 {
        let mut mask = 0u16;
        for j in 0..16 {
            if self.coords[j] >= 0.0 {
                mask |= 1u16 << j;
            }
        }
        mask
    }

    /// Quantizes the 16 coordinates into signed 16-bit fixed-point (`Q15`) for `/dev/shm` slots.
    #[inline]
    pub fn to_q15(&self) -> [i16; 16] {
        let mut out = [0i16; 16];
        for j in 0..16 {
            out[j] = (self.coords[j].clamp(-1.0, 1.0) * 32767.0).round() as i16;
        }
        out
    }

    /// Reconstructs a `FractalCoord` from signed 16-bit fixed-point (`Q15`) coordinates and `depth`.
    #[inline]
    pub fn from_q15(q15: [i16; 16], depth: u32) -> Self {
        let mut coords = [0.0f64; 16];
        for j in 0..16 {
            coords[j] = (q15[j] as f64 / 32767.0).clamp(-1.0, 1.0);
        }
        Self { coords, depth }
    }

    /// Quantizes the 16 coordinates into signed `Q1.14` fixed-point (`scale = 16,384 = 2^{14}`,
    /// `docs/ahtl/03-fractal-ifs-chaos-game-coordinates.md` §4) supporting bit-exact shift arithmetic.
    #[inline]
    pub fn to_q1_14(&self) -> [i16; 16] {
        let mut out = [0i16; 16];
        for j in 0..16 {
            out[j] = (self.coords[j].clamp(-1.0, 1.0) * 16384.0).round() as i16;
        }
        out
    }

    /// Reconstructs a `FractalCoord` from signed `Q1.14` fixed-point (`scale = 16,384`) coordinates and `depth`.
    #[inline]
    pub fn from_q1_14(q14: [i16; 16], depth: u32) -> Self {
        let mut coords = [0.0f64; 16];
        for j in 0..16 {
            coords[j] = (q14[j] as f64 / 16384.0).clamp(-1.0, 1.0);
        }
        Self { coords, depth }
    }

    /// Computes a 64-bit Morton Z-order prefix code interleaving the top 4 dyadic bit planes
    /// across all 16 coordinates (`bits 48..=63` = `quadrant_signature_u16()`, `bits 32..=47` = 2nd
    /// dyadic plane, `bits 16..=31` = 3rd dyadic plane, `bits 0..=15` = 4th dyadic plane).
    #[inline]
    pub fn morton_z64(&self) -> u64 {
        let mut z = 0u64;
        for j in 0..16 {
            let c = self.coords[j].clamp(-1.0, 1.0);
            let mag3 = ((c.abs() * 8.0).floor() as u64).min(7);
            let q4 = if c >= 0.0 { 8 + mag3 } else { 7 - mag3 };
            for plane in 0..4u64 {
                let bit = (q4 >> plane) & 1;
                z |= bit << (plane * 16 + (j as u64));
            }
        }
        z
    }

    /// Encodes the 16-bit orthant signature into the compact octal quadrant address `@o1:o2`
    /// (`docs/ahtl/03-fractal-ifs-chaos-game-coordinates.md`, `docs/alien-holographic-codec.md`).
    #[inline]
    pub fn encode_octal_addr(&self) -> String {
        let q = self.quadrant_signature_u16();
        let hi = (q >> 8) & 0xFF;
        let lo = q & 0xFF;
        format!("@{hi:04o}:{lo:04o}")
    }

    /// Decodes a compact octal quadrant address `@o1:o2` back into the 16-bit orthant signature.
    pub fn decode_octal_addr(s: &str) -> Option<u16> {
        let inner = s.trim().strip_prefix('@')?;
        let (hi_str, lo_str) = inner.split_once(':')?;
        if hi_str.len() != 4 || lo_str.len() != 4 {
            return None;
        }
        let hi = u16::from_str_radix(hi_str, 8).ok()?;
        let lo = u16::from_str_radix(lo_str, 8).ok()?;
        if hi > 0xFF || lo > 0xFF {
            return None;
        }
        Some((hi << 8) | lo)
    }

    /// Folds the quantized `Q15` trajectory coordinates into a 32-bit digest for Channel B `B[0..3]`
    /// or trajectory fingerprinting.
    #[inline]
    pub fn digest_u32(&self) -> u32 {
        let q15 = self.to_q15();
        let mut acc = 0x811C_9DC5u32;
        for &val in &q15 {
            let bytes = val.cast_unsigned().to_be_bytes();
            acc = (acc ^ u32::from(bytes[0])).wrapping_mul(0x0100_0193);
            acc = (acc ^ u32::from(bytes[1])).wrapping_mul(0x0100_0193);
        }
        acc ^ self.depth
    }

    /// Encodes the 16-D `Q15` coordinate into a 64-character lowercase hexadecimal string
    /// (for the `fc:<64-hex>` wire field in `AHTL v1` Profile A).
    pub fn encode_fc_hex64(&self) -> String {
        let q15 = self.to_q15();
        let mut out = String::with_capacity(64);
        for &val in &q15 {
            use core::fmt::Write;
            let _ = write!(out, "{:04x}", val.cast_unsigned());
        }
        out
    }

    /// Decodes a 64-character hexadecimal string (or `"fc:<64-hex>"`) back into a `FractalCoord`.
    pub fn decode_fc_hex64(s: &str, depth: u32) -> Option<Self> {
        let trimmed = s.trim();
        let hex = trimmed.strip_prefix("fc:").unwrap_or(trimmed);
        if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let mut q15 = [0i16; 16];
        for (j, slot) in q15.iter_mut().enumerate() {
            let slice = &hex[j * 4..(j + 1) * 4];
            let u = u16::from_str_radix(slice, 16).ok()?;
            *slot = u.cast_signed();
        }
        Some(Self::from_q15(q15, depth))
    }
}

/// Contractive IFS Trajectory Archive (`arXiv:2607.07144`) indexing historical `(ArtifactId, FractalCoord)`
/// states by their 64-bit Morton Z-order prefix and $O(1)$ Chebyshev suffix distance.
#[derive(Clone, Debug, Default)]
pub struct FractalTrajectoryArchive {
    current: FractalCoord,
    entries: Vec<(u64, ArtifactId, FractalCoord)>,
}

impl FractalTrajectoryArchive {
    /// Creates a new empty `FractalTrajectoryArchive`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of archived trajectory states.
    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if no trajectory states have been archived.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the current active trajectory coordinate.
    #[inline]
    pub const fn current(&self) -> FractalCoord {
        self.current
    }

    /// Advances the active trajectory by `state_hv`, archives `(artifact_id, current)`, and returns the new coordinate.
    pub fn record_step(&mut self, artifact_id: ArtifactId, state_hv: &Hypervector) -> FractalCoord {
        self.current = self.current.step(state_hv);
        self.insert_coord(artifact_id, self.current);
        self.current
    }

    /// Inserts an explicit `(artifact_id, coord)` pair into the archive.
    pub fn insert_coord(&mut self, artifact_id: ArtifactId, coord: FractalCoord) {
        self.entries.push((coord.morton_z64(), artifact_id, coord));
    }

    /// Queries the archive for historical trajectory states sharing at least `min_suffix_depth`
    /// trailing transitions with `target`, returning `(ArtifactId, shared_suffix_depth, chebyshev_distance)`
    /// sorted by `shared_suffix_depth` descending, `chebyshev_distance` ascending, and 64-bit Morton
    /// XOR proximity ascending.
    pub fn query_shared_suffix(
        &self,
        target: &FractalCoord,
        min_suffix_depth: u32,
        top_k: usize,
    ) -> Vec<(ArtifactId, u32, f64)> {
        if top_k == 0 || self.entries.is_empty() {
            return Vec::new();
        }
        let target_morton = target.morton_z64();
        let mut matches: Vec<(ArtifactId, u32, f64, u64)> = self
            .entries
            .iter()
            .filter_map(|&(morton, id, ref coord)| {
                let depth = target.shared_suffix_depth(coord);
                if depth >= min_suffix_depth {
                    Some((
                        id,
                        depth,
                        target.chebyshev_distance(coord),
                        (morton ^ target_morton).count_ones() as u64,
                    ))
                } else {
                    None
                }
            })
            .collect();

        matches.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| a.2.partial_cmp(&b.2).unwrap_or(core::cmp::Ordering::Equal))
                .then_with(|| a.3.cmp(&b.3))
        });
        matches.truncate(top_k);
        matches
            .into_iter()
            .map(|(id, d, c, _)| (id, d, c))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractal_chaos_game_coordinate_detects_exact_shared_suffix_in_o1() {
        let s_a1 = Hypervector::from_seed("step", "unrelated_prefix_a1");
        let s_a2 = Hypervector::from_seed("step", "unrelated_prefix_a2");
        let s_b1 = Hypervector::from_seed("step", "different_prefix_b1");

        // Verify shared suffix depth for K in 1..=32 (Theorem 3.2: d_inf <= 2^{1-K} and d_2 <= 2^{3-K})
        let mut traj_a = FractalCoord::default().step(&s_a1).step(&s_a2);
        let mut traj_b = FractalCoord::default().step(&s_b1);

        for k in 1..=32u32 {
            let hv = Hypervector::from_seed("step", &format!("shared_loop_{k}"));
            traj_a = traj_a.step(&hv);
            traj_b = traj_b.step(&hv);

            let depth = traj_a.shared_suffix_depth(&traj_b);
            assert!(
                depth >= k,
                "for K={k}, expected shared suffix depth >= {k}, got {depth}"
            );
            let d_inf = traj_a.chebyshev_distance(&traj_b);
            let d_l2 = traj_a.euclidean_distance(&traj_b);
            let bound_inf = 2.0f64.powi(1 - k.cast_signed()) + 1e-12;
            let bound_l2 = 4.0 * bound_inf;
            assert!(d_inf <= bound_inf);
            assert!(d_l2 <= bound_l2);
        }
    }

    #[test]
    fn fractal_coord_fc_hex64_q15_q14_morton_octal_and_archive_round_trip() {
        let s1 = Hypervector::from_seed("step", "state_alpha");
        let s2 = Hypervector::from_seed("step", "state_beta");
        let s3 = Hypervector::from_seed("step", "state_gamma");
        let coord = FractalCoord::default().step(&s1).step(&s2).step(&s3);

        let hex64 = coord.encode_fc_hex64();
        assert_eq!(hex64.len(), 64);
        assert!(hex64.bytes().all(|b| b.is_ascii_hexdigit()));

        let decoded = FractalCoord::decode_fc_hex64(&hex64, coord.depth).expect("valid hex64");
        let decoded_prefixed =
            FractalCoord::decode_fc_hex64(&format!("fc:{hex64}"), coord.depth).expect("prefixed");
        assert_eq!(decoded.to_q15(), coord.to_q15());
        assert_eq!(decoded_prefixed.to_q15(), coord.to_q15());
        assert_eq!(
            decoded.quadrant_signature_u16(),
            coord.quadrant_signature_u16()
        );
        assert_eq!(decoded.digest_u32(), coord.digest_u32());

        // Verify Q1.14 fixed-point conversion (exact for dyadic depth <= 14)
        let q14 = coord.to_q1_14();
        let from_q14 = FractalCoord::from_q1_14(q14, coord.depth);
        assert_eq!(from_q14, coord);

        // Verify Morton Z-order top 16 bits equal quadrant_signature_u16() including tiny negatives
        assert_eq!(
            (coord.morton_z64() >> 48) as u16,
            coord.quadrant_signature_u16()
        );
        let sub_neg = FractalCoord {
            coords: [-1e-300; 16],
            depth: 1,
        };
        assert_eq!(
            (sub_neg.morton_z64() >> 48) as u16,
            sub_neg.quadrant_signature_u16()
        );

        // Verify octal quadrant address @o1:o2 round-trip and error cases
        let octal = coord.encode_octal_addr();
        assert_eq!(octal.len(), 10);
        assert_eq!(
            FractalCoord::decode_octal_addr(&octal),
            Some(coord.quadrant_signature_u16())
        );
        assert_eq!(FractalCoord::decode_octal_addr("invalid"), None);
        assert_eq!(FractalCoord::decode_octal_addr("@123:456"), None);
        assert_eq!(FractalCoord::decode_octal_addr("@0777:0000"), None);
        assert_eq!(FractalCoord::decode_octal_addr("@0000:0777"), None);

        // Verify FractalTrajectoryArchive suffix search and zero-boundary coordinates
        let mut archive = FractalTrajectoryArchive::new();
        assert!(archive.is_empty());
        assert_eq!(archive.len(), 0);
        let _ = archive.record_step(ArtifactId(1), &s1);
        let _ = archive.record_step(ArtifactId(2), &s2);
        let c3 = archive.record_step(ArtifactId(3), &s3);
        assert_eq!(archive.len(), 3);
        assert!(!archive.is_empty());
        assert_eq!(archive.current(), c3);

        let other_prefix = Hypervector::from_seed("step", "other_prefix");
        let query_coord = FractalCoord::default()
            .step(&other_prefix)
            .step(&s2)
            .step(&s3);
        let hits = archive.query_shared_suffix(&query_coord, 2, 5);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, ArtifactId(3));
        assert!(hits[0].1 >= 2);
        assert_eq!(archive.query_shared_suffix(&query_coord, 2, 0).len(), 0);

        // Verify zero-boundary coordinate `[0.0; 16]` vs `[-0.75; 16]` with depth >= 1
        let mut boundary_archive = FractalTrajectoryArchive::new();
        let zero_boundary = FractalCoord {
            coords: [0.0; 16],
            depth: 55,
        };
        let neg_two_steps = FractalCoord {
            coords: [-0.75; 16],
            depth: 2,
        };
        boundary_archive.insert_coord(ArtifactId(77), zero_boundary);
        let boundary_hits = boundary_archive.query_shared_suffix(&neg_two_steps, 1, 5);
        assert_eq!(boundary_hits.len(), 1);
        assert_eq!(boundary_hits[0].0, ArtifactId(77));
        assert_eq!(boundary_hits[0].1, 1);

        // Edge cases for shared_suffix_depth and invalid hex
        assert_eq!(coord.shared_suffix_depth(&coord), 3);
        let opp = FractalCoord {
            coords: [-1.0; 16],
            depth: 4,
        };
        let pos = FractalCoord {
            coords: [1.0; 16],
            depth: 4,
        };
        assert_eq!(pos.shared_suffix_depth(&opp), 0);
        assert!(FractalCoord::decode_fc_hex64("fc:1234", 1).is_none());
        assert!(
            FractalCoord::decode_fc_hex64(&"z".repeat(64), 1).is_none(),
            "non-hex must return None"
        );
    }
}
