use icx_core::ArtifactId;
use icx_hdc::HdcAssociativeIndex;

#[test]
fn independently_generated_legacy03_snapshot_remains_readable() {
    let index =
        HdcAssociativeIndex::decode_snapshot(include_bytes!("fixtures/legacy03.bin")).unwrap();
    assert_eq!(index.len(), 1);
    assert!(!index.has_exact_lexical_index());
    assert_eq!(
        index.query_associative("legacy unique", 1)[0].artifact_id,
        ArtifactId(1)
    );
    let reread = HdcAssociativeIndex::decode_snapshot(&index.encode_snapshot()).unwrap();
    assert!(!reread.has_exact_lexical_index());
    assert_eq!(
        reread.query_associative("legacy unique", 1),
        index.query_associative("legacy unique", 1)
    );
}

#[test]
fn snapshots_reject_nonfinite_and_out_of_range_entry_state() {
    let mut index = HdcAssociativeIndex::new();
    index.observe_event(ArtifactId(1), "x", "hello unique", false);
    // Header + count + id + length-prefixed one-byte subject + 160 HV words.
    let coords = 8 + 8 + 8 + 8 + 1 + 1280;
    let surprise = coords + 128 + 4;
    for value in [f32::NAN, f32::INFINITY, -0.5, 1.5] {
        let mut bytes = index.encode_snapshot();
        bytes[surprise..surprise + 4].copy_from_slice(&value.to_le_bytes());
        assert!(HdcAssociativeIndex::decode_snapshot(&bytes).is_err());
    }
    for value in [f64::NAN, f64::INFINITY, 1.5] {
        let mut bytes = index.encode_snapshot();
        bytes[coords..coords + 8].copy_from_slice(&value.to_le_bytes());
        assert!(HdcAssociativeIndex::decode_snapshot(&bytes).is_err());
    }
}

#[test]
fn snapshots_reject_forged_cached_entry_summaries() {
    let mut index = HdcAssociativeIndex::new();
    index.observe_event(ArtifactId(1), "x", "hello unique", false);
    let summary = 8 + 8 + 8 + 8 + 1 + 1280 + 128 + 4 + 4;
    for offset in [0, 8, 12, 14, 15, 16] {
        let mut bytes = index.encode_snapshot();
        bytes[summary + offset] ^= 0x80;
        assert!(
            HdcAssociativeIndex::decode_snapshot(&bytes).is_err(),
            "{offset}"
        );
    }
}

#[test]
fn snapshots_reject_nonfinite_surprise_parameters_and_false_bundle_readout() {
    let index = HdcAssociativeIndex::new();
    for distance_from_end in [29, 25, 21] {
        let mut bytes = index.encode_snapshot();
        let offset = bytes.len() - distance_from_end;
        bytes[offset..offset + 4].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(HdcAssociativeIndex::decode_snapshot(&bytes).is_err());
    }
    let mut bytes = index.encode_snapshot();
    let bundle_readout = 16 + 132 + 8 + 10_240 * 4;
    bytes[bundle_readout] ^= 1;
    assert!(HdcAssociativeIndex::decode_snapshot(&bytes).is_err());
}
