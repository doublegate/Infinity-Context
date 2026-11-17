//! BLAKE3 Content-Addressed Store (`CasStore`) with chunk-level `LineIndexEntry` indexing,
//! selective chunk line slicing, 48-bit/64-bit prefix resolution, and cold grep.

use crate::chunker::{chunk_bytes_with_lines, ChunkerConfig};
use crate::{ArtifactId, Blake3Digest, ContentAddressedStore, IcxError, ICX_MAX_INGEST_BYTES};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

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

    /// Ingests raw bytes with the 64 MiB STRIDE DoS bound check.
    pub fn put_bytes_checked(
        &mut self,
        raw: &[u8],
    ) -> Result<(ArtifactId, Blake3Digest), IcxError> {
        if raw.len() > ICX_MAX_INGEST_BYTES {
            return Err(IcxError::PayloadTooLarge {
                size: raw.len(),
                max: ICX_MAX_INGEST_BYTES,
            });
        }
        Ok(self.put_bytes(raw))
    }

    /// Ingests raw bytes into the Content-Addressed Store, returning `(ArtifactId, Blake3Digest)`.
    pub fn put_bytes(&mut self, raw: &[u8]) -> (ArtifactId, Blake3Digest) {
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

        // Identify the contiguous sub-slice of chunks overlapping [req_start, req_end],
        // stepping out to exact newline chunk boundaries when a line spans across two chunks.
        let mut first_idx = meta
            .line_index
            .iter()
            .position(|entry| entry.end_line >= req_start)
            .unwrap_or(0);
        while first_idx > 0
            && meta.line_index[first_idx - 1].end_line == meta.line_index[first_idx].start_line
        {
            first_idx -= 1;
        }

        let mut last_idx = meta
            .line_index
            .iter()
            .rposition(|entry| entry.start_line <= req_end)
            .unwrap_or(meta.line_index.len().saturating_sub(1));
        while last_idx + 1 < meta.line_index.len()
            && meta.line_index[last_idx].end_line == meta.line_index[last_idx + 1].start_line
        {
            last_idx += 1;
        }

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
        let lines: Vec<&str> = text.lines().collect();
        let rel_start = start_line.saturating_sub(base_line);
        let count = end_line - start_line + 1;
        let rel_end = (rel_start + count).min(lines.len());

        Ok(lines[rel_start..rel_end].join("\n"))
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
        let tmp_path = dir_path.join("cas_store.json.tmp");
        let final_path = dir_path.join("cas_store.json");
        fs::write(&tmp_path, encoded).map_err(|e| IcxError::Io(e.to_string()))?;
        fs::rename(&tmp_path, &final_path).map_err(|e| IcxError::Io(e.to_string()))?;
        Ok(())
    }

    /// Loads and cryptographically verifies a `CasStore` from `dir/cas_store.json`.
    pub fn load_from_dir(dir: impl AsRef<Path>) -> Result<Self, IcxError> {
        let final_path = dir.as_ref().join("cas_store.json");
        let raw = fs::read(&final_path).map_err(|e| IcxError::Io(e.to_string()))?;
        let snapshot: CasSnapshot =
            serde_json::from_slice(&raw).map_err(|e| IcxError::Io(e.to_string()))?;

        let mut store = Self::with_config(snapshot.chunk_config);
        for (cid, cdigest, bytes) in snapshot.chunks {
            let (computed_id, computed_digest) = ArtifactId::hash_bytes(&bytes);
            if computed_id != cid || computed_digest != cdigest {
                return Err(IcxError::IntegrityViolation(cid));
            }
            store.chunks.insert(cid, (cdigest, bytes));
        }
        for art in snapshot.artifacts {
            let id = art.id;
            if ArtifactId::from_digest(&art.full_digest) != id {
                return Err(IcxError::IntegrityViolation(id));
            }
            let reconstructed_check = {
                store.artifacts.insert(id, art);
                store.deref_bytes(id)?
            };
            // Re-ingest through put_bytes so line_index and total_lines are guaranteed consistent
            store.artifacts.remove(&id);
            let (re_id, _) = store.put_bytes(&reconstructed_check);
            if re_id != id {
                return Err(IcxError::IntegrityViolation(id));
            }
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
    }
}
