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
use std::collections::{BTreeMap, BTreeSet, HashMap};

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
    /// 256-bit Bloom filter over the entry's subject + content tokens (ICX-AUD-049): lets the
    /// pre-screen find exact rare identifiers that a bundled hypervector dilutes.
    content_bloom: [u64; 4],
}

/// Number of leading hypervector words compared in the Stage-1 pre-screen (16 x 64 = 1,024 bits;
/// ICX-AUD-049: 64 bits left a single matching token below the noise floor).
const PRESCREEN_WORDS: usize = 16;

/// Characters a lookup token consists of.
fn is_token_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == ':' || c == '/'
}

/// Splits text into the lowercase lookup tokens shared by indexing and querying. A run joined by
/// `:` or `/` (`Bash:seq`, `src/main.rs`) yields the whole run and, from v1.2.1 (ICXHDC05), each
/// of its components, so the bare command or path segment is found too.
fn lookup_tokens(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !is_token_char(c))
        .filter(|s| s.len() >= 2)
        .flat_map(|run| {
            let parts = run
                .contains([':', '/'])
                .then(|| run.split([':', '/']).filter(|p| p.len() >= 2))
                .into_iter()
                .flatten();
            std::iter::once(run).chain(parts)
        })
        .map(str::to_ascii_lowercase)
}

/// Whether `token` is a single well-formed lookup token as [`lookup_tokens`] emits it.
fn is_lexical_token(token: &str) -> bool {
    (2..=MAX_LEXICAL_TOKEN_BYTES).contains(&token.len())
        && token.chars().all(is_token_char)
        && token == token.to_ascii_lowercase()
}

/// Longest token (bytes) admitted to the exact lexical index. Longer runs (hex blobs, minified
/// lines) are not identifiers, and a run beyond the snapshot string bound would make the whole
/// snapshot undecodable. Indexing and querying apply the same rule.
const MAX_LEXICAL_TOKEN_BYTES: usize = 256;

/// Maximum number of distinct tokens in the exact lexical index, equal to the decoder's bound, so
/// `encode_snapshot` never writes a token table `decode_snapshot` rejects.
const MAX_LEXICAL_TOKENS: usize = snapshot::MAX_SNAPSHOT_ITEMS as usize;

/// Longest stored subject (bytes), equal to the decoder's string bound.
const MAX_STORED_SUBJECT_BYTES: usize = snapshot::MAX_SNAPSHOT_STRING as usize;

/// [`lookup_tokens`] restricted to tokens eligible for the exact lexical index.
fn index_tokens(text: &str) -> impl Iterator<Item = String> + '_ {
    lookup_tokens(text).filter(|t| t.len() <= MAX_LEXICAL_TOKEN_BYTES)
}

/// Inverse document frequency weight of a token present in `df` of `n` entries.
fn idf(n: usize, df: usize) -> f64 {
    (1.0 + n as f64 / df.max(1) as f64).ln()
}

/// Three bit positions (k = 3) in a 256-bit Bloom filter for `token` (FNV-1a 64).
fn bloom_bits(token: &str) -> [u8; 3] {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in token.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    [
        (h & 0xFF) as u8,
        ((h >> 21) & 0xFF) as u8,
        ((h >> 42) & 0xFF) as u8,
    ]
}

fn bloom_insert(bloom: &mut [u64; 4], token: &str) {
    for bit in bloom_bits(token) {
        bloom[usize::from(bit >> 6)] |= 1u64 << (bit & 63);
    }
}

fn bloom_contains(bloom: &[u64; 4], token: &str) -> bool {
    bloom_bits(token)
        .iter()
        .all(|&bit| bloom[usize::from(bit >> 6)] & (1u64 << (bit & 63)) != 0)
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
#[derive(Clone, Debug)]
pub struct HdcAssociativeIndex {
    entries: Vec<IndexedEntry>,
    summaries: Vec<EntrySummary>,
    trajectory_archive: FractalTrajectoryArchive,
    surprise_gate: TitansSurpriseGate,
    /// Exact token -> entry postings. `None` denotes a legacy snapshot without content tokens.
    lexical_postings: Option<BTreeMap<String, Vec<usize>>>,
}

impl Default for HdcAssociativeIndex {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            summaries: Vec::new(),
            trajectory_archive: FractalTrajectoryArchive::default(),
            surprise_gate: TitansSurpriseGate::default(),
            lexical_postings: Some(BTreeMap::new()),
        }
    }
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
            for &word in &s.content_bloom {
                w.u64(word);
            }
        }
        self.trajectory_archive.write_snapshot(&mut w);
        self.surprise_gate.write_snapshot(&mut w);
        w.u8(u8::from(self.lexical_postings.is_some()));
        if let Some(postings) = &self.lexical_postings {
            w.len(postings.len());
            for (token, indices) in postings {
                w.str(token);
                w.len(indices.len());
                for &idx in indices {
                    w.len(idx);
                }
            }
        }
        w.buf
    }

    /// Restores an index from [`Self::encode_snapshot`] output. Fails closed on any truncation,
    /// bound violation, bad magic, or trailing data.
    pub fn decode_snapshot(bytes: &[u8]) -> Result<Self, SnapshotError> {
        let mut r = snapshot::Reader::new(bytes);
        let version = r.expect_magic()?;
        let n = r.len("index.entries", snapshot::MAX_SNAPSHOT_ITEMS)?;
        let mut entries = Vec::with_capacity(n.min(4096));
        let mut summaries = Vec::with_capacity(n.min(4096));
        for _ in 0..n {
            let artifact_id = ArtifactId(r.u64()?);
            let subject = r.str("entry.subject")?;
            let hv = Hypervector::read_snapshot(&mut r)?;
            let fractal_coord = FractalCoord::read_snapshot(&mut r)?;
            let surprise = r.f32()?;
            if !surprise.is_finite() || !(0.0..=1.0).contains(&surprise) {
                return Err(SnapshotError::Inconsistent("entry.surprise"));
            }
            entries.push(IndexedEntry::new(
                artifact_id,
                subject,
                hv,
                fractal_coord,
                surprise,
            ));
            let summary = EntrySummary {
                hv_sub64: r.u64()?,
                subj_bloom32: r.u32()?,
                quadrant_u16: r.u16()?,
                surprise_q8: r.u8()?,
                flags: r.u8()?,
                content_bloom: [r.u64()?, r.u64()?, r.u64()?, r.u64()?],
            };
            let entry = entries.last().expect("entry was just appended");
            if summary.hv_sub64 != entry.hv.sub_hypervector_64()
                || summary.subj_bloom32 != Self::compute_subject_bloom32(entry.subject_lower())
                || summary.quadrant_u16 != entry.fractal_coord.quadrant_signature_u16()
                || summary.surprise_q8 != (surprise * 255.0) as u8
                || summary.flags & !0x07 != 0
                || summary.flags & 0x03 == 3
            {
                return Err(SnapshotError::Inconsistent("entry.summary"));
            }
            summaries.push(summary);
        }
        let trajectory_archive = FractalTrajectoryArchive::read_snapshot(&mut r)?;
        let surprise_gate = TitansSurpriseGate::read_snapshot(&mut r)?;
        let lexical_postings = if version >= 4 {
            match r.u8()? {
                0 => None,
                1 => {
                    let count = r.len("lexical.tokens", snapshot::MAX_SNAPSHOT_ITEMS)?;
                    let mut postings = BTreeMap::new();
                    for _ in 0..count {
                        let token = r.str("lexical.token")?;
                        // Every tokenizer version emits well-formed tokens. ICXHDC04 postings
                        // (pre-v1.2.1 tokenizer) are validated here, then dropped below so the MMU
                        // rebuilds them.
                        if !is_lexical_token(&token) {
                            return Err(SnapshotError::Inconsistent("lexical.token"));
                        }
                        let len = r.len("lexical.postings", n as u64)?;
                        if len == 0 {
                            return Err(SnapshotError::Inconsistent("lexical.empty"));
                        }
                        let mut indices = Vec::with_capacity(len.min(4096));
                        for _ in 0..len {
                            let idx = r.len("lexical.entry", n.saturating_sub(1) as u64)?;
                            if idx >= n || indices.last().is_some_and(|last| *last >= idx) {
                                return Err(SnapshotError::Inconsistent("lexical.entry"));
                            }
                            indices.push(idx);
                        }
                        if postings.insert(token, indices).is_some() {
                            return Err(SnapshotError::Inconsistent("lexical.duplicate"));
                        }
                    }
                    Some(postings)
                }
                _ => return Err(SnapshotError::Inconsistent("lexical.present")),
            }
        } else {
            None
        };
        if let Some(postings) = &lexical_postings {
            let mut expected_blooms = vec![[0u64; 4]; n];
            for (token, indices) in postings {
                for &idx in indices {
                    bloom_insert(&mut expected_blooms[idx], token);
                }
            }
            if summaries
                .iter()
                .zip(expected_blooms)
                .any(|(summary, expected)| summary.content_bloom != expected)
            {
                return Err(SnapshotError::Inconsistent("lexical.content_bloom"));
            }
        }
        r.finish()?;
        let lexical_postings = if version >= 5 { lexical_postings } else { None };
        Ok(Self {
            entries,
            summaries,
            trajectory_archive,
            surprise_gate,
            lexical_postings,
        })
    }

    /// Whether exact content candidates are persisted. Legacy03 snapshots should be rebuilt
    /// from CAS by the coordinator; standalone legacy indexes use exhaustive refinement.
    pub const fn has_exact_lexical_index(&self) -> bool {
        self.lexical_postings.is_some()
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
        self.observe_composite(
            artifact_id,
            subject,
            content,
            &composite_hv,
            exit_code,
            force_anomaly,
        )
    }

    /// The body of [`Self::observe_event_with_exit_code`] once the event's hypervector is known.
    fn observe_composite(
        &mut self,
        artifact_id: ArtifactId,
        subject: &str,
        content: &str,
        composite_hv: &Hypervector,
        exit_code: Option<i32>,
        force_anomaly: bool,
    ) -> ObservationReceipt {
        let verdict = self.surprise_gate.observe_with_exit_code(
            composite_hv,
            content,
            exit_code,
            force_anomaly,
        );
        let current_trajectory = self
            .trajectory_archive
            .record_step(artifact_id, composite_hv);

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

        // The stored subject is bounded like every snapshot string; the hypervector above still
        // encodes the full text.
        let stored_subject = &subject[..subject.floor_char_boundary(MAX_STORED_SUBJECT_BYTES)];
        let entry = IndexedEntry::new(
            artifact_id,
            stored_subject,
            *composite_hv,
            current_trajectory,
            verdict.momentum_surprise,
        );
        // Index the same (bounded) subject that `score_entry` matches against.
        // A cut inside a token would index a fragment that never occurred; drop it.
        let indexed_subject = if subject[stored_subject.len()..]
            .chars()
            .next()
            .is_some_and(is_token_char)
        {
            stored_subject.trim_end_matches(is_token_char)
        } else {
            stored_subject
        };
        let content_bloom = self.index_lexical_tokens(indexed_subject, content, MAX_LEXICAL_TOKENS);
        self.summaries.push(EntrySummary {
            content_bloom,
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

    /// Adds the next entry's eligible tokens to the postings (admitting a new distinct token only
    /// while fewer than `max_tokens` exist) and returns its content Bloom filter. The Bloom covers
    /// exactly the tokens posted for the entry, which `decode_snapshot` verifies.
    fn index_lexical_tokens(
        &mut self,
        subject: &str,
        content: &str,
        max_tokens: usize,
    ) -> [u64; 4] {
        // Distinct tokens in first-occurrence order, so the vocabulary cap admits the entry's
        // earliest tokens rather than its lexicographically smallest ones.
        let mut seen = BTreeSet::new();
        let tokens: Vec<String> = index_tokens(subject)
            .chain(index_tokens(content))
            .filter(|t| seen.insert(t.clone()))
            .collect();
        let entry_idx = self.entries.len();
        let mut bloom = [0u64; 4];
        let Some(postings) = self.lexical_postings.as_mut() else {
            for token in &tokens {
                bloom_insert(&mut bloom, token);
            }
            return bloom;
        };
        for token in tokens {
            if let Some(list) = postings.get_mut(&token) {
                list.push(entry_idx);
            } else if postings.len() < max_tokens {
                postings.insert(token.clone(), vec![entry_idx]);
            } else {
                continue;
            }
            bloom_insert(&mut bloom, &token);
        }
        bloom
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
    fn content_hits(summary: &EntrySummary, q_tokens: &[String]) -> usize {
        q_tokens
            .iter()
            .filter(|t| bloom_contains(&summary.content_bloom, t))
            .count()
    }

    fn score_entry(
        &self,
        idx: usize,
        q_hv: &Hypervector,
        q_tokens: &[String],
        exact_hits: Option<usize>,
    ) -> AssociativeHit {
        let e = &self.entries[idx];
        let text_sim = q_hv.similarity(&e.hv);
        let lexical_overlap = q_tokens
            .iter()
            .filter(|t| Self::subject_matches_token(&e.subject_lower, t.as_str()))
            .count() as f32;
        let content_frac = if q_tokens.is_empty() {
            0.0
        } else {
            exact_hits.unwrap_or_else(|| Self::content_hits(&self.summaries[idx], q_tokens)) as f32
                / q_tokens.len() as f32
        };
        let hdc_sim = (text_sim + 0.25 * lexical_overlap + 0.5 * content_frac).clamp(0.0, 1.0);
        let suffix = self.exact_shared_suffix(idx);
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

    /// How many trailing events entry `idx`'s history shares with the current one, counted on the
    /// stored states themselves (ICX-AUD-052). Each IFS step keeps only a 16-bit corner of its
    /// state, so two distinct states can produce the same coordinates; the coordinate distance is
    /// used only as an upper bound, and each step within it is confirmed by comparing the two
    /// events' hypervectors. Entries are appended one per trajectory step, so entry `i` is step
    /// `i`, and the latest entry shares its whole (bounded) history with itself.
    fn exact_shared_suffix(&self, idx: usize) -> u32 {
        let Some(last) = self.entries.len().checked_sub(1) else {
            return 0;
        };
        let bound = self
            .current_trajectory()
            .verified_suffix_depth(&self.entries[idx].fractal_coord) as usize;
        let mut k = 0usize;
        while k < bound && k <= idx && self.entries[idx - k].hv == self.entries[last - k].hv {
            k += 1;
        }
        k as u32
    }

    /// Distinct, sorted lookup tokens of a query, filtered by the same rule as indexing.
    fn query_tokens(query_text: &str) -> Vec<String> {
        index_tokens(query_text)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Posting lists of the query tokens present in the exact index, rarest (lowest df) first.
    fn query_postings(&self, q_tokens: &[String]) -> Vec<&[usize]> {
        let Some(postings) = &self.lexical_postings else {
            return Vec::new();
        };
        let mut lists: Vec<&[usize]> = q_tokens
            .iter()
            .filter_map(|t| postings.get(t).map(Vec::as_slice))
            .collect();
        lists.sort_by_key(|list| list.len());
        lists
    }

    /// Entry indices to refine exactly: the Stage-1 pre-screen survivors plus the postings of the
    /// rarest query tokens. Tokens are expanded in ascending df order and expansion stops at the
    /// first token whose df exceeds the candidate cap (or once the cap's worth of postings has been
    /// admitted), so a common token ("pub", "fn") cannot pull the whole corpus into 10,240-bit
    /// scoring. Such tokens still count toward the lexical score of the selected candidates.
    /// Every entry holding a query token with df <= cap stays a candidate (ICX-CODEX-004), so a
    /// unique identifier is never dropped by approximate filtering. Result: sorted, distinct,
    /// at most `3 * candidate_cap` long unless the full corpus is scanned.
    fn candidate_set(
        &self,
        q_hv: &Hypervector,
        q_tokens: &[String],
        q_postings: &[&[usize]],
        top_k: usize,
    ) -> Vec<usize> {
        let n = self.entries.len();
        let adaptive_floor = (n / 64).clamp(16, 512);
        // Grows with the corpus (>= 4 * sqrt(n)) so recall does not collapse as n increases.
        let sqrt_floor = ((n as f64).sqrt() * 4.0).ceil() as usize;
        let candidate_cap = top_k
            .saturating_mul(4)
            .clamp(16, 64)
            .max(adaptive_floor)
            .max(sqrt_floor)
            .max(top_k)
            .min(n);
        if self.lexical_postings.is_none() || n <= 64 || candidate_cap == n {
            return (0..n).collect();
        }

        let mut q_bloom32 = 0u32;
        for tok in q_tokens {
            q_bloom32 |= Self::composite_token_bloom32(tok);
        }
        let q_bloom_bits = q_bloom32.count_ones().max(1);
        let cur_quad = self.current_trajectory().quadrant_signature_u16();
        let mut candidates: Vec<(u32, usize)> = Vec::with_capacity(candidate_cap);
        let mut min_score = 0u32;
        let mut min_pos = 0usize;

        for (idx, s) in self.summaries.iter().enumerate() {
            let ham1024: u32 = q_hv.words[..PRESCREEN_WORDS]
                .iter()
                .zip(&self.entries[idx].hv.words[..PRESCREEN_WORDS])
                .map(|(a, b)| (a ^ b).count_ones())
                .sum();
            let sim1024 = (PRESCREEN_WORDS as u32 * 64).saturating_sub(ham1024);
            let content = Self::content_hits(s, q_tokens) as u32;
            let raw_bloom_hits = (q_bloom32 & s.subj_bloom32).count_ones();
            let subj_density = s.subj_bloom32.count_ones().max(2);
            let norm_bloom = (raw_bloom_hits * 64) / subj_density.max(q_bloom_bits);
            let quad_sim = 16u32.saturating_sub((cur_quad ^ s.quadrant_u16).count_ones());
            let coarse = sim1024 / 2
                + content * 1200
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

        let mut selected: Vec<usize> = candidates.into_iter().map(|(_, idx)| idx).collect();
        let mut expanded = 0usize;
        for list in q_postings {
            if list.len() > candidate_cap || expanded >= candidate_cap {
                break;
            }
            selected.extend_from_slice(list);
            expanded += list.len();
        }
        selected.sort_unstable();
        selected.dedup();
        selected
    }

    /// Queries the associative index using a two-stage pipeline:
    /// 1. Stage 1 L2-cache spectral/Bloom pre-screening over 16-byte `EntrySummary` records.
    /// 2. Stage 2 exact 10,240-bit `hamming_distance_simd` and boundary-aware inverse-IFS `verified_suffix_depth` refinement.
    pub fn query_associative(&self, query_text: &str, top_k: usize) -> Vec<AssociativeHit> {
        if top_k == 0 || self.entries.is_empty() {
            return Vec::new();
        }
        let q_hv = Hypervector::encode_text(query_text);
        let q_tokens = Self::query_tokens(query_text);
        let q_postings = self.query_postings(&q_tokens);
        let candidates = self.candidate_set(&q_hv, &q_tokens, &q_postings, top_k);
        let n = self.entries.len();

        // Exact lexical evidence per candidate: matched-token count (for `content_frac`) and the
        // IDF-weighted score that ranks a rare exact identifier above common-word matches.
        // Exact 10,240-bit scoring dominates large candidate sets (legacy indexes without
        // postings score every entry); `par_map_ordered` splits those across threads and
        // returns results in candidate order, so ranking is identical to a sequential pass.
        let mut hits: Vec<(f64, AssociativeHit)> =
            crate::hv::par_map_ordered(&candidates, |&idx| {
                let (count, weight) = q_postings
                    .iter()
                    .filter(|list| list.binary_search(&idx).is_ok())
                    .fold((0usize, 0.0f64), |(c, w), list| {
                        (c + 1, w + idf(n, list.len()))
                    });
                let exact = self.lexical_postings.as_ref().map(|_| count);
                (weight, self.score_entry(idx, &q_hv, &q_tokens, exact))
            });

        let mut weight_by_artifact = HashMap::<ArtifactId, f64>::with_capacity(hits.len());
        for (weight, hit) in &hits {
            let best = weight_by_artifact.entry(hit.artifact_id).or_insert(0.0);
            if *weight > *best {
                *best = *weight;
            }
        }
        hits.sort_by(|(_, a), (_, b)| {
            weight_by_artifact[&b.artifact_id]
                .total_cmp(&weight_by_artifact[&a.artifact_id])
                .then_with(|| {
                    b.combined_score
                        .partial_cmp(&a.combined_score)
                        .unwrap_or(core::cmp::Ordering::Equal)
                })
                .then_with(|| a.artifact_id.cmp(&b.artifact_id))
        });
        let mut hits: Vec<AssociativeHit> = hits.into_iter().map(|(_, hit)| hit).collect();
        // One hit per artifact (best score wins): repeated ingests of identical content must not
        // crowd out distinct recalls (ICX-AUD-042).
        let mut seen = std::collections::BTreeSet::new();
        hits.retain(|h| seen.insert(h.artifact_id));
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

    fn candidate_count(idx: &HdcAssociativeIndex, query: &str, top_k: usize) -> usize {
        let q_hv = Hypervector::encode_text(query);
        let q_tokens = HdcAssociativeIndex::query_tokens(query);
        let q_postings = idx.query_postings(&q_tokens);
        idx.candidate_set(&q_hv, &q_tokens, &q_postings, top_k)
            .len()
    }

    #[test]
    fn common_query_tokens_do_not_pull_the_whole_corpus_into_exact_scoring() {
        let mut idx = HdcAssociativeIndex::new();
        let n = 3_000u64;
        for i in 0..n {
            idx.observe_event(
                ArtifactId(i + 1),
                &format!("src/m{i}.rs"),
                &format!("pub fn helper_{i}() -> usize {{ {i} }}"),
                false,
            );
        }
        // candidate_cap for n = 3,000 and top_k = 5 is ceil(4 * sqrt(3000)) = 220.
        let bound = 3 * 220;
        for query in ["pub fn helper_17", "helper_17 zzzqqq", "pub fn usize"] {
            let count = candidate_count(&idx, query, 5);
            assert!(count <= bound, "{query}: {count} candidates > {bound}");
        }
        assert_eq!(
            idx.query_associative("pub fn helper_17", 3)[0].artifact_id,
            ArtifactId(18)
        );
    }

    #[test]
    fn distinct_token_cap_is_enforced_at_insert_and_snapshot_stays_consistent() {
        let mut idx = HdcAssociativeIndex::new();
        let cap = 3;
        for (i, body) in ["alpha beta", "gamma delta", "alpha epsilon"]
            .iter()
            .enumerate()
        {
            let bloom = idx.index_lexical_tokens("", body, cap);
            idx.summaries.push(EntrySummary {
                content_bloom: bloom,
                hv_sub64: Hypervector::zero().sub_hypervector_64(),
                subj_bloom32: HdcAssociativeIndex::compute_subject_bloom32(""),
                quadrant_u16: FractalCoord::default().quadrant_signature_u16(),
                surprise_q8: 0,
                flags: 0,
            });
            idx.entries.push(IndexedEntry::new(
                ArtifactId(i as u64 + 1),
                "",
                Hypervector::zero(),
                FractalCoord::default(),
                0.0,
            ));
        }
        let postings = idx.lexical_postings.as_ref().unwrap();
        assert_eq!(postings.len(), cap);
        // Existing tokens keep accumulating postings once the cap is reached.
        assert_eq!(postings["alpha"], vec![0, 2]);
        // Distinct tokens are admitted in first-occurrence order (v1.2.1): alpha, beta, gamma.
        assert!(postings.contains_key("gamma"));
        assert!(!postings.contains_key("delta") && !postings.contains_key("epsilon"));
        HdcAssociativeIndex::decode_snapshot(&idx.encode_snapshot())
            .expect("capped index must round-trip");
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

    /// ICX-AUD-052: a shared fractal suffix is reported only for a history that really repeats.
    /// Each IFS step keeps a 16-bit corner of its state, so two distinct states with the same
    /// corners give identical trajectories; that must not count as a shared suffix.
    #[test]
    fn distinct_states_with_equal_corners_share_no_suffix() {
        let a = Hypervector::encode_text("Bash:cargo test ok 1");
        // Swap a 1 and a 0 inside the first 640-bit block: every block's popcount, and so every
        // corner, is unchanged, but the state differs.
        let mut b = a;
        let one = (0..640)
            .find(|&i| a.words[i / 64] >> (i % 64) & 1 == 1)
            .unwrap();
        let zero = (0..640)
            .find(|&i| a.words[i / 64] >> (i % 64) & 1 == 0)
            .unwrap();
        b.words[one / 64] ^= 1 << (one % 64);
        b.words[zero / 64] ^= 1 << (zero % 64);
        assert_ne!(a, b);
        assert_eq!(
            FractalCoord::corner_vertex(&a),
            FractalCoord::corner_vertex(&b)
        );

        let mut idx = HdcAssociativeIndex::new();
        idx.observe_composite(ArtifactId(1), "first", "one", &a, None, false);
        idx.observe_composite(ArtifactId(2), "second", "two", &b, None, false);
        // The coordinates alone claim a one-step shared history.
        let coord_claim = idx
            .current_trajectory()
            .verified_suffix_depth(&idx.entries[0].fractal_coord);
        assert!(coord_claim >= 1, "the constructed collision exists");
        let first = idx
            .query_associative("first one", 10)
            .into_iter()
            .find(|h| h.artifact_id == ArtifactId(1))
            .expect("recalled");
        assert_eq!(
            first.shared_fractal_suffix, 0,
            "distinct states reported as shared"
        );

        // A genuinely repeated sequence still is: A, B, C, then A, B, C again.
        let mut rep = HdcAssociativeIndex::new();
        for (i, (s, c)) in [("a", "one"), ("b", "two"), ("c", "three")]
            .iter()
            .cycle()
            .take(6)
            .enumerate()
        {
            rep.observe_event(ArtifactId(i as u64), s, c, false);
        }
        let first_c = rep
            .query_associative("c three", 10)
            .into_iter()
            .find(|h| h.artifact_id == ArtifactId(2))
            .expect("the first C is recalled");
        assert_eq!(first_c.shared_fractal_suffix, 3, "A, B, C repeated exactly");
        // The latest entry shares its whole history with itself.
        let latest = rep
            .query_associative("c three", 10)
            .into_iter()
            .find(|h| h.artifact_id == ArtifactId(5))
            .expect("the latest C is recalled");
        assert_eq!(latest.shared_fractal_suffix, 6);
    }

    /// A small deterministic generator for the measurement corpora (xorshift64*).
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
    }

    /// Builds `n` events over a `vocab`-word vocabulary: subject `Bash:run-<a>-<b>`, content of
    /// 24 words. Returns the index and each event's words, for building queries.
    fn synthetic_corpus(
        n: usize,
        vocab: usize,
        seed: u64,
    ) -> (HdcAssociativeIndex, Vec<Vec<usize>>) {
        let mut rng = Rng(seed);
        let mut idx = HdcAssociativeIndex::new();
        let mut words = Vec::with_capacity(n);
        for i in 0..n {
            let w: Vec<usize> = (0..24).map(|_| rng.below(vocab)).collect();
            let content: Vec<String> = w.iter().map(|x| format!("w{x}")).collect();
            idx.observe_event(
                ArtifactId(i as u64 + 1),
                &format!("Bash:run-{}-{}", w[0], w[1]),
                &content.join(" "),
                false,
            );
            words.push(w);
        }
        (idx, words)
    }

    /// T-71-003 measurement (run with `--ignored --nocapture` in release): stage-1 recall, the
    /// share of queries whose target survives the prescreen and postings union, and final
    /// recall@1/@5, at 1k, 10k and 100k artifacts. Two query kinds: three of the target's words
    /// from a 50,000-word vocabulary (rare tokens, df below the cap) and from a 500-word
    /// vocabulary (common tokens, df above the cap, so only the prescreen can keep the target).
    #[test]
    #[ignore = "measurement; prints a table"]
    fn measure_stage1_recall_by_corpus_size() {
        println!(
            "| n | vocabulary | queries | max candidates | artifacts with all 3 words (mean) | stage-1 recall | recall@1 | recall@5 | mean query |"
        );
        for n in [1_000usize, 10_000, 100_000] {
            for vocab in [50_000usize, 500] {
                let (idx, words) = synthetic_corpus(n, vocab, 0x1c5_0071 ^ n as u64);
                let mut rng = Rng(0xfeed ^ n as u64 ^ vocab as u64);
                let queries = 200;
                let (mut s1, mut r1, mut r5, mut ties) = (0, 0, 0, 0usize);
                let mut cap = 0;
                // Only `query_associative` is timed; the inspection of stage 1 and the count of
                // ambiguous artifacts run outside the timer (review of #97).
                let mut timed = std::time::Duration::ZERO;
                for _ in 0..queries {
                    let t = rng.below(n);
                    let q: Vec<String> = (0..3)
                        .map(|_| format!("w{}", words[t][2 + rng.below(22)]))
                        .collect();
                    let query = q.join(" ");
                    let qw: Vec<usize> = q.iter().map(|w| w[1..].parse().unwrap()).collect();
                    ties += words
                        .iter()
                        .filter(|d| qw.iter().all(|x| d.contains(x)))
                        .count();
                    let q_hv = Hypervector::encode_text(&query);
                    let q_tokens = HdcAssociativeIndex::query_tokens(&query);
                    let q_postings = idx.query_postings(&q_tokens);
                    let cands = idx.candidate_set(&q_hv, &q_tokens, &q_postings, 5);
                    cap = cap.max(cands.len());
                    s1 += usize::from(cands.binary_search(&t).is_ok());
                    let started = std::time::Instant::now();
                    let hits = idx.query_associative(&query, 5);
                    timed += started.elapsed();
                    let target = ArtifactId(t as u64 + 1);
                    r1 += usize::from(hits.first().is_some_and(|h| h.artifact_id == target));
                    r5 += usize::from(hits.iter().any(|h| h.artifact_id == target));
                }
                let per = timed.as_secs_f64() * 1e3 / f64::from(queries);
                let pct = |x: usize| 100.0 * x as f64 / f64::from(queries);
                println!(
                    "| {n} | {vocab} | {queries} | {cap} | {:.1} | {:.1}% | {:.1}% | {:.1}% | {per:.2} ms |",
                    ties as f64 / f64::from(queries),
                    pct(s1),
                    pct(r1),
                    pct(r5)
                );
            }
        }
    }

    /// When the cap covers the whole corpus, the prescreen admits every entry, so ranking equals
    /// an exhaustive scan (T-71-003).
    #[test]
    fn a_cap_that_covers_the_corpus_scans_every_entry() {
        let (idx, _) = synthetic_corpus(60, 500, 7);
        for query in ["w1 w2 w3", "Bash:run", "w499"] {
            assert_eq!(candidate_count(&idx, query, 5), 60, "{query}");
        }
    }

    /// ICX-AUD-052 measurement (run with `--ignored --nocapture` in release): over 20,000 events,
    /// how many earlier entries the coordinates alone report as sharing the latest history, and
    /// how many the confirmed suffix does. Every coordinate-level claim here is false, since no
    /// event repeats.
    #[test]
    #[ignore = "measurement; prints counts"]
    fn measure_fractal_suffix_false_positives() {
        for (label, similar) in [("distinct", false), ("similar", true)] {
            let mut idx = HdcAssociativeIndex::new();
            let n = 20_000usize;
            for i in 0..n {
                let (subj, content) = if similar {
                    (
                        "Bash:cargo test".to_owned(),
                        format!("test result: ok. {i} passed"),
                    )
                } else {
                    (
                        format!("cmd-{i}"),
                        format!("payload {} {}", i * 7919, i ^ 0x5a5a),
                    )
                };
                idx.observe_event(ArtifactId(i as u64), &subj, &content, false);
            }
            let cur = idx.current_trajectory();
            let coord = (0..n - 1)
                .filter(|&i| cur.verified_suffix_depth(&idx.entries[i].fractal_coord) > 0)
                .count();
            let exact = (0..n - 1)
                .filter(|&i| idx.exact_shared_suffix(i) > 0)
                .count();
            println!("{label}: n={n} coordinate-level claims={coord} confirmed={exact}");
            assert_eq!(exact, 0);
        }
    }
}
