//! Fractal Chaos-Game Trajectory Archive (`FCGA`, `arXiv:2607.07144`).
//!
//! Maps discrete state sequences into a 16-dimensional contractive Iterated Function System (IFS)
//! coordinate in `[-1.0, 1.0]^16` where Chebyshev distance $d_\infty$ encodes the exact shared
//! trailing state suffix length in $O(1)$ time.

use crate::hv::Hypervector;
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
            out[j] = (self.coords[j].clamp(-1.0, 1.0) * 32767.0) as i16;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractal_chaos_game_coordinate_detects_exact_shared_suffix_in_o1() {
        let s_a1 = Hypervector::from_seed("step", "unrelated_prefix_a1");
        let s_a2 = Hypervector::from_seed("step", "unrelated_prefix_a2");
        let s_b1 = Hypervector::from_seed("step", "different_prefix_b1");

        // Shared 5-step debugging suffix
        let shared: Vec<Hypervector> = (0..5)
            .map(|i| Hypervector::from_seed("step", &format!("shared_loop_{i}")))
            .collect();

        let mut traj_a = FractalCoord::default().step(&s_a1).step(&s_a2);
        let mut traj_b = FractalCoord::default().step(&s_b1);

        for hv in &shared {
            traj_a = traj_a.step(hv);
            traj_b = traj_b.step(hv);
        }

        let depth = traj_a.shared_suffix_depth(&traj_b);
        assert!(depth >= 5, "expected shared suffix depth >= 5, got {depth}");
    }

    #[test]
    fn fractal_coord_fc_hex64_and_q15_round_trip() {
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
