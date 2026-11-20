//! Property-based invariants for the codec boundary (ICX-AUD-039).

use icx_codec::SecretRedactor;
use icx_codec::ahtl::{AhtlClause, AhtlPacket, AhtlStream, ClauseOp, PacketKind};
use icx_core::ArtifactId;
use proptest::prelude::*;

/// Text with a high density of AHTL structural characters and line endings.
fn hostile_text() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![
            Just("⟦".to_owned()),
            Just("⟧".to_owned()),
            Just("⟪".to_owned()),
            Just("⟫".to_owned()),
            Just("⊢".to_owned()),
            Just("⋮".to_owned()),
            Just("↦".to_owned()),
            Just("⇒".to_owned()),
            Just(":".to_owned()),
            Just("(".to_owned()),
            Just(")".to_owned()),
            Just("%".to_owned()),
            Just("\n".to_owned()),
            Just("\r\n".to_owned()),
            "[a-z0-9 _./-]{1,8}",
        ],
        0..40,
    )
    .prop_map(|v| v.concat())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    /// Secret-free text is stored byte-exact (ICX-AUD-021).
    #[test]
    fn redact_is_identity_on_secret_free_text(s in "[a-zA-Z0-9 ,.;()\\[\\]{}<>\\n\\r\\t⟦⟧⟪⟫-]{0,200}") {
        prop_assert_eq!(SecretRedactor::redact(&s), s);
    }

    /// Redaction is idempotent.
    #[test]
    fn redact_is_idempotent(s in hostile_text()) {
        let once = SecretRedactor::redact(&s);
        prop_assert_eq!(SecretRedactor::redact(&once), once);
    }

    /// Sanitized (model-facing) text never carries a raw structural delimiter.
    #[test]
    fn sanitize_removes_every_delimiter(s in hostile_text()) {
        let out = SecretRedactor::sanitize(&s);
        prop_assert!(!out.contains(['⟦', '⟧', '⟪', '⟫']), "{out:?}");
    }

    /// A packet built from arbitrary field text renders to exactly one packet and parses back.
    #[test]
    fn packet_render_parse_round_trips(subject in hostile_text(), key in hostile_text(), value in hostile_text()) {
        prop_assume!(!subject.trim().is_empty() && !key.is_empty());
        let p = AhtlPacket::new(
            PacketKind::CodeDelta,
            &subject,
            None,
            vec![AhtlClause::new(ClauseOp::Neutral, &key, Some(value))],
            Some(ArtifactId(0x0123_4567_89ab_cdef)),
            [0; 8],
        );
        let wire = p.render();
        let stream = AhtlStream::parse_stream(&wire).expect("own output parses");
        prop_assert_eq!(stream.packets.len(), 1, "{}", wire);
        prop_assert_eq!(AhtlPacket::parse(&wire).expect("parses"), p);
    }

    /// Clause parsing never panics on arbitrary input.
    #[test]
    fn clause_parse_is_total(s in hostile_text()) {
        let _ = AhtlClause::parse(&s).render();
    }
}
