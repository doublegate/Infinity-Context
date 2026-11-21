//! Synthetic credentials must be removed before storage, independent of model-facing encoding.
use icx_adapters::InfinityContextMmu;
use icx_core::AgentKind;

#[test]
fn yaml_secrets_cannot_be_recovered_from_ingested_cas_bytes() {
    let mut mmu = InfinityContextMmu::default();
    for raw in [
        "password: |\r\n  firstpiece\r\n  secondpiece\r\npublic: ok\r\n",
        "password:\n  user=correcthorse\npublic: ok\n",
    ] {
        let receipt = mmu.ingest_payload(AgentKind::Codex, "config.yaml", raw, false);
        let stored = mmu.deref_text(receipt.artifact_id).unwrap();
        for secret in ["firstpiece", "secondpiece", "correcthorse"] {
            assert!(!stored.contains(secret), "{stored:?}");
            assert!(!receipt.compressed_ahtl.contains(secret));
        }
        assert!(stored.contains("public: ok"), "{stored:?}");
        assert_eq!(icx_codec::SecretRedactor::redact(&stored), stored);
    }
}
