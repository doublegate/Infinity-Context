//! `icx-hdc` — 10,240-bit Hyperdimensional Computing (`HDC/HRR`), Infini-attention Delta-Rule
//! Bundler, Contractive IFS Fractal Chaos-Game Trajectory Archive (`FCGA`), and Titans
//! Surprise-Gated Momentum Tracker.

pub mod fractal;
pub mod hv;
pub mod snapshot;
pub mod surprise;

pub use crate::fractal::{FractalCoord, FractalTrajectoryArchive};
pub use crate::hv::{HdcBundler, Hypervector};
pub use crate::snapshot::SnapshotError;
pub use crate::surprise::{RetentionTier, SurpriseVerdict, TitansSurpriseGate};
use icx_core::ArtifactId;
use serde::{Deserialize, Serialize};

/// Normative trait contract for the Hyperdimensional & Fractal Associative Memory Index (`docs/interfaces.md` §3).
pub trait AssociativeMemoryIndex: Send + Sync {
    /// Encodes text and metadata into a 10,240-bit Hypervector, updates the Fractal IFS trajectory,
    /// computes the Titans Surprise score, and indexes the artifact.
    fn observe_event(
        &mut self,
        artifact_id: ArtifactId,
        subject: &str,
        content: &str,
        is_anomaly: bool,
    ) -> ObservationReceipt;

    /// Queries the associative index using combined HDC Hamming similarity and Fractal suffix matching.
    fn query_associative(&self, query_text: &str, top_k: usize) -> Vec<AssociativeHit>;
}

/// Compact 16-byte L2-cache-friendly summary of an `IndexedEntry` for Stage-1 spectral pre-screening.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct EntrySummary {
    hv_sub64: u64,
    subj_bloom32: u32,
    quadrant_u16: u16,
    surprise_q8: u8,
    flags: u8,
}

/// Entry stored in the `HdcAssociativeIndex`.
#[derive(Clone, Debug)]
pub struct IndexedEntry {
    pub artifact_id: ArtifactId,
    subject: String,
    subject_lower: String,
    pub hv: Hypervector,
    pub fractal_coord: FractalCoord,
    pub surprise: f32,
}

impl IndexedEntry {
    /// Creates a new `IndexedEntry` with precomputed lowercase subject invariant.
    pub fn new(
        artifact_id: ArtifactId,
        subject: impl Into<String>,
        hv: Hypervector,
        fractal_coord: FractalCoord,
        surprise: f32,
    ) -> Self {
        let subject = subject.into();
        let subject_lower = subject.to_ascii_lowercase();
        Self {
            artifact_id,
            subject,
            subject_lower,
            hv,
            fractal_coord,
            surprise,
        }
    }

    /// Returns the original subject string.
    #[inline]
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// Returns the cached lowercase subject string.
    #[inline]
    pub fn subject_lower(&self) -> &str {
        &self.subject_lower
    }
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
    summaries: Vec<EntrySummary>,
    trajectory_archive: FractalTrajectoryArchive,
    surprise_gate: TitansSurpriseGate,
}

impl HdcAssociativeIndex {
    /// Creates a new empty `HdcAssociativeIndex`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of indexed historical artifacts.
    /// Serializes the complete index state (entries, pre-screen summaries, fractal archive and
    /// surprise gate) into a versioned binary snapshot, so a workspace can be restored without
    /// re-encoding every stored event (ICX-AUD-001).
    pub fn encode_snapshot(&self) -> Vec<u8> {
        let mut w = snapshot::Writer::default();
        w.buf.extend_from_slice(snapshot::SNAPSHOT_MAGIC);
        w.len(self.entries.len());
        for (e, s) in self.entries.iter().zip(&self.summaries) {
            w.u64(e.artifact_id.0);
            w.str(&e.subject);
            e.hv.write_snapshot(&mut w);
            e.fractal_coord.write_snapshot(&mut w);
            w.f32(e.surprise);
            w.u64(s.hv_sub64);
            w.u32(s.subj_bloom32);
            w.u16(s.quadrant_u16);
            w.u8(s.surprise_q8);
            w.u8(s.flags);
        }
        self.trajectory_archive.write_snapshot(&mut w);
        self.surprise_gate.write_snapshot(&mut w);
        w.buf
    }

    /// Restores an index from [`Self::encode_snapshot`] output. Fails closed on any truncation,
    /// bound violation, bad magic, or trailing data.
    pub fn decode_snapshot(bytes: &[u8]) -> Result<Self, SnapshotError> {
        let mut r = snapshot::Reader::new(bytes);
        r.expect_magic()?;
        let n = r.len("index.entries", snapshot::MAX_SNAPSHOT_ITEMS)?;
        let mut entries = Vec::with_capacity(n.min(4096));
        let mut summaries = Vec::with_capacity(n.min(4096));
        for _ in 0..n {
            let artifact_id = ArtifactId(r.u64()?);
            let subject = r.str("entry.subject")?;
            let hv = Hypervector::read_snapshot(&mut r)?;
            let fractal_coord = FractalCoord::read_snapshot(&mut r)?;
            let surprise = r.f32()?;
            entries.push(IndexedEntry::new(
                artifact_id,
                subject,
                hv,
                fractal_coord,
                surprise,
            ));
            summaries.push(EntrySummary {
                hv_sub64: r.u64()?,
                subj_bloom32: r.u32()?,
                quadrant_u16: r.u16()?,
                surprise_q8: r.u8()?,
                flags: r.u8()?,
            });
        }
        let trajectory_archive = FractalTrajectoryArchive::read_snapshot(&mut r)?;
        let surprise_gate = TitansSurpriseGate::read_snapshot(&mut r)?;
        r.finish()?;
        Ok(Self {
            entries,
            summaries,
            trajectory_archive,
            surprise_gate,
        })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if no artifacts have been indexed.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the current 16-D Fractal Chaos-Game trajectory coordinate.
    pub const fn current_trajectory(&self) -> FractalCoord {
        self.trajectory_archive.current()
    }

    /// Returns a reference to the underlying `FractalTrajectoryArchive`.
    pub const fn trajectory_archive(&self) -> &FractalTrajectoryArchive {
        &self.trajectory_archive
    }

    /// Returns a reference to the underlying `TitansSurpriseGate`.
    pub const fn surprise_gate(&self) -> &TitansSurpriseGate {
        &self.surprise_gate
    }

    /// Applies `steps` intervals of idle exponential decay to the underlying `TitansSurpriseGate`.
    pub fn apply_idle_decay(&mut self, steps: u32) {
        self.surprise_gate.apply_idle_decay(steps);
    }

    /// Returns the most recently indexed entry, if any.
    pub fn latest_entry(&self) -> Option<&IndexedEntry> {
        self.entries.last()
    }

    #[inline]
    fn token_bloom_bits32(tok: &str) -> u32 {
        let mut h = 0x811C_9DC5u32;
        for &b in tok.as_bytes() {
            h ^= u32::from(b.to_ascii_lowercase());
            h = h.wrapping_mul(0x0100_0193);
        }
        (1u32 << (h & 31)) | (1u32 << ((h >> 5) & 31))
    }

    #[inline]
    fn composite_token_bloom32(comp: &str) -> u32 {
        let mut mask = 0u32;
        if comp.len() >= 2 {
            mask |= Self::token_bloom_bits32(comp);
        }
        let mut seg_starts = [0usize; 8];
        let mut seg_count = 0usize;
        let mut byte_idx = 0usize;
        for seg in comp.split(['/', ':']) {
            let start = byte_idx;
            let end = start + seg.len();
            byte_idx = end + 1;
            if seg.len() >= 2 {
                mask |= Self::token_bloom_bits32(seg);
            }
            if !seg.is_empty() {
                for &prev_start in &seg_starts[..seg_count] {
                    mask |= Self::token_bloom_bits32(&comp[prev_start..end]);
                }
                if seg_count == seg_starts.len() {
                    seg_starts.copy_within(1.., 0);
                    seg_count -= 1;
                }
                seg_starts[seg_count] = start;
                seg_count += 1;
            }
        }
        mask
    }

    #[inline]
    fn compute_subject_bloom32(subject_lower: &str) -> u32 {
        let mut mask = 0u32;
        for comp in
            subject_lower.split(|c: char| !c.is_alphanumeric() && c != '_' && c != ':' && c != '/')
        {
            mask |= Self::composite_token_bloom32(comp);
        }
        if !subject_lower.is_empty() {
            mask |= Self::token_bloom_bits32(subject_lower);
        }
        mask
    }

    /// Encodes and indexes an event, advancing the Fractal trajectory and Titans Surprise Gate.
    pub fn observe_event(
        &mut self,
        artifact_id: ArtifactId,
        subject: &str,
        content: &str,
        force_anomaly: bool,
    ) -> ObservationReceipt {
        self.observe_event_with_exit_code(artifact_id, subject, content, None, force_anomaly)
    }

    /// Encodes and indexes an event with an optional process `exit_code`, advancing the Fractal
    /// trajectory and Titans Surprise Gate.
    pub fn observe_event_with_exit_code(
        &mut self,
        artifact_id: ArtifactId,
        subject: &str,
        content: &str,
        exit_code: Option<i32>,
        force_anomaly: bool,
    ) -> ObservationReceipt {
        let composite_hv = Hypervector::encode_text(&format!("{subject} {content}"));

        let verdict = self.surprise_gate.observe_with_exit_code(
            &composite_hv,
            content,
            exit_code,
            force_anomaly,
        );
        let current_trajectory = self
            .trajectory_archive
            .record_step(artifact_id, &composite_hv);

        let sketch = composite_hv.sketch_u32().to_be_bytes();
        let quad_u16 = current_trajectory.quadrant_signature_u16();
        let quad = quad_u16.to_be_bytes();
        let surp_byte = (verdict.momentum_surprise.clamp(0.0, 1.0) * 255.0) as u8;
        let tier_bits = verdict.tier as u8; // Bits 0..=1: 0 = PinHighResolution, 1 = FoldAhtlStub, 2 = ColdBeacon
        let err_flag = if force_anomaly
            || exit_code.is_some_and(|c| c != 0)
            || TitansSurpriseGate::has_diagnostic_anomaly(content)
        {
            1u8 << 2 // FLAG_HAS_ERROR
        } else {
            0u8
        };
        let flags_byte = tier_bits | err_flag;
        let hologram_bytes = [
            sketch[0], sketch[1], sketch[2], sketch[3], quad[0], quad[1], surp_byte, flags_byte,
        ];

        let entry = IndexedEntry::new(
            artifact_id,
            subject,
            composite_hv,
            current_trajectory,
            verdict.momentum_surprise,
        );
        self.summaries.push(EntrySummary {
            hv_sub64: composite_hv.sub_hypervector_64(),
            subj_bloom32: Self::compute_subject_bloom32(entry.subject_lower()),
            quadrant_u16: quad_u16,
            surprise_q8: surp_byte,
            flags: flags_byte,
        });
        self.entries.push(entry);

        ObservationReceipt {
            verdict,
            fractal_coord: current_trajectory,
            hologram_bytes,
        }
    }

    #[inline]
    fn subject_matches_token(subject_lower: &str, tok: &str) -> bool {
        if !tok.contains('/') && !tok.contains(':') && tok.chars().count() >= 4 {
            return subject_lower.contains(tok);
        }
        if tok.is_empty() {
            return false;
        }
        let is_word_char = |c: char| c.is_alphanumeric() || c == '_';
        let left_seg_long = tok
            .split(['/', ':'])
            .next()
            .is_some_and(|s| s.chars().count() >= 4);
        let right_seg_long = tok
            .rsplit(['/', ':'])
            .next()
            .is_some_and(|s| s.chars().count() >= 4);
        let tok_left_sep = left_seg_long || tok.chars().next().is_some_and(|c| !is_word_char(c));
        let tok_right_sep =
            right_seg_long || tok.chars().next_back().is_some_and(|c| !is_word_char(c));
        let mut start = 0usize;
        while let Some(rel) = subject_lower[start..].find(tok) {
            let pos = start + rel;
            let end = pos + tok.len();
            let left_ok = tok_left_sep
                || subject_lower[..pos]
                    .chars()
                    .next_back()
                    .is_none_or(|c| !is_word_char(c));
            let right_ok = tok_right_sep
                || subject_lower[end..]
                    .chars()
                    .next()
                    .is_none_or(|c| !is_word_char(c));
            if left_ok && right_ok {
                return true;
            }
            start = pos
                + subject_lower[pos..]
                    .chars()
                    .next()
                    .map_or(1, char::len_utf8);
        }
        false
    }

    #[inline]
    fn score_entry(
        &self,
        e: &IndexedEntry,
        q_hv: &Hypervector,
        q_tokens: &[String],
    ) -> AssociativeHit {
        let text_sim = q_hv.similarity(&e.hv);
        let lexical_overlap = q_tokens
            .iter()
            .filter(|t| Self::subject_matches_token(&e.subject_lower, t.as_str()))
            .count() as f32;
        let hdc_sim = (text_sim + 0.25 * lexical_overlap).clamp(0.0, 1.0);
        let suffix = self
            .current_trajectory()
            .verified_suffix_depth(&e.fractal_coord);
        let combined =
            0.65 * hdc_sim + 0.20 * e.surprise + 0.15 * ((suffix as f32) / 8.0).clamp(0.0, 1.0);

        AssociativeHit {
            artifact_id: e.artifact_id,
            subject: e.subject.clone(),
            hdc_similarity: hdc_sim,
            shared_fractal_suffix: suffix,
            combined_score: combined,
        }
    }

    /// Queries the associative index using a two-stage pipeline:
    /// 1. Stage 1 L2-cache spectral/Bloom pre-screening over 16-byte `EntrySummary` records.
    /// 2. Stage 2 exact 10,240-bit `hamming_distance_simd` and boundary-aware inverse-IFS `verified_suffix_depth` refinement.
    pub fn query_associative(&self, query_text: &str, top_k: usize) -> Vec<AssociativeHit> {
        if top_k == 0 || self.entries.is_empty() {
            return Vec::new();
        }
        let q_hv = Hypervector::encode_text(query_text);
        let q_tokens: Vec<String> = query_text
            .split(|c: char| !c.is_alphanumeric() && c != '_' && c != ':' && c != '/')
            .filter(|s| s.len() >= 2)
            .map(str::to_ascii_lowercase)
            .collect();

        let adaptive_floor = (self.entries.len() / 64).clamp(16, 512);
        let candidate_cap = top_k
            .saturating_mul(4)
            .clamp(16, 64)
            .max(adaptive_floor)
            .max(top_k)
            .min(self.entries.len());

        let mut hits: Vec<AssociativeHit> =
            if self.entries.len() <= 64 || candidate_cap == self.entries.len() {
                self.entries
                    .iter()
                    .map(|e| self.score_entry(e, &q_hv, &q_tokens))
                    .collect()
            } else {
                let q_sub64 = q_hv.sub_hypervector_64();
                let mut q_bloom32 = 0u32;
                for tok in &q_tokens {
                    q_bloom32 |= Self::composite_token_bloom32(tok);
                }
                let q_bloom_bits = q_bloom32.count_ones().max(1);
                let cur_quad = self.current_trajectory().quadrant_signature_u16();
                let mut candidates: Vec<(u32, usize)> = Vec::with_capacity(candidate_cap);
                let mut min_score = 0u32;
                let mut min_pos = 0usize;

                for (idx, s) in self.summaries.iter().enumerate() {
                    let sim64 = 64u32.saturating_sub((q_sub64 ^ s.hv_sub64).count_ones());
                    let raw_bloom_hits = (q_bloom32 & s.subj_bloom32).count_ones();
                    let subj_density = s.subj_bloom32.count_ones().max(2);
                    let norm_bloom = (raw_bloom_hits * 64) / subj_density.max(q_bloom_bits);
                    let quad_sim = 16u32.saturating_sub((cur_quad ^ s.quadrant_u16).count_ones());
                    let coarse = sim64 * 10
                        + norm_bloom * 4
                        + (u32::from(s.surprise_q8) >> 1)
                        + quad_sim * 4
                        + u32::from(s.flags & 0x04) * 4;

                    if candidates.len() < candidate_cap {
                        candidates.push((coarse, idx));
                        if candidates.len() == candidate_cap {
                            let (pos, &(sc, _)) = candidates
                                .iter()
                                .enumerate()
                                .min_by_key(|&(_, &(sc, _))| sc)
                                .unwrap();
                            min_pos = pos;
                            min_score = sc;
                        }
                    } else if coarse > min_score {
                        candidates[min_pos] = (coarse, idx);
                        let (pos, &(sc, _)) = candidates
                            .iter()
                            .enumerate()
                            .min_by_key(|&(_, &(sc, _))| sc)
                            .unwrap();
                        min_pos = pos;
                        min_score = sc;
                    }
                }

                candidates
                    .into_iter()
                    .map(|(_, idx)| self.score_entry(&self.entries[idx], &q_hv, &q_tokens))
                    .collect()
            };

        hits.sort_by(|a, b| {
            b.combined_score
                .partial_cmp(&a.combined_score)
                .unwrap_or(core::cmp::Ordering::Equal)
        });
        hits.truncate(top_k);
        hits
    }
}

impl AssociativeMemoryIndex for HdcAssociativeIndex {
    fn observe_event(
        &mut self,
        artifact_id: ArtifactId,
        subject: &str,
        content: &str,
        is_anomaly: bool,
    ) -> ObservationReceipt {
        Self::observe_event(self, artifact_id, subject, content, is_anomaly)
    }

    fn query_associative(&self, query_text: &str, top_k: usize) -> Vec<AssociativeHit> {
        Self::query_associative(self, query_text, top_k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn populated_index(n: u64) -> HdcAssociativeIndex {
        let mut idx = HdcAssociativeIndex::new();
        for i in 1..=n {
            idx.observe_event_with_exit_code(
                ArtifactId(i * 7919),
                &format!("Bash:cargo test -p crate{i}"),
                &format!("test m{i}::case ... ok\nerror[E0{i:03}]: mismatch at line {i}\n"),
                if i % 5 == 0 { Some(101) } else { None },
                i % 11 == 0,
            );
        }
        idx
    }

    #[test]
    fn snapshot_round_trip_preserves_queries_and_future_observations() {
        let mut original = populated_index(150);
        let bytes = original.encode_snapshot();
        let mut restored = HdcAssociativeIndex::decode_snapshot(&bytes).expect("decode");
        assert_eq!(restored.len(), original.len());
        assert_eq!(restored.current_trajectory(), original.current_trajectory());
        for q in ["E0042 mismatch", "cargo test crate17", "line 99"] {
            assert_eq!(
                original.query_associative(q, 5),
                restored.query_associative(q, 5)
            );
        }
        let a = original.observe_event(ArtifactId(1), "Bash:ls", "new event body", false);
        let b = restored.observe_event(ArtifactId(1), "Bash:ls", "new event body", false);
        assert_eq!(a.fractal_coord, b.fractal_coord);
        assert_eq!(a.hologram_bytes, b.hologram_bytes);
        assert_eq!(a.verdict, b.verdict);
        assert_eq!(restored.encode_snapshot(), original.encode_snapshot());
    }

    #[test]
    fn snapshot_decode_rejects_corrupt_and_truncated_input() {
        let bytes = populated_index(3).encode_snapshot();
        assert!(HdcAssociativeIndex::decode_snapshot(&[]).is_err());
        assert!(HdcAssociativeIndex::decode_snapshot(&bytes[..bytes.len() - 1]).is_err());
        let mut bad_magic = bytes.clone();
        bad_magic[0] ^= 0xFF;
        assert!(HdcAssociativeIndex::decode_snapshot(&bad_magic).is_err());
        let mut trailing = bytes;
        trailing.push(0);
        assert!(HdcAssociativeIndex::decode_snapshot(&trailing).is_err());
    }

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
        assert_eq!(idx.trajectory_archive().len(), 1);
        assert_eq!(idx.surprise_gate().events_seen(), 1);
        idx.apply_idle_decay(2);
        assert_eq!(
            idx.latest_entry().map(IndexedEntry::subject_lower),
            Some("src/mmu.rs")
        );
        let manual_entry = IndexedEntry::new(
            ArtifactId(7),
            "src/Manual.rs",
            Hypervector::zero(),
            FractalCoord::default(),
            0.5,
        );
        assert_eq!(manual_entry.subject(), "src/Manual.rs");
        assert_eq!(manual_entry.subject_lower(), "src/manual.rs");
        assert_ne!(idx.current_trajectory(), init_traj);
        assert_eq!(receipt.fractal_coord, idx.current_trajectory());
        let hits = idx.query_associative("allocate_page mmu", 3);
        assert_eq!(hits.len(), 1);
        assert_eq!(idx.query_associative("allocate_page", 0).len(), 0);

        // Exercise trait object / trait method surface and >64 entry two-stage pre-screening path
        let trait_ref: &mut dyn AssociativeMemoryIndex = &mut idx;
        for i in 2..=120u64 {
            let _ = trait_ref.observe_event(
                ArtifactId(i),
                &format!("src/module_{i}.rs"),
                &format!("pub fn helper_{i}() -> usize {{ {i} }}"),
                false,
            );
        }
        let _ = trait_ref.observe_event(
            ArtifactId(999),
            "crates/icx-core/src/quantum_needle.rs",
            "pub fn locate_quantum_passkey() { panic!(\"E0502 quantum fault\"); }",
            true,
        );
        let staged_hits = trait_ref.query_associative(
            "crates/icx-core/src/quantum_needle locate_quantum_passkey a/b",
            5,
        );
        assert_eq!(staged_hits.len(), 5);
        assert_eq!(staged_hits[0].artifact_id, ArtifactId(999));
        assert!(HdcAssociativeIndex::subject_matches_token(
            "pkg:a/b:item",
            "a/b"
        ));
        assert!(!HdcAssociativeIndex::subject_matches_token(
            "pkg:alpha/beta:item",
            "a/b"
        ));
        assert!(!HdcAssociativeIndex::subject_matches_token("café", "é"));
        assert!(!HdcAssociativeIndex::subject_matches_token("α1", "α"));
        assert!(HdcAssociativeIndex::subject_matches_token(
            "src/é/mod.rs",
            "é"
        ));
        assert!(HdcAssociativeIndex::subject_matches_token(
            "myfoo/foo/foo",
            "foo/foo"
        ));
        assert!(HdcAssociativeIndex::subject_matches_token(
            "src/quantum_needle_v2.rs",
            "src/quantum_needle"
        ));
        assert!(!HdcAssociativeIndex::subject_matches_token("myfoo", ""));
        let _ = trait_ref.observe_event(ArtifactId(1000), "", "empty subject body", false);
        let _ = trait_ref.observe_event(
            ArtifactId(1001),
            "dir/a/b x:y a:b/c a/b/c/d/e/f/g/h/i/j",
            "short composite",
            false,
        );
        let _ = trait_ref.query_associative("a:b/c a/b", 5);

        // Verify requesting top_k > 64 returns all requested hits up to entries.len()
        let large_hits = trait_ref.query_associative("helper module", 100);
        assert_eq!(large_hits.len(), 100);
    }
}
