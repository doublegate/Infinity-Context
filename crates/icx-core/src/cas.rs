//! BLAKE3 Content-Addressed Store (`CasStore`) with line-offset indexing and cold grep.

use crate::chunker::{chunk_bytes, ChunkerConfig};
use crate::{ArtifactId, Blake3Digest, IcxError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A single matching line returned by `CasStore::grep_cold`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColdMatch {
    pub artifact_id: ArtifactId,
    pub line_number: usize,
    pub line_text: String,
}

/// Metadata and chunk list for a stored artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredArtifact {
    pub id: ArtifactId,
    pub full_digest: Blake3Digest,
    pub total_bytes: usize,
    pub total_lines: usize,
    pub chunk_ids: Vec<ArtifactId>,
}

/// Deterministic Content-Addressed Store backed by FastCDC chunks and BLAKE3 integrity checks.
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

    /// Ingests raw bytes into the Content-Addressed Store, returning `(ArtifactId, Blake3Digest)`.
    pub fn put_bytes(&mut self, raw: &[u8]) -> (ArtifactId, Blake3Digest) {
        let (artifact_id, full_digest) = ArtifactId::hash_bytes(raw);
        if self.artifacts.contains_key(&artifact_id) {
            return (artifact_id, full_digest);
        }

        let spans = chunk_bytes(raw, self.chunk_config);
        let mut chunk_ids = Vec::with_capacity(spans.len());

        for (off, len) in spans {
            let slice = &raw[off..off + len];
            let (cid, cdigest) = ArtifactId::hash_bytes(slice);
            self.chunks
                .entry(cid)
                .or_insert_with(|| (cdigest, slice.to_vec()));
            chunk_ids.push(cid);
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

    /// Reconstructs the 1-based inclusive line slice `[start_line, end_line]` from `id`.
    pub fn deref_lines(
        &self,
        id: ArtifactId,
        start_line: usize,
        end_line: usize,
    ) -> Result<String, IcxError> {
        let bytes = self.deref_bytes(id)?;
        let text = String::from_utf8_lossy(&bytes);
        let lines: Vec<&str> = text.lines().collect();
        let total = lines.len();

        if start_line == 0 || start_line > end_line || end_line > total {
            return Err(IcxError::InvalidLineRange {
                start: start_line,
                end: end_line,
                total,
            });
        }

        Ok(lines[start_line - 1..end_line].join("\n"))
    }

    /// Searches cold storage for `pattern` (either within `target_id` or across all stored artifacts).
    pub fn grep_cold(
        &self,
        target_id: Option<ArtifactId>,
        pattern: &str,
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
            for (idx, line) in text.lines().enumerate() {
                if line.contains(pattern) {
                    matches.push(ColdMatch {
                        artifact_id: id,
                        line_number: idx + 1,
                        line_text: line.to_owned(),
                    });
                    if matches.len() >= max_matches {
                        return Ok(matches);
                    }
                }
            }
        }

        Ok(matches)
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

        let hits = cas.grep_cold(Some(id), "passkey_9941", 10).expect("grep");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line_number, 2);

        // Corrupt underlying chunk and verify fail-closed BLAKE3 check
        cas.corrupt_chunk_for_test(id);
        let err = cas.deref_bytes(id).unwrap_err();
        assert!(matches!(err, IcxError::IntegrityViolation(_)));
    }
}
