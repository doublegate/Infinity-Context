//! `icx-codec` — Alien Holographic Token Lattice (`AHTL`) non-human machine dialect,
//! Braille-256 topological sketcher, domain-specific `StructuralSkeletonizer`, and
//! pre-ingestion `SecretRedactor`.

pub mod ahtl;
pub mod braille;
pub mod redact;
pub mod skeleton;

pub use ahtl::{
    AhtlClause, AhtlPacket, AhtlStream, AhtlWirePacketV1, ClauseOp, CodecError, EpochHeader,
    PacketKind, WirePacket,
};
pub use braille::{
    ChannelBHologram, FLAG_HAS_CAUSAL, FLAG_HAS_DIFF, FLAG_HAS_ERROR, FLAG_HAS_PANIC,
    FLAG_IS_EPOCH, FLAG_IS_FOLDED, FLAG_IS_PINNED, FLAG_IS_REDACTED, braille_to_byte,
    braille_utf8_to_byte, byte_to_braille, byte_to_braille_utf8, decode_braille_64,
    decode_braille_sketch, encode_braille_64, encode_braille_sketch, sketch_hamming_distance,
    spectral_hamming_distance_64,
};
pub use redact::SecretRedactor;
pub use skeleton::StructuralSkeletonizer;

/// Normative trait contract for the Alien Holographic Token Lattice (`AHTL`) Codec
/// (`docs/interfaces.md` §2).
pub trait ContextCodec: Send + Sync {
    /// Sanitizes secrets, skeletonizes the payload, and compiles an `AhtlPacket`.
    fn compress_to_packet(
        &self,
        subject: &str,
        raw_text: &str,
        deref_id: Option<icx_core::ArtifactId>,
        hologram_bytes: [u8; 8],
    ) -> AhtlPacket;

    /// Renders a structured `AhtlPacket` into its canonical wire string.
    fn render_packet(&self, pkt: &AhtlPacket) -> String {
        pkt.render()
    }

    /// Parses a wire string back into a structured `AhtlPacket`.
    fn parse_packet(&self, encoded: &str) -> Result<AhtlPacket, CodecError> {
        AhtlPacket::parse(encoded)
    }
}

impl ContextCodec for StructuralSkeletonizer {
    fn compress_to_packet(
        &self,
        subject: &str,
        raw_text: &str,
        deref_id: Option<icx_core::ArtifactId>,
        hologram_bytes: [u8; 8],
    ) -> AhtlPacket {
        Self::compress_to_packet(subject, raw_text, deref_id, hologram_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_codec_trait_compresses_renders_and_parses() {
        let codec = StructuralSkeletonizer;
        let (id, _) = icx_core::ArtifactId::hash_bytes(b"pub fn run() {}");
        let pkt = codec.compress_to_packet("src/lib.rs", "pub fn run() {}", Some(id), [0x11; 8]);
        let rendered = codec.render_packet(&pkt);
        let parsed = codec.parse_packet(&rendered).expect("valid packet");
        assert_eq!(parsed, pkt);
    }
}
