//! `icx-hdc` — 10,240-bit Hyperdimensional Computing (`HDC/HRR`), Infini-attention Delta-Rule
//! Bundler, Contractive IFS Fractal Chaos-Game Trajectory Archive (`FCGA`), and Titans
//! Surprise-Gated Momentum Tracker.

pub mod fractal;
pub mod hv;
pub mod surprise;

use crate::fractal::FractalCoord;
use crate::hv::Hypervector;
use crate::surprise::{SurpriseVerdict, TitansSurpriseGate};
use icx_core::ArtifactId;
use serde::{Deserialize, Serialize};

/// Entry stored in the `HdcAssociativeIndex`.
#[derive(Clone, Debug)]
pub struct IndexedEntry {
    pub artifact_id: ArtifactId,
    pub subject: String,
    pub hv: Hypervector,
    pub fractal_coord: FractalCoord,
    pub surprise: f32,
}

/// Ranked associative retrieval hit combining HDC Hamming similarity and Fractal suffix depth.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AssociativeHit {
    pub artifact_id: ArtifactId,
    pub subject: String,
    pub hdc_similarity: f32,
    pub shared_fractal_suffix: u32,
    pub combined_score: f32,
}

/// Receipt returned when an event is observed and indexed.
#[derive(Clone, Copy, Debug)]
pub struct ObservationReceipt {
    pub verdict: SurpriseVerdict,
    pub fractal_coord: FractalCoord,
    pub hologram_bytes: [u8; 8],
}

/// Unified 10,240-Bit HDC + Fractal Chaos-Game + Titans Surprise Associative Index.
#[derive(Clone, Debug, Default)]
pub struct HdcAssociativeIndex {
    entries: Vec<IndexedEntry>,
    current_trajectory: FractalCoord,
    surprise_gate: TitansSurpriseGate,
}

impl HdcAssociativeIndex {
    /// Creates a new empty `HdcAssociativeIndex`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of indexed historical artifacts.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if no artifacts have been indexed.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the current 16-D Fractal Chaos-Game trajectory coordinate.
    pub const fn current_trajectory(&self) -> FractalCoord {
        self.current_trajectory
    }

    /// Encodes and indexes an event, advancing the Fractal trajectory and Titans Surprise Gate.
    pub fn observe_event(
        &mut self,
        artifact_id: ArtifactId,
        subject: &str,
        content: &str,
        force_anomaly: bool,
    ) -> ObservationReceipt {
        let subj_hv = Hypervector::from_seed("subj", &subject.to_ascii_lowercase());
        let text_hv = Hypervector::encode_text(&format!("{subject} {content}"));
        let composite_hv = subj_hv.bind(&text_hv).permute(1);

        let verdict = self
            .surprise_gate
            .observe(&composite_hv, content, force_anomaly);
        self.current_trajectory = self.current_trajectory.step(&composite_hv);

        let sketch = composite_hv.sketch_u32().to_be_bytes();
        let quad = self
            .current_trajectory
            .quadrant_signature_u16()
            .to_be_bytes();
        let surp_byte = (verdict.momentum_surprise.clamp(0.0, 1.0) * 255.0) as u8;
        let tier_byte = verdict.tier as u8;
        let hologram_bytes = [
            sketch[0], sketch[1], sketch[2], sketch[3], quad[0], quad[1], surp_byte, tier_byte,
        ];

        self.entries.push(IndexedEntry {
            artifact_id,
            subject: subject.to_owned(),
            hv: composite_hv,
            fractal_coord: self.current_trajectory,
            surprise: verdict.momentum_surprise,
        });

        ObservationReceipt {
            verdict,
            fractal_coord: self.current_trajectory,
            hologram_bytes,
        }
    }

    /// Queries the associative index using 10,240-bit HDC Hamming similarity, Fractal trajectory
    /// suffix matching, and Titans Surprise weighting.
    pub fn query_associative(&self, query_text: &str, top_k: usize) -> Vec<AssociativeHit> {
        let q_hv = Hypervector::encode_text(query_text);
        let q_tokens: Vec<String> = query_text
            .split(|c: char| !c.is_alphanumeric() && c != '_' && c != ':' && c != '/')
            .filter(|s| s.len() >= 2)
            .map(str::to_ascii_lowercase)
            .collect();

        let mut hits: Vec<AssociativeHit> = self
            .entries
            .iter()
            .map(|e| {
                let text_sim = q_hv.similarity(&e.hv);
                let subj_lower = e.subject.to_ascii_lowercase();
                let lexical_overlap = q_tokens
                    .iter()
                    .filter(|t| subj_lower.contains(t.as_str()))
                    .count() as f32;
                let hdc_sim = (text_sim + 0.25 * lexical_overlap).clamp(0.0, 1.0);
                let suffix = self
                    .current_trajectory
                    .shared_suffix_depth(&e.fractal_coord);
                let combined = 0.65 * hdc_sim
                    + 0.20 * e.surprise
                    + 0.15 * ((suffix as f32) / 8.0).clamp(0.0, 1.0);

                AssociativeHit {
                    artifact_id: e.artifact_id,
                    subject: e.subject.clone(),
                    hdc_similarity: hdc_sim,
                    shared_fractal_suffix: suffix,
                    combined_score: combined,
                }
            })
            .collect();

        hits.sort_by(|a, b| {
            b.combined_score
                .partial_cmp(&a.combined_score)
                .unwrap_or(core::cmp::Ordering::Equal)
        });
        hits.truncate(top_k);
        hits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hdc_associative_index_tracks_empty_state_and_trajectory() {
        let mut idx = HdcAssociativeIndex::new();
        assert!(idx.is_empty());
        assert_eq!(idx.len(), 0);
        let init_traj = idx.current_trajectory();
        let receipt = idx.observe_event(
            ArtifactId(1),
            "src/mmu.rs",
            "pub fn allocate_page() {}",
            true,
        );
        assert!(!idx.is_empty());
        assert_eq!(idx.len(), 1);
        assert_ne!(idx.current_trajectory(), init_traj);
        assert_eq!(receipt.fractal_coord, idx.current_trajectory());
        let hits = idx.query_associative("allocate_page mmu", 3);
        assert_eq!(hits.len(), 1);
    }
}
