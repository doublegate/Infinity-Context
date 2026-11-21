//! v1.2.1: tokens joined by `:` or `/` (`Bash:seq`, `crates/icx-core`) are also indexed by their
//! components, so a query for the bare command or path segment finds the entry.
use icx_core::ArtifactId;
use icx_hdc::HdcAssociativeIndex;

#[test]
fn a_command_subject_is_found_by_its_bare_command_word() {
    let mut index = HdcAssociativeIndex::new();
    for i in 0..300u64 {
        let content = format!("noise{i} line about build {} and docs", i * 7);
        index.observe_event(ArtifactId(i + 1), "Edit", &content, false);
    }
    // Entries that merely mention the command text.
    for i in 0..5u64 {
        index.observe_event(
            ArtifactId(1000 + i),
            "Read",
            &format!("notes {i}: we ran seq 1 400 earlier"),
            false,
        );
    }
    let numbers = (1..=400)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    let target = ArtifactId(9999);
    index.observe_event(target, "Bash:seq 1 400", &numbers, false);
    for i in 0..20u64 {
        index.observe_event(
            ArtifactId(2000 + i),
            "Edit",
            &format!("later edit {i}"),
            false,
        );
    }
    let hits = index.query_associative("seq 1 400", 5);
    assert_eq!(
        hits.first().map(|h| h.artifact_id),
        Some(target),
        "{hits:?}"
    );
}

#[test]
fn path_segments_are_searchable() {
    let mut index = HdcAssociativeIndex::new();
    for i in 0..200u64 {
        index.observe_event(ArtifactId(i + 1), "Edit", &format!("filler {i}"), false);
    }
    let target = ArtifactId(777);
    index.observe_event(
        target,
        "Read",
        "opened crates/icx-hdc/src/zxqvsegment.rs",
        false,
    );
    let hits = index.query_associative("zxqvsegment", 3);
    assert_eq!(
        hits.first().map(|h| h.artifact_id),
        Some(target),
        "{hits:?}"
    );
}

#[test]
fn snapshots_round_trip_component_tokens() {
    let mut index = HdcAssociativeIndex::new();
    index.observe_event(ArtifactId(1), "Bash:seq 1 400", "1\n2\n", false);
    let decoded = HdcAssociativeIndex::decode_snapshot(&index.encode_snapshot()).expect("decode");
    assert!(decoded.has_exact_lexical_index());
    assert_eq!(
        decoded.query_associative("seq", 1)[0].artifact_id,
        ArtifactId(1)
    );
}

#[test]
fn icxhdc04_snapshots_drop_their_postings_so_the_index_is_rebuilt() {
    let mut index = HdcAssociativeIndex::new();
    index.observe_event(ArtifactId(1), "Bash:seq 1 400", "1\n2\n", false);
    let mut bytes = index.encode_snapshot();
    assert_eq!(&bytes[..8], b"ICXHDC05");
    // ICXHDC04 has the same layout; only its tokenizer differs.
    bytes[..8].copy_from_slice(b"ICXHDC04");
    let decoded = HdcAssociativeIndex::decode_snapshot(&bytes).expect("v4 still decodes");
    assert!(
        !decoded.has_exact_lexical_index(),
        "v4 postings must not be reused"
    );
    assert_eq!(decoded.len(), 1);
}
