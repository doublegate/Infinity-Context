//! `icx-codec` — Alien Holographic Token Lattice (`AHTL`) non-human machine dialect,
//! Braille-256 topological sketcher, domain-specific `StructuralSkeletonizer`, and
//! pre-ingestion `SecretRedactor`.

pub mod ahtl;
pub mod braille;
pub mod redact;
pub mod skeleton;

pub use ahtl::{
    AhtlClause, AhtlPacket, AhtlStream, AhtlWirePacketV1, ClauseOp, CodecError, EpochCountKind,
    EpochHeader, PacketKind, WirePacket,
};
pub use braille::{
    ChannelBHologram, FLAG_HAS_DIFF, FLAG_HAS_ERROR, FLAG_HAS_PANIC, FLAG_IS_EPOCH, FLAG_IS_FOLDED,
    FLAG_IS_REDACTED, TIER_COLD_BEACON, TIER_FOLD_STUB, TIER_MASK, TIER_PIN_HIGH_RES,
    braille_to_byte, braille_utf8_to_byte, byte_to_braille, byte_to_braille_utf8,
    decode_braille_64, decode_braille_sketch, encode_braille_64, encode_braille_sketch,
    sketch_hamming_distance, spectral_hamming_distance_64,
};
pub use redact::SecretRedactor;
pub use skeleton::StructuralSkeletonizer;

/// Normative trait contract for the Alien Holographic Token Lattice (`AHTL`) Codec
/// (`docs/interfaces.md` §2).
pub trait ContextCodec: Send + Sync {
    /// Redacts secrets, skeletonizes `raw_text`, and compiles an AHTL Holographic Packet.
    fn compress_to_packet(
        &self,
        kind: PacketKind,
        subject: &str,
        raw_text: &str,
        deref_id: Option<icx_core::ArtifactId>,
        hologram: [u8; 8],
    ) -> AhtlPacket;

    /// Renders an `AhtlPacket` into its canonical non-human BPE-aligned string representation.
    fn render_packet(&self, packet: &AhtlPacket) -> String {
        packet.render()
    }

    /// Parses a rendered AHTL packet back into its structured representation.
    fn parse_packet(&self, encoded: &str) -> Result<AhtlPacket, CodecError> {
        AhtlPacket::parse(encoded)
    }
}

impl ContextCodec for StructuralSkeletonizer {
    fn compress_to_packet(
        &self,
        kind: PacketKind,
        subject: &str,
        raw_text: &str,
        deref_id: Option<icx_core::ArtifactId>,
        hologram: [u8; 8],
    ) -> AhtlPacket {
        let mut pkt = Self::compress_to_packet(subject, raw_text, deref_id, hologram);
        pkt.kind = kind;
        pkt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_codec_trait_compresses_renders_and_parses() {
        let codec = StructuralSkeletonizer;
        let (id, _) = icx_core::ArtifactId::hash_bytes(b"pub fn run() {}");
        let pkt = codec.compress_to_packet(
            PacketKind::CodeDelta,
            "src/lib.rs",
            "pub fn run() {}",
            Some(id),
            [0x11; 8],
        );
        let rendered = codec.render_packet(&pkt);
        let parsed = codec.parse_packet(&rendered).expect("valid packet");
        assert_eq!(parsed, pkt);
    }
}
