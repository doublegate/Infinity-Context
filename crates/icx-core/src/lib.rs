//! `icx-core` — Foundational types, FastCDC Content-Defined Chunking, BLAKE3 Content-Addressed
//! Storage (`CAS`), Lossless Context Management (`LCM`) Merkle-DAG, and Prefix-Cache-Locked
//! `CacheEpochController`.

pub mod cas;
pub mod chunker;
pub mod dag;
pub mod epoch;

use core::fmt;
use core::str::FromStr;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Maximum single-artifact ingestion size (`64 MiB`) per STRIDE DoS mitigation (`docs/security-and-threat-model.md` §2).
pub const ICX_MAX_INGEST_BYTES: usize = 64 * 1024 * 1024;

/// Normative trait contract for Content-Addressed Storage (`docs/interfaces.md` §1).
pub trait ContentAddressedStore: Send + Sync {
    /// Ingests raw bytes (after secret redaction), chunks via FastCDC, and returns `(ArtifactId, Blake3Digest)`.
    fn put_bytes_checked(&mut self, raw: &[u8]) -> Result<(ArtifactId, Blake3Digest), IcxError>;

    /// Reconstructs the exact uncompressed byte stream for `id` and verifies its BLAKE3 digest.
    fn deref_bytes(&self, id: ArtifactId) -> Result<Vec<u8>, IcxError>;

    /// Reconstructs the exact 1-based inclusive line range `[start_line, end_line]`.
    fn deref_lines(
        &self,
        id: ArtifactId,
        start_line: usize,
        end_line: usize,
    ) -> Result<String, IcxError>;

    /// Searches cold storage for `pattern` and returns matching lines with line numbers.
    fn grep_cold(
        &self,
        id: Option<ArtifactId>,
        pattern: &str,
        max_matches: usize,
    ) -> Result<Vec<cas::ColdMatch>, IcxError>;
}

/// Short 64-bit content-addressed handle derived from the first 8 big-endian bytes of a BLAKE3 hash.
/// Rendered externally as `#icx:<16-lowercase-hex>` (Profile B) or `#icx:<12-lowercase-hex>` (Profile A).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct ArtifactId(pub u64);

impl ArtifactId {
    /// Extracts the canonical 64-bit short handle from a full 256-bit `Blake3Digest`.
    #[inline]
    pub const fn from_digest(digest: &Blake3Digest) -> Self {
        let b = digest.0;
        let val = u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
        Self(val)
    }

    /// Hashes arbitrary bytes with BLAKE3 and returns both `(ArtifactId, Blake3Digest)`.
    #[inline]
    pub fn hash_bytes(bytes: &[u8]) -> (Self, Blake3Digest) {
        let digest = Blake3Digest::hash(bytes);
        (Self::from_digest(&digest), digest)
    }

    /// Formats the raw 16-char lowercase hex suffix without the `#icx:` prefix.
    #[inline]
    pub fn to_hex(self) -> String {
        format!("{:016x}", self.0)
    }

    /// Formats the 48-bit (12-hex) compact Profile A suffix without the `#icx:` prefix.
    #[inline]
    pub fn to_short_hex12(self) -> String {
        format!("{:012x}", self.0 >> 16)
    }

    /// Formats the 48-bit (12-hex) compact Profile A handle (`#icx:<12-hex>`).
    #[inline]
    pub fn to_short_handle(self) -> String {
        format!("#icx:{:012x}", self.0 >> 16)
    }

    /// Parses either a 16-hex Profile B handle (`#icx:<16-hex>`) or a 12-hex Profile A prefix
    /// (`#icx:<12-hex>`) into an inclusive `[min_id, max_id]` range for $O(\log N)$ `BTreeMap` lookup.
    pub fn parse_prefix_range(s: &str) -> Result<(Self, Self), IcxError> {
        let trimmed_input = s.trim();
        let trimmed = trimmed_input.strip_prefix("#icx:").unwrap_or(trimmed_input);
        if !trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(IcxError::InvalidHandle(s.to_owned()));
        }
        match trimmed.len() {
            16 => {
                let val = u64::from_str_radix(trimmed, 16)
                    .map_err(|_| IcxError::InvalidHandle(s.to_owned()))?;
                Ok((Self(val), Self(val)))
            }
            12 => {
                let prefix48 = u64::from_str_radix(trimmed, 16)
                    .map_err(|_| IcxError::InvalidHandle(s.to_owned()))?;
                let min_val = prefix48 << 16;
                let max_val = min_val | 0xFFFF;
                Ok((Self(min_val), Self(max_val)))
            }
            _ => Err(IcxError::InvalidHandle(s.to_owned())),
        }
    }
}

impl fmt::Display for ArtifactId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#icx:{:016x}", self.0)
    }
}

impl FromStr for ArtifactId {
    type Err = IcxError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed_input = s.trim();
        let trimmed = trimmed_input.strip_prefix("#icx:").unwrap_or(trimmed_input);
        if trimmed.len() != 16 || !trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(IcxError::InvalidHandle(s.to_owned()));
        }
        let val =
            u64::from_str_radix(trimmed, 16).map_err(|_| IcxError::InvalidHandle(s.to_owned()))?;
        Ok(Self(val))
    }
}

/// Full 256-bit cryptographic BLAKE3 digest.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Blake3Digest(pub [u8; 32]);

impl Blake3Digest {
    /// Computes the 256-bit BLAKE3 digest of `bytes`.
    #[inline]
    pub fn hash(bytes: &[u8]) -> Self {
        Self(*blake3::hash(bytes).as_bytes())
    }

    /// Computes a domain-separated 256-bit BLAKE3 digest using `blake3::Hasher::new_derive_key(context)`.
    #[inline]
    pub fn hash_with_domain(domain_context: &str, bytes: &[u8]) -> Self {
        let mut hasher = blake3::Hasher::new_derive_key(domain_context);
        hasher.update(bytes);
        Self(*hasher.finalize().as_bytes())
    }

    /// Fills `out` with arbitrary-length XOF output from a domain-separated BLAKE3 stream.
    #[inline]
    pub fn fill_xof(domain_context: &str, seed: &[u8], out: &mut [u8]) {
        let mut hasher = blake3::Hasher::new_derive_key(domain_context);
        hasher.update(seed);
        let mut reader = hasher.finalize_xof();
        reader.fill(out);
    }

    /// Returns a 64-character lowercase hexadecimal string.
    #[inline]
    pub fn to_hex(&self) -> String {
        blake3::Hash::from_bytes(self.0).to_hex().to_string()
    }
}

/// Agent harness classification for hook, database, and wire interception.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[repr(u8)]
pub enum AgentKind {
    ClaudeCode = 1,
    Codex = 2,
    AntigravityCli = 3,
    Hermes = 4,
    GeminiCli = 5,
    OpenCode = 6,
    Cursor = 7,
    GenericMcp = 255,
}

impl AgentKind {
    /// Parses an agent identifier from CLI arguments or hook metadata.
    pub fn parse_agent(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "claude" | "claude-code" | "claude_code" => Self::ClaudeCode,
            "codex" | "codex-cli" | "openai-codex" => Self::Codex,
            "agy" | "antigravity" | "antigravity-cli" | "antigravity_cli" => Self::AntigravityCli,
            "hermes" | "hermes-agent" => Self::Hermes,
            "gemini" | "gemini-cli" => Self::GeminiCli,
            "opencode" | "open-code" => Self::OpenCode,
            "cursor" => Self::Cursor,
            _ => Self::GenericMcp,
        }
    }

    /// Canonical short name used in AHTL `Θ` telepathy packets.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::Codex => "codex",
            Self::AntigravityCli => "agy",
            Self::Hermes => "hermes",
            Self::GeminiCli => "gemini-cli",
            Self::OpenCode => "open-code",
            Self::Cursor => "cursor",
            Self::GenericMcp => "mcp",
        }
    }
}

impl fmt::Display for AgentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Core error type for `icx-core`.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum IcxError {
    #[error("invalid artifact handle `{0}` (expected #icx:<16-hex> or #icx:<12-hex>)")]
    InvalidHandle(String),

    #[error("ambiguous 48-bit handle prefix `{0}` matches multiple stored artifacts")]
    AmbiguousHandlePrefix(String),

    #[error("artifact `{0}` not found in Content-Addressed Store")]
    ArtifactNotFound(ArtifactId),

    #[error("cryptographic BLAKE3 integrity violation for artifact `{0}`")]
    IntegrityViolation(ArtifactId),

    #[error("invalid line range {start}..={end} (artifact has {total} lines)")]
    InvalidLineRange {
        start: usize,
        end: usize,
        total: usize,
    },

    #[error("payload size {size} bytes exceeds maximum allowed ingestion limit of {max} bytes")]
    PayloadTooLarge { size: usize, max: usize },

    #[error("no active sub-trajectory branch to fold")]
    NoActiveBranch,

    #[error("I/O error in Content-Addressed Store: {0}")]
    Io(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_id_round_trips_display_from_str_and_short_prefix() {
        let (id, digest) = ArtifactId::hash_bytes(b"infinity-context-test");
        assert_eq!(id, ArtifactId::from_digest(&digest));
        let rendered = id.to_string();
        assert!(rendered.starts_with("#icx:"));
        assert_eq!(rendered.len(), 21);
        let parsed: ArtifactId = rendered.parse().expect("valid handle");
        assert_eq!(parsed, id);
        let bare: ArtifactId = id.to_hex().parse().expect("valid bare hex");
        assert_eq!(bare, id);

        let short_handle = id.to_short_handle();
        assert_eq!(short_handle.len(), 17);
        let (min_id, max_id) =
            ArtifactId::parse_prefix_range(&short_handle).expect("valid 12-hex prefix");
        assert!(min_id <= id && id <= max_id);
    }

    #[test]
    fn domain_separated_blake3_and_xof_are_deterministic_and_distinct() {
        let d1 = Blake3Digest::hash_with_domain("ICX_TEST_DOMAIN_A", b"payload");
        let d2 = Blake3Digest::hash_with_domain("ICX_TEST_DOMAIN_B", b"payload");
        assert_ne!(d1, d2);

        let mut xof_a = [0u8; 64];
        let mut xof_b = [0u8; 64];
        Blake3Digest::fill_xof("ICX_XOF_V1", b"seed_1", &mut xof_a);
        Blake3Digest::fill_xof("ICX_XOF_V1", b"seed_1", &mut xof_b);
        assert_eq!(xof_a, xof_b);
    }

    #[test]
    fn rejects_malformed_handles() {
        assert!("invalid".parse::<ArtifactId>().is_err());
        assert!("#icx:1234".parse::<ArtifactId>().is_err());
        assert!("#icx:zzzzzzzzzzzzzzzz".parse::<ArtifactId>().is_err());
    }
}
