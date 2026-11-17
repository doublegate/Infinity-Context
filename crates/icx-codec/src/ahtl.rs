//! Canonical Alien Holographic Token Lattice (`AHTL`) packet model, serializer, and parser.

use crate::braille::{decode_braille_sketch, encode_braille_sketch};
use icx_core::ArtifactId;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Top-level semantic category of an `AhtlPacket`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PacketKind {
    /// `Δ`: Code / AST / file read or mutation
    CodeDelta,
    /// `Ψ`: Tool execution output or diagnostic stub
    ToolStub,
    /// `Ω`: Architectural invariant or constraint
    Invariant,
    /// `Θ`: Cross-agent `/dev/shm` telepathy broadcast
    Telepathy,
    /// `Φ`: Folded sub-trajectory branch commit
    FoldedBranch,
}

impl PacketKind {
    /// Single-codepoint AHTL prefix glyph.
    pub const fn glyph(self) -> char {
        match self {
            Self::CodeDelta => 'Δ',
            Self::ToolStub => 'Ψ',
            Self::Invariant => 'Ω',
            Self::Telepathy => 'Θ',
            Self::FoldedBranch => 'Φ',
        }
    }

    /// Parses a `PacketKind` from its single-codepoint glyph.
    pub const fn from_glyph(c: char) -> Option<Self> {
        match c {
            'Δ' => Some(Self::CodeDelta),
            'Ψ' => Some(Self::ToolStub),
            'Ω' => Some(Self::Invariant),
            'Θ' => Some(Self::Telepathy),
            'Φ' => Some(Self::FoldedBranch),
            _ => None,
        }
    }
}

/// Relational operator prefix on an `AhtlClause`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClauseOp {
    /// `⊕`: Added / defined symbol or fact
    Added,
    /// `⊖`: Removed / invalidated fact
    Removed,
    /// `¬`: Hard negative constraint / forbidden
    Negated,
    /// `!`: High-surprise anomaly / error / panic
    Anomaly,
    /// `✓`: Verified / test passed
    Verified,
    /// Plain key/value clause
    Neutral,
}

impl ClauseOp {
    /// Optional single-codepoint prefix glyph.
    pub const fn prefix_str(self) -> &'static str {
        match self {
            Self::Added => "⊕",
            Self::Removed => "⊖",
            Self::Negated => "¬",
            Self::Anomaly => "!",
            Self::Verified => "✓",
            Self::Neutral => "",
        }
    }
}

/// A single relational clause inside an `AhtlPacket`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AhtlClause {
    pub op: ClauseOp,
    pub key: String,
    pub value: Option<String>,
}

impl AhtlClause {
    /// Creates a new `AhtlClause`.
    pub fn new(op: ClauseOp, key: impl Into<String>, value: Option<String>) -> Self {
        Self {
            op,
            key: key.into(),
            value,
        }
    }

    /// Renders this clause into its compact AHTL token representation.
    pub fn render(&self) -> String {
        match &self.value {
            Some(v) => format!("{}{}:{}", self.op.prefix_str(), self.key, v),
            None => format!("{}{}", self.op.prefix_str(), self.key),
        }
    }

    /// Parses a single clause string.
    pub fn parse(raw: &str) -> Self {
        let mut chars = raw.chars();
        let first = chars.next();
        let (op, rest) = match first {
            Some('⊕') => (ClauseOp::Added, chars.as_str()),
            Some('⊖') => (ClauseOp::Removed, chars.as_str()),
            Some('¬') => (ClauseOp::Negated, chars.as_str()),
            Some('!') => (ClauseOp::Anomaly, chars.as_str()),
            Some('✓') => (ClauseOp::Verified, chars.as_str()),
            _ => (ClauseOp::Neutral, raw),
        };
        if let Some((k, v)) = rest.split_once(':') {
            Self {
                op,
                key: k.to_owned(),
                value: Some(v.to_owned()),
            }
        } else {
            Self {
                op,
                key: rest.to_owned(),
                value: None,
            }
        }
    }
}

/// Complete Dual-Channel Alien Holographic Token Lattice (`AHTL`) packet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AhtlPacket {
    pub kind: PacketKind,
    pub subject: String,
    pub line_range: Option<(u32, u32)>,
    pub clauses: Vec<AhtlClause>,
    pub deref_id: Option<ArtifactId>,
    pub hologram_bytes: [u8; 8],
}

/// Error returned when parsing a malformed AHTL packet.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CodecError {
    #[error("missing outer AHTL packet delimiters `⟦...⟧`")]
    MissingDelimiters,
    #[error("unknown packet kind glyph")]
    UnknownPacketKind,
    #[error("malformed subject header")]
    MalformedSubject,
    #[error("malformed dereference handle `{0}`")]
    MalformedHandle(String),
    #[error("malformed Braille-256 sketch")]
    MalformedBrailleSketch,
}

impl AhtlPacket {
    /// Renders this `AhtlPacket` into its canonical single-line non-human BPE-aligned string.
    pub fn render(&self) -> String {
        let mut out = String::with_capacity(128);
        out.push('⟦');
        out.push(self.kind.glyph());
        out.push(':');
        out.push_str(&self.subject);

        if let Some((start, end)) = self.line_range {
            out.push_str(&format!("#L{start}-{end}"));
        }

        if !self.clauses.is_empty() {
            out.push('⊢');
            for (idx, c) in self.clauses.iter().enumerate() {
                if idx > 0 {
                    out.push('⋮');
                }
                out.push_str(&c.render());
            }
        }

        if let Some(id) = self.deref_id {
            out.push('↦');
            out.push_str(&id.to_string());
        }

        out.push_str("⋮Ψ:");
        out.push_str(&encode_braille_sketch(&self.hologram_bytes));
        out.push('⟧');
        out
    }

    /// Parses a rendered AHTL packet string back into a structured `AhtlPacket`.
    pub fn parse(encoded: &str) -> Result<Self, CodecError> {
        let trimmed = encoded.trim();
        let inner = trimmed
            .strip_prefix('⟦')
            .and_then(|s| s.strip_suffix('⟧'))
            .ok_or(CodecError::MissingDelimiters)?;

        // Extract trailing `⋮Ψ:<8-braille>` hologram
        let (before_holo, holo_str) = inner
            .rsplit_once("⋮Ψ:")
            .ok_or(CodecError::MalformedBrailleSketch)?;
        let hologram_bytes =
            decode_braille_sketch(holo_str).ok_or(CodecError::MalformedBrailleSketch)?;

        // Extract optional `↦#icx:<16-hex>` handle
        let (before_deref, deref_id) =
            if let Some((left, handle_str)) = before_holo.rsplit_once('↦') {
                let id: ArtifactId = handle_str
                    .parse()
                    .map_err(|_| CodecError::MalformedHandle(handle_str.to_owned()))?;
                (left, Some(id))
            } else {
                (before_holo, None)
            };

        // Split `<Kind>:<Subject>[#L<a>-<b>]` from optional `⊢<clauses>`
        let (header, clauses_str) = match before_deref.split_once('⊢') {
            Some((h, c)) => (h, Some(c)),
            None => (before_deref, None),
        };

        let mut h_chars = header.chars();
        let kind_glyph = h_chars.next().ok_or(CodecError::UnknownPacketKind)?;
        let kind = PacketKind::from_glyph(kind_glyph).ok_or(CodecError::UnknownPacketKind)?;
        if h_chars.next() != Some(':') {
            return Err(CodecError::MalformedSubject);
        }
        let subj_raw = h_chars.as_str();

        let (subject, line_range) = if let Some((path, range_part)) = subj_raw.rsplit_once("#L") {
            if let Some((s_str, e_str)) = range_part.split_once('-') {
                if let (Ok(s), Ok(e)) = (s_str.parse::<u32>(), e_str.parse::<u32>()) {
                    (path.to_owned(), Some((s, e)))
                } else {
                    (subj_raw.to_owned(), None)
                }
            } else {
                (subj_raw.to_owned(), None)
            }
        } else {
            (subj_raw.to_owned(), None)
        };

        let clauses = match clauses_str {
            Some(cs) if !cs.is_empty() => cs.split('⋮').map(AhtlClause::parse).collect(),
            _ => Vec::new(),
        };

        Ok(Self {
            kind,
            subject,
            line_range,
            clauses,
            deref_id,
            hologram_bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ahtl_packet_renders_and_parses_round_trip() {
        let (id, _) = ArtifactId::hash_bytes(b"raw_payload_bytes");
        let pkt = AhtlPacket {
            kind: PacketKind::CodeDelta,
            subject: "crates/icx-core/src/dag.rs".into(),
            line_range: Some((42, 118)),
            clauses: vec![
                AhtlClause::new(ClauseOp::Added, "fn", Some("seal_epoch".into())),
                AhtlClause::new(ClauseOp::Negated, "unwrap()", None),
                AhtlClause::new(ClauseOp::Anomaly, "PANIC@L99", Some("oob".into())),
                AhtlClause::new(ClauseOp::Verified, "test", Some("14/14".into())),
            ],
            deref_id: Some(id),
            hologram_bytes: [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0],
        };

        let rendered = pkt.render();
        assert!(rendered.starts_with("⟦Δ:crates/icx-core/src/dag.rs#L42-118⊢"));
        let parsed = AhtlPacket::parse(&rendered).expect("valid AHTL packet");
        assert_eq!(parsed, pkt);
    }
}
