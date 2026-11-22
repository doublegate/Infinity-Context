//! v1.2.0 lexical-index bounds and ranking: oversized tokens never make a snapshot
//! undecodable, rare identifiers outrank stop-word matches, and common tokens do not drag the
//! whole corpus into exact scoring.

use icx_core::ArtifactId;
use icx_hdc::HdcAssociativeIndex;

#[test]
fn oversized_content_token_does_not_make_the_snapshot_undecodable() {
    let mut index = HdcAssociativeIndex::new();
    index.observe_event(ArtifactId(1), "Bash:cat blob", &"ab".repeat(40_000), false);
    index.observe_event(
        ArtifactId(2),
        "Bash:cat",
        "needle_identifier inside a normal output",
        false,
    );
    let restored = HdcAssociativeIndex::decode_snapshot(&index.encode_snapshot())
        .expect("snapshot with an 80 KB token run must still decode");
    assert!(restored.has_exact_lexical_index());
    assert_eq!(
        restored.query_associative("needle_identifier", 1)[0].artifact_id,
        ArtifactId(2)
    );
    // A query carrying the same oversized run is accepted and does not panic.
    let _ = restored.query_associative(&"ab".repeat(40_000), 3);
}

#[test]
fn oversized_subject_does_not_make_the_snapshot_undecodable() {
    let mut index = HdcAssociativeIndex::new();
    index.observe_event(ArtifactId(1), &"s".repeat(70_000), "body", false);
    HdcAssociativeIndex::decode_snapshot(&index.encode_snapshot())
        .expect("snapshot with a 70 KB subject must still decode");
}

fn stop_word_corpus() -> HdcAssociativeIndex {
    let mut index = HdcAssociativeIndex::new();
    index.observe_event(
        ArtifactId(1),
        "notes.md",
        "where is the build log and how is it rotated",
        false,
    );
    index.observe_event(
        ArtifactId(2),
        "src/vault.rs",
        "pub fn locate_quantum_passkey() -> Key { todo!() }",
        false,
    );
    for i in 0..30u64 {
        index.observe_event(
            ArtifactId(100 + i),
            &format!("docs/faq_{i}.md"),
            &format!("where is file {i}? it is in the folder where it is kept"),
            false,
        );
    }
    index
}

#[test]
fn rare_identifier_outranks_stop_word_matches() {
    let index = stop_word_corpus();
    let hits = index.query_associative("where is locate_quantum_passkey", 5);
    assert_eq!(hits[0].artifact_id, ArtifactId(2), "{hits:?}");
    let restored = HdcAssociativeIndex::decode_snapshot(&index.encode_snapshot()).unwrap();
    assert_eq!(
        restored.query_associative("where is locate_quantum_passkey", 5)[0].artifact_id,
        ArtifactId(2)
    );
}

#[test]
fn rare_identifier_mixed_with_very_common_tokens_ranks_first_in_a_large_corpus() {
    let mut index = HdcAssociativeIndex::new();
    for i in 0..3_000u64 {
        index.observe_event(
            ArtifactId(i + 1),
            &format!("src/m{i}.rs"),
            &format!("pub fn helper_{i}() -> usize {{ {i} }} #[test] fn test_{i}() {{}}"),
            false,
        );
    }
    for (query, want) in [
        ("pub fn helper_17", 18),
        ("test pub fn usize helper_2999", 3_000),
        ("helper_1234 pub fn test", 1_235),
    ] {
        let hits = index.query_associative(query, 5);
        assert_eq!(hits[0].artifact_id, ArtifactId(want), "{query}: {hits:?}");
    }
}
