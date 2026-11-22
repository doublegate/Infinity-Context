//! Exact identifier retrieval must survive a saturated approximate content filter.
use icx_core::ArtifactId;
use icx_hdc::HdcAssociativeIndex;

fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

#[test]
fn varied_and_multilingual_identifiers_are_not_lost_by_candidate_filtering() {
    for prefix in ["needle", "标识符"] {
        let mut state = 1;
        let mut index = HdcAssociativeIndex::new();
        let mut queries = Vec::new();
        //256 entries already fail against the a996d59 writer/query implementation: the first
        // exact identifier incorrectly ranks artifact256. Larger scaling belongs to qualification.
        for i in 0..256 {
            let mut words: Vec<String> = (0..120)
                .map(|_| format!("word{:x}", next(&mut state) % 100_000))
                .collect();
            let needle = format!("{prefix}{i:05}x{:08x}", next(&mut state));
            words[(next(&mut state) % 120) as usize] = needle.clone();
            index.observe_event(
                ArtifactId(i + 1),
                "Bash:cargo test",
                &words.join(" "),
                false,
            );
            if i % 100 == 0 {
                queries.push((ArtifactId(i + 1), needle));
            }
        }
        for (id, query) in &queries {
            let hits = index.query_associative(query, 5);
            assert_eq!(hits.first().map(|h| h.artifact_id), Some(*id), "{query}");
        }
        let restored = HdcAssociativeIndex::decode_snapshot(&index.encode_snapshot()).unwrap();
        for (id, query) in &queries {
            assert_eq!(restored.query_associative(query, 1)[0].artifact_id, *id);
        }
    }
}
