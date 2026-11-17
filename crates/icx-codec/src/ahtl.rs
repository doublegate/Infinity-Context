//! Canonical Alien Holographic Token Lattice (`AHTL`) Dual-Profile packet model, serializer,
//! and parser (`Profile A` `AhtlWirePacketV1` and `Profile B` `AhtlPacket`).
//!
//! References:
//! - `docs/ahtl/01-formal-grammar-and-packet-specification.md`
//! - `docs/ahtl/08-v1-reference-implementation-blueprint.md`
//! - `docs/alien-holographic-codec.md`

use crate::braille::{
    decode_braille_64, decode_braille_sketch, encode_braille_64, encode_braille_sketch,
};
use icx_core::ArtifactId;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Top-level semantic category of an `AhtlPacket` (`Profile B`).
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

/// A single relational clause inside an `AhtlPacket`, optionally carrying a causal implication
/// chain (`⇒`, `U+21D2`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AhtlClause {
    pub op: ClauseOp,
    pub key: String,
    pub value: Option<String>,
    pub implies: Option<Box<AhtlClause>>,
}

impl AhtlClause {
    /// Creates a new `AhtlClause` without a causal implication tail.
    pub fn new(op: ClauseOp, key: impl Into<String>, value: Option<String>) -> Self {
        Self {
            op,
            key: key.into(),
            value,
            implies: None,
        }
    }

    /// Attaches a causal implication clause (`cause⇒effect`).
    pub fn with_implication(mut self, effect: AhtlClause) -> Self {
        self.implies = Some(Box::new(effect));
        self
    }

    /// Renders this clause into its compact AHTL token representation.
    pub fn render(&self) -> String {
        let base = match &self.value {
            Some(v) => format!("{}{}:{}", self.op.prefix_str(), self.key, v),
            None => format!("{}{}", self.op.prefix_str(), self.key),
        };
        match &self.implies {
            Some(effect) => format!("{base}⇒{}", effect.render()),
            None => base,
        }
    }

    /// Parses a single clause string, recursively resolving `⇒` causal implication chains.
    pub fn parse(raw: &str) -> Self {
        let (head_raw, implies) = match raw.split_once('⇒') {
            Some((h, tail)) => (h, Some(Box::new(Self::parse(tail)))),
            None => (raw, None),
        };
        let mut chars = head_raw.chars();
        let first = chars.next();
        let (op, rest) = match first {
            Some('⊕') => (ClauseOp::Added, chars.as_str()),
            Some('⊖') => (ClauseOp::Removed, chars.as_str()),
            Some('¬') => (ClauseOp::Negated, chars.as_str()),
            Some('!') => (ClauseOp::Anomaly, chars.as_str()),
            Some('✓') => (ClauseOp::Verified, chars.as_str()),
            _ => (ClauseOp::Neutral, head_raw),
        };
        if let Some((k, v)) = rest.split_once(':') {
            Self {
                op,
                key: k.to_owned(),
                value: Some(v.to_owned()),
                implies,
            }
        } else {
            Self {
                op,
                key: rest.to_owned(),
                value: None,
                implies,
            }
        }
    }
}

/// Bijectively escapes `%` (`%25`), `#L` (`%23L`), `⟦` (`%E2%9F%A6`), and `⟧` (`%E2%9F%A7`) in a
/// subject string so that literal `#L`, `%23L`, or packet delimiters inside `subject` never
/// collide with outer `⟦...⟧` delimiters or the `#L<start>-<end>` line-range suffix.
fn escape_subject(s: &str) -> String {
    if !s.contains('%') && !s.contains("#L") && !s.contains('⟦') && !s.contains('⟧') {
        return s.to_owned();
    }
    s.replace('%', "%25")
        .replace("#L", "%23L")
        .replace('⟦', "%E2%9F%A6")
        .replace('⟧', "%E2%9F%A7")
}

/// Single-pass left-to-right inverse of `escape_subject`, restoring `%25` -> `%`, `%23L` -> `#L`,
/// `%E2%9F%A6` -> `⟦`, and `%E2%9F%A7` -> `⟧`.
fn unescape_subject(s: &str) -> String {
    if !s.contains('%') {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("%25") {
            out.push('%');
            rest = after;
        } else if let Some(after) = rest.strip_prefix("%23L") {
            out.push_str("#L");
            rest = after;
        } else if let Some(after) = rest.strip_prefix("%E2%9F%A6") {
            out.push('⟦');
            rest = after;
        } else if let Some(after) = rest.strip_prefix("%E2%9F%A7") {
            out.push('⟧');
            rest = after;
        } else {
            let mut chars = rest.chars();
            if let Some(ch) = chars.next() {
                out.push(ch);
                rest = chars.as_str();
            }
        }
    }
    out
}

/// Profile B — Active Interoperable Dual-Channel `AhtlPacket` (`docs/alien-holographic-codec.md` §2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AhtlPacket {
    pub kind: PacketKind,
    pub subject: String,
    pub line_range: Option<(u32, u32)>,
    pub clauses: Vec<AhtlClause>,
    pub deref_id: Option<ArtifactId>,
    pub compact_48bit_handle: bool,
    pub slice_hint: Option<String>,
    pub hologram_bytes: [u8; 8],
    pub hologram_tag: Option<u16>,
}

/// Profile A — Canonical 6-Section `AHTL v1.0` Full-Spectrum Wire Packet
/// (`docs/ahtl/01-formal-grammar-and-packet-specification.md` §1.1):
/// `⟦AHTL v1│#icx:<12|16-hex>((<slice_hint>))?│fc:<64-hex>│hv:<16-hex>│<Braille-64>│<Skeleton>⟧`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AhtlWirePacketV1 {
    pub cas_handle: ArtifactId,
    pub compact_48bit_handle: bool,
    pub slice_hint: Option<String>,
    pub fractal_coord_hex: String,
    pub hdc_digest_hex: String,
    pub braille_spectrum: [u8; 64],
    pub skeleton_body: String,
}

/// Error returned when parsing a malformed AHTL packet or stream.
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
    #[error("malformed Profile A fractal coordinate `fc:<64-hex>`")]
    MalformedFractalCoord,
    #[error("malformed Profile A hypervector digest `hv:<16-hex>`")]
    MalformedHdcDigest,
    #[error("malformed AHTL epoch header `⟪ICX:v1│...⟫`")]
    MalformedEpochHeader,
}

impl AhtlPacket {
    /// Creates a new `AhtlPacket` with default `compact_48bit_handle: false`, `slice_hint: None`,
    /// and `hologram_tag: None`.
    pub fn new(
        kind: PacketKind,
        subject: impl Into<String>,
        line_range: Option<(u32, u32)>,
        clauses: Vec<AhtlClause>,
        deref_id: Option<ArtifactId>,
        hologram_bytes: [u8; 8],
    ) -> Self {
        Self {
            kind,
            subject: subject.into(),
            line_range,
            clauses,
            deref_id,
            compact_48bit_handle: false,
            slice_hint: None,
            hologram_bytes,
            hologram_tag: None,
        }
    }

    /// Formats the dereference handle string (`#icx:<12-hex>` or `#icx:<16-hex>`), preserving
    /// handle width so `CasStore::resolve_handle` performs the proper prefix or exact lookup.
    pub fn deref_handle_string(&self) -> Option<String> {
        self.deref_id.map(|id| {
            if self.compact_48bit_handle {
                id.to_short_handle()
            } else {
                id.to_string()
            }
        })
    }

    /// Renders this `AhtlPacket` into its canonical single-line non-human BPE-aligned string.
    pub fn render(&self) -> String {
        let mut out = String::with_capacity(128);
        out.push('⟦');
        out.push(self.kind.glyph());
        out.push(':');
        out.push_str(&escape_subject(&self.subject));

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

        if let Some(handle_str) = self.deref_handle_string() {
            out.push('↦');
            out.push_str(&handle_str);
            if let Some(hint) = &self.slice_hint {
                if !hint.is_empty() {
                    out.push('(');
                    out.push_str(hint);
                    out.push(')');
                }
            }
        }

        out.push_str("⋮Ψ:");
        out.push_str(&encode_braille_sketch(&self.hologram_bytes));
        if let Some(tag) = self.hologram_tag {
            out.push_str(&format!("[{tag:04x}]"));
        }
        out.push('⟧');
        out
    }

    /// Parses a rendered Profile B AHTL packet string back into a structured `AhtlPacket`.
    pub fn parse(encoded: &str) -> Result<Self, CodecError> {
        let trimmed = encoded.trim();
        let inner = trimmed
            .strip_prefix('⟦')
            .and_then(|s| s.strip_suffix('⟧'))
            .ok_or(CodecError::MissingDelimiters)?;

        // Extract trailing `⋮Ψ:<8-braille>([<hex4>])?` hologram (optional when parsing compact `Φ` or `Ψ` micro-beacons)
        let (before_holo, hologram_bytes, hologram_tag) = if let Some((left, holo_raw)) =
            inner.rsplit_once("⋮Ψ:")
        {
            let (holo_str, tag_opt) = if let Some(tag_part) = holo_raw.strip_suffix(']') {
                let (b_str, hex_tag) = tag_part
                    .rsplit_once('[')
                    .ok_or(CodecError::MalformedBrailleSketch)?;
                if hex_tag.len() != 4 || !hex_tag.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(CodecError::MalformedBrailleSketch);
                }
                let tag = u16::from_str_radix(hex_tag, 16)
                    .map_err(|_| CodecError::MalformedBrailleSketch)?;
                (b_str, Some(tag))
            } else {
                (holo_raw, None)
            };
            let holo = decode_braille_sketch(holo_str).ok_or(CodecError::MalformedBrailleSketch)?;
            (left, holo, tag_opt)
        } else if inner.starts_with("Φ:")
            || (inner.starts_with("Ψ:") && inner.contains('↦') && !inner.contains('⊢'))
        {
            (inner, [0u8; 8], None)
        } else {
            return Err(CodecError::MalformedBrailleSketch);
        };

        // Extract optional `↦#icx:<12|16-hex>((slice_hint))?` handle
        let (before_deref, deref_id, compact_48bit_handle, slice_hint) =
            if let Some((left, deref_tail)) = before_holo.rsplit_once('↦') {
                let (handle_str, hint_opt) = if let Some(with_hint) = deref_tail.strip_suffix(')') {
                    let (h_part, hint_part) = with_hint
                        .split_once('(')
                        .ok_or_else(|| CodecError::MalformedHandle(deref_tail.to_owned()))?;
                    if hint_part.is_empty() {
                        return Err(CodecError::MalformedHandle(deref_tail.to_owned()));
                    }
                    (h_part, Some(hint_part.to_owned()))
                } else {
                    (deref_tail, None)
                };
                let hex_part = handle_str
                    .strip_prefix("#icx:")
                    .ok_or_else(|| CodecError::MalformedHandle(handle_str.to_owned()))?;
                let is_compact = hex_part.len() == 12;
                let (min_id, _) = ArtifactId::parse_prefix_range(handle_str)
                    .map_err(|_| CodecError::MalformedHandle(handle_str.to_owned()))?;
                (left, Some(min_id), is_compact, hint_opt)
            } else {
                (before_holo, None, false, None)
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
        if subj_raw.is_empty() {
            return Err(CodecError::MalformedSubject);
        }

        let (subject, line_range) = if let Some((path, range_part)) = subj_raw.rsplit_once("#L") {
            if let Some((s_str, e_str)) = range_part.split_once('-') {
                if let (Ok(s), Ok(e)) = (s_str.parse::<u32>(), e_str.parse::<u32>()) {
                    (unescape_subject(path), Some((s, e)))
                } else {
                    (unescape_subject(subj_raw), None)
                }
            } else {
                (unescape_subject(subj_raw), None)
            }
        } else {
            (unescape_subject(subj_raw), None)
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
            compact_48bit_handle,
            slice_hint,
            hologram_bytes,
            hologram_tag,
        })
    }

    /// Promotes this Profile B `AhtlPacket` into a Profile A `AhtlWirePacketV1` given the
    /// 256-bit Fractal trajectory hex (`64` hex chars), 64-bit HDC digest hex (`16` hex chars),
    /// and 64-byte (`512-bit`) Braille spectral projection.
    pub fn to_profile_a(
        &self,
        fractal_coord_hex: &str,
        hdc_digest_hex: &str,
        braille_spectrum: [u8; 64],
    ) -> Result<AhtlWirePacketV1, CodecError> {
        let cas_handle = self.deref_id.ok_or_else(|| {
            CodecError::MalformedHandle("cannot promote AhtlPacket without deref_id".into())
        })?;
        let fc_clean = fractal_coord_hex
            .trim()
            .strip_prefix("fc:")
            .unwrap_or(fractal_coord_hex.trim());
        if fc_clean.len() != 64 || !fc_clean.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(CodecError::MalformedFractalCoord);
        }
        let hv_clean = hdc_digest_hex
            .trim()
            .strip_prefix("hv:")
            .unwrap_or(hdc_digest_hex.trim());
        if hv_clean.len() != 16 || !hv_clean.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(CodecError::MalformedHdcDigest);
        }
        let skeleton_body = self
            .clauses
            .iter()
            .map(AhtlClause::render)
            .collect::<Vec<_>>()
            .join("⋮");

        Ok(AhtlWirePacketV1 {
            cas_handle,
            compact_48bit_handle: self.compact_48bit_handle,
            slice_hint: self.slice_hint.clone(),
            fractal_coord_hex: fc_clean.to_ascii_lowercase(),
            hdc_digest_hex: hv_clean.to_ascii_lowercase(),
            braille_spectrum,
            skeleton_body,
        })
    }
}

impl AhtlWirePacketV1 {
    /// Formats the pure CAS handle string (`#icx:<12-hex>` or `#icx:<16-hex>`), preserving
    /// handle width for `CasStore::resolve_handle`.
    pub fn cas_handle_string(&self) -> String {
        if self.compact_48bit_handle {
            self.cas_handle.to_short_handle()
        } else {
            self.cas_handle.to_string()
        }
    }

    /// Renders the canonical 6-section Profile A wire packet:
    /// `⟦AHTL v1│#icx:<12|16-hex>((<slice_hint>))?│fc:<64-hex>│hv:<16-hex>│<Braille-64>│<Skeleton>⟧`
    pub fn render(&self) -> String {
        let mut handle_field = self.cas_handle_string();
        if let Some(hint) = &self.slice_hint {
            if !hint.is_empty() {
                handle_field.push('(');
                handle_field.push_str(hint);
                handle_field.push(')');
            }
        }
        format!(
            "⟦AHTL v1│{}│fc:{}│hv:{}│{}│{}⟧",
            handle_field,
            self.fractal_coord_hex,
            self.hdc_digest_hex,
            encode_braille_64(&self.braille_spectrum),
            self.skeleton_body
        )
    }

    /// Parses a canonical 6-section Profile A wire packet (`⟦AHTL v1│...⟧`).
    pub fn parse(input: &str) -> Result<Self, CodecError> {
        let inner = input
            .trim()
            .strip_prefix('⟦')
            .and_then(|s| s.strip_suffix('⟧'))
            .ok_or(CodecError::MissingDelimiters)?;

        let mut parts = inner.splitn(6, '│');
        let magic = parts.next().ok_or(CodecError::UnknownPacketKind)?;
        if magic != "AHTL v1" {
            return Err(CodecError::UnknownPacketKind);
        }

        let handle_field = parts
            .next()
            .ok_or_else(|| CodecError::MalformedHandle(String::new()))?;
        let (handle_raw, slice_hint) = if let Some(with_hint) = handle_field.strip_suffix(')') {
            let (h_part, hint_part) = with_hint
                .split_once('(')
                .ok_or_else(|| CodecError::MalformedHandle(handle_field.to_owned()))?;
            if hint_part.is_empty() {
                return Err(CodecError::MalformedHandle(handle_field.to_owned()));
            }
            (h_part, Some(hint_part.to_owned()))
        } else {
            (handle_field, None)
        };
        let hex_part = handle_raw
            .strip_prefix("#icx:")
            .ok_or_else(|| CodecError::MalformedHandle(handle_raw.to_owned()))?;
        let compact_48bit_handle = hex_part.len() == 12;
        let (cas_handle, _) = ArtifactId::parse_prefix_range(handle_raw)
            .map_err(|_| CodecError::MalformedHandle(handle_raw.to_owned()))?;

        let fc_raw = parts.next().ok_or(CodecError::MalformedFractalCoord)?;
        let fc_hex = fc_raw
            .strip_prefix("fc:")
            .ok_or(CodecError::MalformedFractalCoord)?;
        if fc_hex.len() != 64 || !fc_hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(CodecError::MalformedFractalCoord);
        }

        let hv_raw = parts.next().ok_or(CodecError::MalformedHdcDigest)?;
        let hv_hex = hv_raw
            .strip_prefix("hv:")
            .ok_or(CodecError::MalformedHdcDigest)?;
        if hv_hex.len() != 16 || !hv_hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(CodecError::MalformedHdcDigest);
        }

        let braille_raw = parts.next().ok_or(CodecError::MalformedBrailleSketch)?;
        let braille_spectrum =
            decode_braille_64(braille_raw).ok_or(CodecError::MalformedBrailleSketch)?;

        let skeleton_body = parts.next().ok_or(CodecError::MalformedSubject)?.to_owned();

        Ok(Self {
            cas_handle,
            compact_48bit_handle,
            slice_hint,
            fractal_coord_hex: fc_hex.to_ascii_lowercase(),
            hdc_digest_hex: hv_hex.to_ascii_lowercase(),
            braille_spectrum,
            skeleton_body,
        })
    }

    /// Down-projects this Profile A packet into a Profile B `AhtlPacket` (`docs/ahtl/08-v1-reference-implementation-blueprint.md` §3).
    pub fn to_profile_b(
        &self,
        kind: PacketKind,
        subject: impl Into<String>,
        line_range: Option<(u32, u32)>,
        surprise_q8: u8,
        structural_flags: u8,
    ) -> AhtlPacket {
        let hv_u64 = u64::from_str_radix(&self.hdc_digest_hex, 16).unwrap_or(0);
        let hdc_sketch_u32 = ((hv_u64 >> 32) as u32) ^ (hv_u64 as u32);
        let s_bytes = hdc_sketch_u32.to_be_bytes();

        // Derive 16-bit orthant mask from the 16 Q15 big-endian 16-bit words in `fractal_coord_hex`
        let mut quad_mask = 0u16;
        for j in 0..16 {
            if let Some(word_hex) = self.fractal_coord_hex.get(j * 4..(j + 1) * 4) {
                if let Ok(u) = u16::from_str_radix(word_hex, 16) {
                    if u.cast_signed() >= 0 {
                        quad_mask |= 1u16 << j;
                    }
                }
            }
        }
        let q_bytes = quad_mask.to_be_bytes();
        let hologram_bytes = [
            s_bytes[0],
            s_bytes[1],
            s_bytes[2],
            s_bytes[3],
            q_bytes[0],
            q_bytes[1],
            surprise_q8,
            structural_flags,
        ];

        let clauses = if self.skeleton_body.is_empty() {
            Vec::new()
        } else {
            self.skeleton_body
                .split('⋮')
                .map(AhtlClause::parse)
                .collect()
        };

        AhtlPacket {
            kind,
            subject: subject.into(),
            line_range,
            clauses,
            deref_id: Some(self.cas_handle),
            compact_48bit_handle: self.compact_48bit_handle,
            slice_hint: self.slice_hint.clone(),
            hologram_bytes,
            hologram_tag: None,
        }
    }
}

/// Discriminant for the third field of an `EpochHeader` (`τ=<u32>`, `nodes=<u32>`, or `folded=<u32>`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EpochCountKind {
    TurnStart(u32),
    Nodes(u32),
    Folded(u32),
}

/// Top-level `⟪ICX:v1│ep=<hex4>│root=<hex8|16>│(τ|nodes|folded)=<u32>⟫` stream epoch header
/// (`docs/ahtl/01-formal-grammar-and-packet-specification.md` §1.2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpochHeader {
    pub epoch_id: u16,
    pub merkle_root_hex: String,
    pub count_kind: EpochCountKind,
}

impl EpochHeader {
    /// Renders the canonical epoch header `⟪ICX:v1│ep=<hex4>│root=<hex>│(τ|nodes|folded)=<u32>⟫`.
    pub fn render(&self) -> String {
        let count_str = match self.count_kind {
            EpochCountKind::TurnStart(v) => format!("τ={v}"),
            EpochCountKind::Nodes(v) => format!("nodes={v}"),
            EpochCountKind::Folded(v) => format!("folded={v}"),
        };
        format!(
            "⟪ICX:v1│ep={:04x}│root={}│{}⟫",
            self.epoch_id, self.merkle_root_hex, count_str
        )
    }

    /// Parses an epoch header string `⟪ICX:v1│ep=<hex4>│root=<hex8|16>│(τ|nodes|folded)=<u32>⟫`.
    pub fn parse(input: &str) -> Result<Self, CodecError> {
        let inner = input
            .trim()
            .strip_prefix("⟪ICX:v1│")
            .and_then(|s| s.strip_suffix('⟫'))
            .ok_or(CodecError::MalformedEpochHeader)?;
        let mut parts = inner.split('│');
        let ep_part = parts
            .next()
            .and_then(|s| s.strip_prefix("ep="))
            .ok_or(CodecError::MalformedEpochHeader)?;
        if ep_part.len() != 4 || !ep_part.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(CodecError::MalformedEpochHeader);
        }
        let epoch_id =
            u16::from_str_radix(ep_part, 16).map_err(|_| CodecError::MalformedEpochHeader)?;

        let root_part = parts
            .next()
            .and_then(|s| s.strip_prefix("root="))
            .ok_or(CodecError::MalformedEpochHeader)?;
        if (root_part.len() != 8 && root_part.len() != 16)
            || !root_part.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(CodecError::MalformedEpochHeader);
        }

        let count_raw = parts.next().ok_or(CodecError::MalformedEpochHeader)?;
        if parts.next().is_some() {
            return Err(CodecError::MalformedEpochHeader);
        }
        let count_kind = if let Some(v) = count_raw.strip_prefix("τ=") {
            EpochCountKind::TurnStart(
                v.parse::<u32>()
                    .map_err(|_| CodecError::MalformedEpochHeader)?,
            )
        } else if let Some(v) = count_raw.strip_prefix("nodes=") {
            EpochCountKind::Nodes(
                v.parse::<u32>()
                    .map_err(|_| CodecError::MalformedEpochHeader)?,
            )
        } else if let Some(v) = count_raw.strip_prefix("folded=") {
            EpochCountKind::Folded(
                v.parse::<u32>()
                    .map_err(|_| CodecError::MalformedEpochHeader)?,
            )
        } else {
            return Err(CodecError::MalformedEpochHeader);
        };

        Ok(Self {
            epoch_id,
            merkle_root_hex: root_part.to_ascii_lowercase(),
            count_kind,
        })
    }
}

/// Polymorphic wire packet supporting both `Profile A` (`AhtlWirePacketV1`) and `Profile B` (`AhtlPacket`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WirePacket {
    ProfileA(AhtlWirePacketV1),
    ProfileB(AhtlPacket),
}

impl WirePacket {
    /// Automatically detects and parses either a `Profile A` (`⟦AHTL v1│...⟧`) or `Profile B` (`⟦Δ:...⟧`) packet.
    pub fn parse_auto(input: &str) -> Result<Self, CodecError> {
        let trimmed = input.trim();
        if trimmed.starts_with("⟦AHTL v1│") {
            Ok(Self::ProfileA(AhtlWirePacketV1::parse(trimmed)?))
        } else {
            Ok(Self::ProfileB(AhtlPacket::parse(trimmed)?))
        }
    }

    /// Renders the underlying wire packet.
    pub fn render(&self) -> String {
        match self {
            Self::ProfileA(p) => p.render(),
            Self::ProfileB(p) => p.render(),
        }
    }
}

/// Multi-packet `AhtlStream` (`EpochHeader? Packet+` per `docs/alien-holographic-codec.md` §2.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AhtlStream {
    pub header: Option<EpochHeader>,
    pub packets: Vec<WirePacket>,
}

impl AhtlStream {
    /// Parses a stream containing an optional `⟪ICX:v1│...⟫` header followed by one or more
    /// `⟦...⟧` packets (`Profile A` or `Profile B`), even across multiple lines.
    pub fn parse_stream(input: &str) -> Result<Self, CodecError> {
        let mut rest = input.trim();
        let header = if rest.starts_with("⟪ICX:v1│") {
            let end_idx = rest.find('⟫').ok_or(CodecError::MalformedEpochHeader)?;
            let hdr_slice = &rest[..(end_idx + '⟫'.len_utf8())];
            let hdr = EpochHeader::parse(hdr_slice)?;
            rest = rest[hdr_slice.len()..].trim_start();
            Some(hdr)
        } else {
            None
        };

        let mut packets = Vec::new();
        while !rest.is_empty() {
            let Some(start_rel) = rest.find('⟦') else {
                return Err(CodecError::MissingDelimiters);
            };
            if !rest[..start_rel].trim().is_empty() {
                return Err(CodecError::MissingDelimiters);
            }
            let after_start = &rest[start_rel..];
            let end_rel = after_start.find('⟧').ok_or(CodecError::MissingDelimiters)?;
            let pkt_end = end_rel + '⟧'.len_utf8();
            let pkt_slice = &after_start[..pkt_end];
            packets.push(WirePacket::parse_auto(pkt_slice)?);
            rest = after_start[pkt_end..].trim_start();
        }

        if packets.is_empty() {
            return Err(CodecError::MissingDelimiters);
        }

        Ok(Self { header, packets })
    }

    /// Renders this `AhtlStream` into its wire representation.
    pub fn render(&self) -> String {
        let mut lines = Vec::with_capacity(self.packets.len() + 1);
        if let Some(h) = &self.header {
            lines.push(h.render());
        }
        for p in &self.packets {
            lines.push(p.render());
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ahtl_packet_renders_and_parses_round_trip_with_all_features() {
        let (id, _) = ArtifactId::hash_bytes(b"raw_payload_bytes");
        let mut pkt = AhtlPacket::new(
            PacketKind::CodeDelta,
            "crates/icx-core/src/dag#L1%23L%25.rs",
            Some((42, 118)),
            vec![
                AhtlClause::new(ClauseOp::Added, "fn", Some("seal_epoch".into())),
                AhtlClause::new(ClauseOp::Negated, "unwrap()", None),
                AhtlClause::new(ClauseOp::Anomaly, "PANIC@L99", Some("oob".into()))
                    .with_implication(AhtlClause::new(
                        ClauseOp::Added,
                        "fix",
                        Some("bounds_check".into()),
                    )),
                AhtlClause::new(ClauseOp::Verified, "test", Some("14/14".into())),
            ],
            Some(id),
            [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0],
        );
        pkt.slice_hint = Some("L42-118".into());
        pkt.hologram_tag = Some(0xbeef);

        let rendered = pkt.render();
        assert!(rendered.starts_with("⟦Δ:crates/icx-core/src/dag%23L1%2523L%2525.rs#L42-118⊢"));
        assert!(rendered.contains("⇒⊕fix:bounds_check"));
        assert!(rendered.contains("(L42-118)"));
        assert!(rendered.ends_with("[beef]⟧"));

        let parsed = AhtlPacket::parse(&rendered).expect("valid AHTL packet");
        assert_eq!(parsed, pkt);

        // Verify empty slice_hint is skipped during render
        let mut empty_hint_pkt = pkt.clone();
        empty_hint_pkt.slice_hint = Some(String::new());
        let rendered_no_hint = empty_hint_pkt.render();
        assert!(!rendered_no_hint.contains(&format!("{id}()")));
        let parsed_no_hint = AhtlPacket::parse(&rendered_no_hint).unwrap();
        assert_eq!(parsed_no_hint.slice_hint, None);

        // Verify line_range == None with subject ending in `#L1-5`, containing `%23L`, or containing `⟦`/`⟧`
        let subj_with_hash_l = AhtlPacket::new(
            PacketKind::Invariant,
            "rule%23L_and_⟦inner⟧_#L1-5",
            None,
            vec![
                AhtlClause::new(ClauseOp::Removed, "legacy", Some("v0".into())),
                AhtlClause::new(ClauseOp::Neutral, "flag", None),
            ],
            None,
            [0; 8],
        );
        let r2 = subj_with_hash_l.render();
        assert_eq!(AhtlPacket::parse(&r2).unwrap(), subj_with_hash_l);

        // Verify compact micro-beacon (`⟦Ψ:role↦#icx:...⟧`) and folded branch (`⟦Φ:...⟧`) without `⋮Ψ:`
        let beacon_str = "⟦Ψ:tool_read↦#icx:0123456789abcdef⟧";
        let beacon = AhtlPacket::parse(beacon_str).expect("parse compact micro-beacon");
        assert_eq!(beacon.kind, PacketKind::ToolStub);

        let folded_str = "⟦Φ:debug_loop⊢steps:3⋮✓resolved↦#icx:0123456789abcdef⟧";
        let folded = AhtlPacket::parse(folded_str).expect("parse folded branch");
        assert_eq!(folded.kind, PacketKind::FoldedBranch);

        for kind in [
            PacketKind::CodeDelta,
            PacketKind::ToolStub,
            PacketKind::Invariant,
            PacketKind::Telepathy,
            PacketKind::FoldedBranch,
        ] {
            assert_eq!(PacketKind::from_glyph(kind.glyph()), Some(kind));
        }
        assert_eq!(PacketKind::from_glyph('?'), None);

        assert_eq!(
            AhtlPacket::parse("no_delims"),
            Err(CodecError::MissingDelimiters)
        );
        assert_eq!(
            AhtlPacket::parse("⟦Δ:subj⟧"),
            Err(CodecError::MalformedBrailleSketch)
        );
        assert_eq!(
            AhtlPacket::parse("⟦Δ:subj⋮Ψ:bad⟧"),
            Err(CodecError::MalformedBrailleSketch)
        );
        assert_eq!(
            AhtlPacket::parse("⟦Δ:subj⋮Ψ:⠀⠀⠀⠀⠀⠀⠀⠀[zz]⟧"),
            Err(CodecError::MalformedBrailleSketch)
        );
        assert_eq!(
            AhtlPacket::parse("⟦?:subj⋮Ψ:⠀⠀⠀⠀⠀⠀⠀⠀⟧"),
            Err(CodecError::UnknownPacketKind)
        );
        assert_eq!(
            AhtlPacket::parse("⟦Δsubj⋮Ψ:⠀⠀⠀⠀⠀⠀⠀⠀⟧"),
            Err(CodecError::MalformedSubject)
        );
        assert_eq!(
            AhtlPacket::parse("⟦Δ:⋮Ψ:⠀⠀⠀⠀⠀⠀⠀⠀⟧"),
            Err(CodecError::MalformedSubject)
        );
        assert!(matches!(
            AhtlPacket::parse("⟦Δ:subj↦#icx:bad⋮Ψ:⠀⠀⠀⠀⠀⠀⠀⠀⟧"),
            Err(CodecError::MalformedHandle(_))
        ));
        assert!(matches!(
            AhtlPacket::parse("⟦Δ:subj↦#icx:0123456789abcdef()⋮Ψ:⠀⠀⠀⠀⠀⠀⠀⠀⟧"),
            Err(CodecError::MalformedHandle(_))
        ));
    }

    #[test]
    fn profile_a_wire_packet_v1_compact_handles_and_stream_round_trip() {
        let (id, _) = ArtifactId::hash_bytes(b"profile_a_payload");
        let mut pkt_b = AhtlPacket::new(
            PacketKind::CodeDelta,
            "src/alloc.rs",
            Some((1, 90)),
            vec![
                AhtlClause::new(ClauseOp::Added, "fn", Some("alloc_page".into())),
                AhtlClause::new(ClauseOp::Anomaly, "ERR@L44", Some("align_offset".into())),
            ],
            Some(id),
            [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88],
        );
        pkt_b.slice_hint = Some("L1-90".into());

        let fc_hex = "7fff80017fff80017fff80017fff80017fff80017fff80017fff80017fff8001";
        let hv_hex = "0123456789abcdef";
        let spectrum = [0xA5u8; 64];

        let mut pkt_a = pkt_b
            .to_profile_a(fc_hex, hv_hex, spectrum)
            .expect("promote to Profile A");
        let wire_a = pkt_a.render();
        assert!(wire_a.starts_with("⟦AHTL v1│#icx:"));
        assert!(wire_a.contains("(L1-90)"));
        let parsed_a = AhtlWirePacketV1::parse(&wire_a).expect("parse Profile A");
        assert_eq!(parsed_a, pkt_a);
        assert_eq!(parsed_a.cas_handle_string(), id.to_string());

        // Test 48-bit compact handle preservation across Profile A <-> Profile B conversions
        pkt_a.compact_48bit_handle = true;
        pkt_a.slice_hint = None;
        let wire_a_short = pkt_a.render();
        let parsed_a_short =
            AhtlWirePacketV1::parse(&wire_a_short).expect("parse 48-bit Profile A");
        assert!(parsed_a_short.compact_48bit_handle);
        assert_eq!(parsed_a_short.cas_handle_string(), id.to_short_handle());

        let down_b_short = parsed_a_short.to_profile_b(
            PacketKind::CodeDelta,
            "src/alloc.rs",
            Some((1, 90)),
            200,
            0x03,
        );
        assert!(down_b_short.compact_48bit_handle);
        assert_eq!(
            down_b_short.deref_handle_string().as_deref(),
            Some(id.to_short_handle().as_str())
        );
        let wire_b_short = down_b_short.render();
        assert!(wire_b_short.contains(&format!("↦{}⋮Ψ:", id.to_short_handle())));
        let reparsed_b_short = AhtlPacket::parse(&wire_b_short).expect("parse 48-bit Profile B");
        assert!(reparsed_b_short.compact_48bit_handle);
        let re_promoted_a = reparsed_b_short
            .to_profile_a(fc_hex, hv_hex, spectrum)
            .unwrap();
        assert!(re_promoted_a.compact_48bit_handle);

        // Multi-packet AhtlStream with EpochHeader (`τ=`, `nodes=`, `folded=`) + Profile A + Profile B
        for count_kind in [
            EpochCountKind::TurnStart(42),
            EpochCountKind::Nodes(8),
            EpochCountKind::Folded(2),
        ] {
            let stream = AhtlStream {
                header: Some(EpochHeader {
                    epoch_id: 0x0007,
                    merkle_root_hex: "8f01a2c4".into(),
                    count_kind,
                }),
                packets: vec![
                    WirePacket::ProfileA(parsed_a.clone()),
                    WirePacket::ProfileB(pkt_b.clone()),
                ],
            };
            let rendered_stream = stream.render();
            let parsed_stream = AhtlStream::parse_stream(&rendered_stream).expect("valid stream");
            assert_eq!(parsed_stream, stream);
        }

        // Error branches for Profile A, EpochHeader, and AhtlStream
        let mut pkt_no_deref = pkt_b.clone();
        pkt_no_deref.deref_id = None;
        assert!(matches!(
            pkt_no_deref.to_profile_a(fc_hex, hv_hex, spectrum),
            Err(CodecError::MalformedHandle(_))
        ));
        assert_eq!(
            pkt_b.to_profile_a("bad_fc", hv_hex, spectrum),
            Err(CodecError::MalformedFractalCoord)
        );
        assert_eq!(
            pkt_b.to_profile_a(fc_hex, "bad_hv", spectrum),
            Err(CodecError::MalformedHdcDigest)
        );
        assert_eq!(
            AhtlWirePacketV1::parse("⟦WRONG│#icx:0123456789abcdef⟧"),
            Err(CodecError::UnknownPacketKind)
        );
        assert!(matches!(
            AhtlWirePacketV1::parse(&format!(
                "⟦AHTL v1│#icx:0123456789abcdef()│fc:{fc_hex}│hv:{hv_hex}│x│s⟧"
            )),
            Err(CodecError::MalformedHandle(_))
        ));
        assert_eq!(
            AhtlWirePacketV1::parse(
                "⟦AHTL v1│#icx:0123456789abcdef│bad_fc│hv:0123456789abcdef│x│s⟧"
            ),
            Err(CodecError::MalformedFractalCoord)
        );
        assert_eq!(
            AhtlWirePacketV1::parse(&format!(
                "⟦AHTL v1│#icx:0123456789abcdef│fc:{fc_hex}│bad_hv│x│s⟧"
            )),
            Err(CodecError::MalformedHdcDigest)
        );
        assert_eq!(
            AhtlWirePacketV1::parse(&format!(
                "⟦AHTL v1│#icx:0123456789abcdef│fc:{fc_hex}│hv:{hv_hex}│bad_braille│s⟧"
            )),
            Err(CodecError::MalformedBrailleSketch)
        );
        assert_eq!(
            EpochHeader::parse("⟪ICX:v1│ep=zzzz│root=8f01a2c4│τ=1⟫"),
            Err(CodecError::MalformedEpochHeader)
        );
        assert_eq!(
            EpochHeader::parse("⟪ICX:v1│ep=0001│root=bad│τ=1⟫"),
            Err(CodecError::MalformedEpochHeader)
        );
        assert_eq!(
            EpochHeader::parse("⟪ICX:v1│ep=0001│root=8f01a2c4│unknown=1⟫"),
            Err(CodecError::MalformedEpochHeader)
        );
        assert_eq!(
            EpochHeader::parse("⟪ICX:v1│ep=0001│root=8f01a2c4│τ=1│extra⟫"),
            Err(CodecError::MalformedEpochHeader)
        );
        assert_eq!(
            AhtlStream::parse_stream("⟪ICX:v1│ep=0001│root=8f01a2c4│τ=1⟫"),
            Err(CodecError::MissingDelimiters)
        );
        assert_eq!(
            AhtlStream::parse_stream("garbage⟦Ψ:tool↦#icx:0123456789abcdef⟧"),
            Err(CodecError::MissingDelimiters)
        );
    }
}
