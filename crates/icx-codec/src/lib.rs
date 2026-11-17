//! `icx-codec` — Alien Holographic Token Lattice (`AHTL`) non-human machine dialect,
//! Braille-256 topological sketcher, domain-specific `StructuralSkeletonizer`, and
//! pre-ingestion `SecretRedactor`.

pub mod ahtl;
pub mod braille;
pub mod redact;
pub mod skeleton;

pub use ahtl::{AhtlClause, AhtlPacket, ClauseOp, CodecError, PacketKind};
pub use braille::{braille_to_byte, byte_to_braille, decode_braille_sketch, encode_braille_sketch};
pub use redact::SecretRedactor;
pub use skeleton::StructuralSkeletonizer;
