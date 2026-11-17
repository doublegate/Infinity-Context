//! BLAKE3 Content-Addressed Store (`CasStore`) with chunk-level `LineIndexEntry` indexing,
//! selective chunk line slicing, 48-bit/64-bit prefix resolution, and cold grep.

use crate::chunker::{chunk_bytes_with_lines, ChunkerConfig};
use crate::{ArtifactId, Blake3Digest, ContentAddressedStore, IcxError, ICX_MAX_INGEST_BYTES};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static SAVE_TMP_COUNTER: AtomicU64 = AtomicU64::new(1);

/// A single matching line returned by `CasStore::grep_cold`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColdMatch {
    pub artifact_id: ArtifactId,
    pub line_number: usize,
    pub line_text: String,
}

/// Chunk-level line-offset index entry (`docs/data-model.md` §2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineIndexEntry {
    pub start_line: u32,
    pub end_line: u32,
    pub byte_offset: u32,
    pub byte_len: u32,
    pub chunk_id: ArtifactId,
}

/// Metadata, chunk list, and line-offset index for a stored artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredArtifact {
    pub id: ArtifactId,
    pub full_digest: Blake3Digest,
    pub total_bytes: usize,
    pub total_lines: usize,
    pub chunk_ids: Vec<ArtifactId>,
    pub line_index: Vec<LineIndexEntry>,
}

/// Serializable snapshot format for directory persistence.
#[derive(Serialize, Deserialize)]
struct CasSnapshot {
    chunk_config: ChunkerConfig,
    chunks: Vec<(ArtifactId, Blake3Digest, Vec<u8>)>,
    artifacts: Vec<StoredArtifact>,
}

/// Deterministic Content-Addressed Store backed by Normalized FastCDC chunks and BLAKE3 integrity checks.
#[derive(Clone, Debug, Default)]
pub struct CasStore {
    chunk_config: ChunkerConfig,
    chunks: BTreeMap<ArtifactId, (Blake3Digest, Vec<u8>)>,
    artifacts: BTreeMap<ArtifactId, StoredArtifact>,
}

impl CasStore {
    /// Creates a new `CasStore` with default FastCDC chunk parameters.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `CasStore` with custom chunk parameters.
    pub fn with_config(chunk_config: ChunkerConfig) -> Self {
        Self {
            chunk_config,
            chunks: BTreeMap::new(),
            artifacts: BTreeMap::new(),
        }
    }

    /// Number of unique content-addressed chunks stored (reflecting deduplication).
    pub fn unique_chunk_count(&self) -> usize {
        Self::chunks_len(&self.chunks)
    }

    #[inline]
    fn chunks_len(map: &BTreeMap<ArtifactId, (Blake3Digest, Vec<u8>)>) -> usize {
        map.len()
    }

    /// Total number of top-level artifacts stored.
    pub fn artifact_count(&self) -> usize {
        self.artifacts.len()
    }

    /// Total physical chunk bytes stored after FastCDC deduplication.
    pub fn physical_chunk_bytes(&self) -> usize {
        self.chunks.values().map(|(_, b)| b.len()).sum()
    }

    /// Resolves either a 64-bit (`#icx:<16-hex>`) or 48-bit (`#icx:<12-hex>`) handle string to a
    /// stored `ArtifactId` in $O(\log N)$ time, detecting prefix ambiguity.
    pub fn resolve_handle(&self, handle_or_prefix: &str) -> Result<ArtifactId, IcxError> {
        let (min_id, max_id) = ArtifactId::parse_prefix_range(handle_or_prefix)?;
        if min_id == max_id {
            if self.artifacts.contains_key(&min_id) {
                return Ok(min_id);
            }
            return Err(IcxError::ArtifactNotFound(min_id));
        }

        let mut matches = self.artifacts.range(min_id..=max_id).map(|(&k, _)| k);
        let Some(first) = matches.next() else {
            return Err(IcxError::ArtifactNotFound(min_id));
        };
        if matches.next().is_some() {
            return Err(IcxError::AmbiguousHandlePrefix(handle_or_prefix.to_owned()));
        }
        Ok(first)
    }

    /// Validates that a payload byte length does not exceed the 64 MiB STRIDE DoS bound.
    #[inline]
    pub const fn check_ingest_size(size: usize) -> Result<(), IcxError> {
        if size > ICX_MAX_INGEST_BYTES {
            return Err(IcxError::PayloadTooLarge {
                size,
                max: ICX_MAX_INGEST_BYTES,
            });
        }
        Ok(())
    }

    /// Ingests raw bytes with the 64 MiB STRIDE DoS bound check.
    pub fn put_bytes_checked(
        &mut self,
        raw: &[u8],
    ) -> Result<(ArtifactId, Blake3Digest), IcxError> {
        Self::check_ingest_size(raw.len())?;
        Ok(self.put_bytes(raw))
    }

    /// Ingests raw bytes into the Content-Addressed Store, returning `(ArtifactId, Blake3Digest)`.
    /// Inputs larger than `ICX_MAX_INGEST_BYTES` (`64 MiB`) are bounded to `ICX_MAX_INGEST_BYTES`.
    pub fn put_bytes(&mut self, raw: &[u8]) -> (ArtifactId, Blake3Digest) {
        let raw = if raw.len() > ICX_MAX_INGEST_BYTES {
            &raw[..ICX_MAX_INGEST_BYTES]
        } else {
            raw
        };
        let (artifact_id, full_digest) = ArtifactId::hash_bytes(raw);
        if self.artifacts.contains_key(&artifact_id) {
            return (artifact_id, full_digest);
        }

        let spans = chunk_bytes_with_lines(raw, self.chunk_config);
        let mut chunk_ids = Vec::with_capacity(spans.len());
        let mut line_index = Vec::with_capacity(spans.len());

        for span in spans {
            let slice = &raw[span.offset..span.offset + span.len];
            let (cid, cdigest) = ArtifactId::hash_bytes(slice);
            self.chunks
                .entry(cid)
                .or_insert_with(|| (cdigest, slice.to_vec()));
            chunk_ids.push(cid);
            line_index.push(LineIndexEntry {
                start_line: span.start_line,
                end_line: span.end_line,
                byte_offset: span.offset as u32,
                byte_len: span.len as u32,
                chunk_id: cid,
            });
        }

        let total_lines = if raw.is_empty() {
            0
        } else {
            let newline_count = raw.iter().filter(|&&b| b == b'\n').count();
            if raw.last() == Some(&b'\n') {
                newline_count
            } else {
                newline_count + 1
            }
        };

        self.artifacts.insert(
            artifact_id,
            StoredArtifact {
                id: artifact_id,
                full_digest,
                total_bytes: raw.len(),
                total_lines,
                chunk_ids,
                line_index,
            },
        );

        (artifact_id, full_digest)
    }

    /// Returns metadata for an artifact if present.
    pub fn get_meta(&self, id: ArtifactId) -> Option<&StoredArtifact> {
        self.artifacts.get(&id)
    }

    /// Losslessly reconstructs the full byte stream for `id` and verifies its 256-bit BLAKE3 digest.
    pub fn deref_bytes(&self, id: ArtifactId) -> Result<Vec<u8>, IcxError> {
        let meta = self
            .artifacts
            .get(&id)
            .ok_or(IcxError::ArtifactNotFound(id))?;
        let mut out = Vec::with_capacity(meta.total_bytes);

        for &cid in &meta.chunk_ids {
            let (cdigest, bytes) = self
                .chunks
                .get(&cid)
                .ok_or(IcxError::ArtifactNotFound(id))?;
            if Blake3Digest::hash(bytes) != *cdigest {
                return Err(IcxError::IntegrityViolation(cid));
            }
            out.extend_from_slice(bytes);
        }

        if Blake3Digest::hash(&out) != meta.full_digest {
            return Err(IcxError::IntegrityViolation(id));
        }

        Ok(out)
    }

    /// Reconstructs the 1-based inclusive line slice `[start_line, end_line]` from `id` by
    /// verifying and reading only the `L0` chunks that overlap `[start_line, end_line]`.
    pub fn deref_lines(
        &self,
        id: ArtifactId,
        start_line: usize,
        end_line: usize,
    ) -> Result<String, IcxError> {
        let meta = self
            .artifacts
            .get(&id)
            .ok_or(IcxError::ArtifactNotFound(id))?;
        let total = meta.total_lines;

        if start_line == 0 || start_line > end_line || end_line > total {
            return Err(IcxError::InvalidLineRange {
                start: start_line,
                end: end_line,
                total,
            });
        }

        let req_start = start_line as u32;
        let req_end = end_line as u32;

        // Identify the exact contiguous sub-slice of chunks overlapping [req_start, req_end].
        // Because `chunk_bytes_with_lines` assigns the same line number to both halves of a split
        // line (`prev.end_line == next.start_line == split_line`), `position(end_line >= req_start)`
        // and `rposition(start_line <= req_end)` automatically include the complete boundary lines
        // without expanding into unrelated chunks.
        let first_idx = meta
            .line_index
            .iter()
            .position(|entry| entry.end_line >= req_start)
            .unwrap_or(0);
        let last_idx = meta
            .line_index
            .iter()
            .rposition(|entry| entry.start_line <= req_end)
            .unwrap_or(meta.line_index.len().saturating_sub(1));

        let base_line = meta
            .line_index
            .get(first_idx)
            .map_or(1usize, |e| e.start_line as usize);
        let mut window_bytes = Vec::new();

        for entry in &meta.line_index[first_idx..=last_idx] {
            let (cdigest, bytes) = self
                .chunks
                .get(&entry.chunk_id)
                .ok_or(IcxError::ArtifactNotFound(id))?;
            if Blake3Digest::hash(bytes) != *cdigest {
                return Err(IcxError::IntegrityViolation(entry.chunk_id));
            }
            window_bytes.extend_from_slice(bytes);
        }

        let text = String::from_utf8_lossy(&window_bytes);
        let rel_start = start_line.saturating_sub(base_line);
        let count = end_line - start_line + 1;
        // Strip trailing `\r` only on segments terminated by `\n` (i.e. CRLF),
        // preserving a standalone trailing `\r` on an unterminated final segment.
        let raw_segments: Vec<&str> = text.split('\n').collect();
        let last_seg_idx = raw_segments.len().saturating_sub(1);
        let out_lines: Vec<&str> = raw_segments
            .into_iter()
            .enumerate()
            .map(|(idx, seg)| {
                if idx < last_seg_idx {
                    seg.strip_suffix('\r').unwrap_or(seg)
                } else {
                    seg
                }
            })
            .skip(rel_start)
            .take(count)
            .collect();

        Ok(out_lines.join("\n"))
    }

    /// Searches cold storage for `pattern` (either within `target_id` or across all stored artifacts).
    pub fn grep_cold(
        &self,
        target_id: Option<ArtifactId>,
        pattern: &str,
        max_matches: usize,
    ) -> Result<Vec<ColdMatch>, IcxError> {
        self.grep_cold_with_context(target_id, pattern, 0, max_matches)
    }

    /// Searches cold storage for `pattern` and includes `context_lines` of surrounding context.
    pub fn grep_cold_with_context(
        &self,
        target_id: Option<ArtifactId>,
        pattern: &str,
        context_lines: usize,
        max_matches: usize,
    ) -> Result<Vec<ColdMatch>, IcxError> {
        let mut matches = Vec::new();
        let ids: Vec<ArtifactId> = match target_id {
            Some(id) => vec![id],
            None => self.artifacts.keys().copied().collect(),
        };

        for id in ids {
            let bytes = self.deref_bytes(id)?;
            let text = String::from_utf8_lossy(&bytes);
            let lines: Vec<&str> = text.lines().collect();
            for (idx, &line) in lines.iter().enumerate() {
                if line.contains(pattern) {
                    let line_text = if context_lines == 0 {
                        line.to_owned()
                    } else {
                        let start = idx.saturating_sub(context_lines);
                        let end = (idx + context_lines + 1).min(lines.len());
                        lines[start..end].join("\n")
                    };
                    matches.push(ColdMatch {
                        artifact_id: id,
                        line_number: idx + 1,
                        line_text,
                    });
                    if matches.len() >= max_matches {
                        return Ok(matches);
                    }
                }
            }
        }

        Ok(matches)
    }

    /// Persists the `CasStore` atomically to `dir/cas_store.json`.
    pub fn save_to_dir(&self, dir: impl AsRef<Path>) -> Result<(), IcxError> {
        let dir_path = dir.as_ref();
        fs::create_dir_all(dir_path).map_err(|e| IcxError::Io(e.to_string()))?;
        let snapshot = CasSnapshot {
            chunk_config: self.chunk_config,
            chunks: self
                .chunks
                .iter()
                .map(|(&id, (dig, bytes))| (id, *dig, bytes.clone()))
                .collect(),
            artifacts: self.artifacts.values().cloned().collect(),
        };
        let encoded = serde_json::to_vec(&snapshot).map_err(|e| IcxError::Io(e.to_string()))?;
        let seq = SAVE_TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let tmp_path = dir_path.join(format!("cas_store.json.{}.{}.tmp", std::process::id(), seq));
        let final_path = dir_path.join("cas_store.json");
        let write_res = (|| -> std::io::Result<()> {
            let mut f = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&tmp_path)?;
            f.write_all(&encoded)?;
            f.sync_all()?;
            drop(f);
            fs::rename(&tmp_path, &final_path)
        })();
        if let Err(e) = write_res {
            let _ = fs::remove_file(&tmp_path);
            return Err(IcxError::Io(e.to_string()));
        }
        Ok(())
    }

    /// Loads and cryptographically verifies a `CasStore` from `dir/cas_store.json`.
    pub fn load_from_dir(dir: impl AsRef<Path>) -> Result<Self, IcxError> {
        let final_path = dir.as_ref().join("cas_store.json");
        let raw = fs::read(&final_path).map_err(|e| IcxError::Io(e.to_string()))?;
        let snapshot: CasSnapshot =
            serde_json::from_slice(&raw).map_err(|e| IcxError::Io(e.to_string()))?;

        let mut verified_chunks = BTreeMap::new();
        for (cid, cdigest, bytes) in snapshot.chunks {
            let (computed_id, computed_digest) = ArtifactId::hash_bytes(&bytes);
            if computed_id != cid || computed_digest != cdigest {
                return Err(IcxError::IntegrityViolation(cid));
            }
            verified_chunks.insert(cid, (cdigest, bytes));
        }

        let mut store = Self::with_config(snapshot.chunk_config);
        for art in snapshot.artifacts {
            let id = art.id;
            if ArtifactId::from_digest(&art.full_digest) != id
                || art.chunk_ids.len() != art.line_index.len()
            {
                return Err(IcxError::IntegrityViolation(id));
            }
            let mut computed_bytes = 0usize;
            for &cid in &art.chunk_ids {
                let (_, bytes) = verified_chunks
                    .get(&cid)
                    .ok_or(IcxError::ArtifactNotFound(id))?;
                computed_bytes = computed_bytes.saturating_add(bytes.len());
            }
            if computed_bytes != art.total_bytes || computed_bytes > ICX_MAX_INGEST_BYTES {
                return Err(IcxError::IntegrityViolation(id));
            }

            let mut out = Vec::with_capacity(computed_bytes);
            let mut current_line: u32 = 1;
            for (&cid, entry) in art.chunk_ids.iter().zip(art.line_index.iter()) {
                let (cdigest, bytes) = verified_chunks
                    .get(&cid)
                    .ok_or(IcxError::ArtifactNotFound(id))?;
                let start_line = current_line;
                let newlines = bytes.iter().filter(|&&b| b == b'\n').count() as u32;
                let end_line = if bytes.last() == Some(&b'\n') {
                    (current_line + newlines).saturating_sub(1).max(start_line)
                } else {
                    current_line + newlines
                };
                if entry.chunk_id != cid
                    || entry.byte_offset as usize != out.len()
                    || entry.byte_len as usize != bytes.len()
                    || entry.start_line != start_line
                    || entry.end_line != end_line
                {
                    return Err(IcxError::IntegrityViolation(id));
                }
                current_line += newlines;
                out.extend_from_slice(bytes);
                store
                    .chunks
                    .entry(cid)
                    .or_insert_with(|| (*cdigest, bytes.clone()));
            }
            let expected_total_lines = if out.is_empty() {
                0
            } else if out.last() == Some(&b'\n') {
                current_line.saturating_sub(1) as usize
            } else {
                current_line as usize
            };
            if Blake3Digest::hash(&out) != art.full_digest
                || art.total_lines != expected_total_lines
            {
                return Err(IcxError::IntegrityViolation(id));
            }
            store.artifacts.insert(id, art);
        }
        Ok(store)
    }

    /// Test-only helper to simulate bit-rot in a chunk and verify `IntegrityViolation` detection.
    #[cfg(test)]
    pub fn corrupt_chunk_for_test(&mut self, artifact_id: ArtifactId) {
        if let Some(meta) = self.artifacts.get(&artifact_id) {
            if let Some(&first_cid) = meta.chunk_ids.first() {
                if let Some((_, bytes)) = self.chunks.get_mut(&first_cid) {
                    if let Some(b) = bytes.first_mut() {
                        *b ^= 0xFF;
                    }
                }
            }
        }
    }
}

impl ContentAddressedStore for CasStore {
    fn put_bytes_checked(&mut self, raw: &[u8]) -> Result<(ArtifactId, Blake3Digest), IcxError> {
        Self::put_bytes_checked(self, raw)
    }

    fn deref_bytes(&self, id: ArtifactId) -> Result<Vec<u8>, IcxError> {
        Self::deref_bytes(self, id)
    }

    fn deref_lines(
        &self,
        id: ArtifactId,
        start_line: usize,
        end_line: usize,
    ) -> Result<String, IcxError> {
        Self::deref_lines(self, id, start_line, end_line)
    }

    fn grep_cold(
        &self,
        id: Option<ArtifactId>,
        pattern: &str,
        max_matches: usize,
    ) -> Result<Vec<ColdMatch>, IcxError> {
        Self::grep_cold(self, id, pattern, max_matches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cas_round_trips_bytes_and_line_slices_and_detects_tampering() {
        let mut cas = CasStore::new();
        let sample = "line 1: alpha\nline 2: beta_passkey_9941\nline 3: gamma\nline 4: delta\n";
        let (id, _) = cas.put_bytes(sample.as_bytes());

        let recovered = cas.deref_bytes(id).expect("deref_bytes succeeds");
        assert_eq!(recovered, sample.as_bytes());

        let slice = cas.deref_lines(id, 2, 3).expect("deref_lines succeeds");
        assert_eq!(slice, "line 2: beta_passkey_9941\nline 3: gamma");

        // Resolve via both 16-hex and 12-hex handles
        assert_eq!(cas.resolve_handle(&id.to_string()).unwrap(), id);
        assert_eq!(cas.resolve_handle(&id.to_short_handle()).unwrap(), id);

        let hits = cas.grep_cold(Some(id), "passkey_9941", 10).expect("grep");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line_number, 2);

        // Corrupt underlying chunk and verify fail-closed BLAKE3 check
        cas.corrupt_chunk_for_test(id);
        let err = cas.deref_bytes(id).unwrap_err();
        assert!(matches!(err, IcxError::IntegrityViolation(_)));
    }

    #[test]
    fn cas_selective_multi_chunk_deref_lines_and_persistence() {
        let cfg = ChunkerConfig {
            min_size: 256,
            avg_size: 1024,
            max_size: 4096,
        };
        let mut cas = CasStore::with_config(cfg);
        let mut lines_vec = Vec::new();
        for i in 1..=400 {
            lines_vec.push(format!(
                "line_{i:04}: deterministic multi-chunk payload data"
            ));
        }
        let full_text = lines_vec.join("\n") + "\n";
        let (id, _) = cas.put_bytes(full_text.as_bytes());
        let meta = cas.get_meta(id).unwrap();
        assert!(meta.line_index.len() >= 4);

        let sliced = cas
            .deref_lines(id, 175, 225)
            .expect("cross-chunk line slice");
        let expected = lines_vec[174..225].join("\n");
        assert_eq!(sliced, expected);

        let tmp_dir = std::env::temp_dir().join(format!("icx_cas_test_{}", id.0));
        cas.save_to_dir(&tmp_dir).expect("save_to_dir succeeds");
        let loaded = CasStore::load_from_dir(&tmp_dir).expect("load_from_dir succeeds");
        assert_eq!(loaded.deref_lines(id, 175, 225).unwrap(), expected);
        let _ = fs::remove_dir_all(&tmp_dir);

        // Corrupt chunk 0 and verify that deref_lines on lines at the tail (chunk >= 2)
        // does not read chunk 0, while deref_lines on line 1 fails closed with IntegrityViolation!
        cas.corrupt_chunk_for_test(id);
        assert!(matches!(
            cas.deref_lines(id, 1, 2),
            Err(IcxError::IntegrityViolation(_))
        ));
        let tail_slice = cas
            .deref_lines(id, 380, 395)
            .expect("tail slice skips chunk 0");
        assert_eq!(tail_slice, lines_vec[379..395].join("\n"));
    }

    #[test]
    fn cas_trait_context_grep_and_ambiguity_guards() {
        let mut cas = CasStore::new();
        let text = "line 1\nline 2 target_hit\nline 3\nline 4\n";
        let (id, _) = ContentAddressedStore::put_bytes_checked(&mut cas, text.as_bytes()).unwrap();
        assert_eq!(
            ContentAddressedStore::deref_bytes(&cas, id).unwrap(),
            text.as_bytes()
        );
        assert_eq!(
            ContentAddressedStore::deref_lines(&cas, id, 1, 2).unwrap(),
            "line 1\nline 2 target_hit"
        );
        let trait_hits = ContentAddressedStore::grep_cold(&cas, None, "target_hit", 5).unwrap();
        assert_eq!(trait_hits.len(), 1);

        let ctx_hits = cas
            .grep_cold_with_context(Some(id), "target_hit", 1, 5)
            .unwrap();
        assert_eq!(ctx_hits.len(), 1);
        assert_eq!(ctx_hits[0].line_text, "line 1\nline 2 target_hit\nline 3");

        // Verify invalid line ranges, oversize payload checks, and missing handles fail closed
        assert!(matches!(
            CasStore::check_ingest_size(ICX_MAX_INGEST_BYTES + 1),
            Err(IcxError::PayloadTooLarge { .. })
        ));
        assert!(matches!(
            cas.deref_lines(id, 0, 2),
            Err(IcxError::InvalidLineRange { .. })
        ));
        assert!(matches!(
            cas.resolve_handle("#icx:ffffffffffffffff"),
            Err(IcxError::ArtifactNotFound(_))
        ));
        assert!(matches!(
            cas.resolve_handle("#icx:ffffffffffff"),
            Err(IcxError::ArtifactNotFound(_))
        ));

        // Inject synthetic colliding 48-bit prefix entry to verify AmbiguousHandlePrefix
        let colliding_id = ArtifactId(id.0 ^ 1);
        let mut colliding_meta = cas.get_meta(id).unwrap().clone();
        colliding_meta.id = colliding_id;
        cas.artifacts.insert(colliding_id, colliding_meta);
        assert!(matches!(
            cas.resolve_handle(&id.to_short_handle()),
            Err(IcxError::AmbiguousHandlePrefix(_))
        ));
        assert_eq!(id.to_short_hex12().len(), 12);

        // Verify standalone trailing carriage return is preserved while CRLF strips `\r`
        let cr_payload = b"first\r\nlast\r";
        let (cr_id, _) = cas.put_bytes(cr_payload);
        assert_eq!(cas.deref_lines(cr_id, 1, 1).unwrap(), "first");
        assert_eq!(cas.deref_lines(cr_id, 2, 2).unwrap(), "last\r");
        assert_eq!(cas.deref_lines(cr_id, 1, 2).unwrap(), "first\nlast\r");
    }
}
