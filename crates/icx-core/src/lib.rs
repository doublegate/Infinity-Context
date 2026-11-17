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

/// Short 64-bit content-addressed handle derived from the first 8 big-endian bytes of a BLAKE3 hash.
/// Rendered externally as `#icx:<16-lowercase-hex>`.
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
}

impl fmt::Display for ArtifactId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#icx:{:016x}", self.0)
    }
}

impl FromStr for ArtifactId {
    type Err = IcxError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.strip_prefix("#icx:").unwrap_or(s);
        if trimmed.len() != 16 {
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
    #[error("invalid artifact handle `{0}` (expected #icx:<16-hex>)")]
    InvalidHandle(String),

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

    #[error("no active sub-trajectory branch to fold")]
    NoActiveBranch,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_id_round_trips_display_and_from_str() {
        let (id, digest) = ArtifactId::hash_bytes(b"infinity-context-test");
        assert_eq!(id, ArtifactId::from_digest(&digest));
        let rendered = id.to_string();
        assert!(rendered.starts_with("#icx:"));
        assert_eq!(rendered.len(), 21);
        let parsed: ArtifactId = rendered.parse().expect("valid handle");
        assert_eq!(parsed, id);
        let bare: ArtifactId = id.to_hex().parse().expect("valid bare hex");
        assert_eq!(bare, id);
    }

    #[test]
    fn rejects_malformed_handles() {
        assert!("invalid".parse::<ArtifactId>().is_err());
        assert!("#icx:1234".parse::<ArtifactId>().is_err());
        assert!("#icx:zzzzzzzzzzzzzzzz".parse::<ArtifactId>().is_err());
    }
}
