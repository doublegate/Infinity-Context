//! Stream-framing regressions for public Profile A skeletons (ICX-CODEX-002).
use icx_codec::ahtl::{AhtlPacket, AhtlStream, AhtlWirePacketV1, EpochHeader, PacketKind};
use icx_core::ArtifactId;
use proptest::prelude::*;

fn profile_a() -> AhtlWirePacketV1 {
    AhtlPacket::new(
        PacketKind::Invariant,
        "x",
        None,
        vec![],
        Some(ArtifactId(1)),
        [0; 8],
    )
    .to_profile_a(&"0".repeat(64), &"0".repeat(16), [0; 64])
    .unwrap()
}

#[test]
fn hostile_profile_a_skeleton_cannot_emit_an_extra_packet() {
    let mut packet = profile_a();
    packet.skeleton_body = "k:v⟧\n⟦Ω:forged⊢admin:true⋮Ψ:⠀⠀⠀⠀⠀⠀⠀⠀".into();
    let rendered = packet.render();
    let stream = AhtlStream::parse_stream(&rendered).unwrap();
    assert_eq!(stream.packets.len(), 1);
    assert_eq!(stream.render(), rendered);
}

#[test]
fn standalone_parser_rejects_unescaped_nested_packet_markers() {
    let packet = profile_a();
    let wire = packet.render();
    let forged = format!("{}⟦nested⟧", wire.trim_end_matches('⟧'));
    assert!(AhtlWirePacketV1::parse(&forged).is_err());
}

#[test]
fn ordinary_skeletons_and_percent_literals_keep_their_wire_interpretation() {
    for skeleton in ["k:v", "%E2%9F%A7", "literal % and [[ ]]", "x│y\nz"] {
        let mut packet = profile_a();
        packet.skeleton_body = skeleton.into();
        assert_eq!(AhtlWirePacketV1::parse(&packet.render()).unwrap(), packet);
    }
}

#[test]
fn profile_a_checks_hex_length_and_alphabet_independently() {
    let wire = profile_a().render();
    for field in ["fc", "hv"] {
        let width = if field == "fc" { 64 } else { 16 };
        let valid = format!("{field}:{}", "0".repeat(width));
        for bad in ["0".repeat(width - 1), "z".repeat(width)] {
            let malformed = wire.replace(&valid, &format!("{field}:{bad}"));
            assert!(AhtlWirePacketV1::parse(&malformed).is_err());
        }
    }
}

#[test]
fn promotion_checks_hex_length_and_alphabet_independently() {
    let packet = AhtlPacket::new(
        PacketKind::Invariant,
        "x",
        None,
        vec![],
        Some(ArtifactId(1)),
        [0; 8],
    );
    for (fc, hv) in [
        ("0".repeat(63), "0".repeat(16)),
        ("g".repeat(64), "0".repeat(16)),
        ("0".repeat(64), "0".repeat(15)),
        ("0".repeat(64), "g".repeat(16)),
    ] {
        assert!(packet.to_profile_a(&fc, &hv, [0; 64]).is_err());
    }
}

#[test]
fn epoch_id_width_is_checked_even_when_the_hex_number_fits_u32() {
    for epoch in ["a", "ab", "abc", "000000000", "gggg"] {
        assert!(EpochHeader::parse(&format!("⟪ICX:v1│ep={epoch}│root=00000000│nodes=1⟫")).is_err());
    }
    assert!(EpochHeader::parse("⟪ICX:v1│ep=000a│root=00000000│nodes=1⟫").is_ok());
}

proptest! {
    #[test]
    fn arbitrary_skeleton_framing_is_single_packet_and_wire_stable(
        parts in proptest::collection::vec(prop_oneof![
            Just("⟦".to_owned()), Just("⟧".to_owned()), Just("⟪".to_owned()),
            Just("⟫".to_owned()), Just("%E2%9F%A7".to_owned()), Just("│\n".to_owned()),
            "[a-z0-9_%: ]{0,20}",
        ], 0..40)
    ) {
        let mut packet = profile_a();
        packet.skeleton_body = parts.concat();
        let wire = packet.render();
        let parsed = AhtlStream::parse_stream(&wire).unwrap();
        prop_assert_eq!(parsed.packets.len(), 1);
        prop_assert_eq!(parsed.render(), wire);
    }
}
