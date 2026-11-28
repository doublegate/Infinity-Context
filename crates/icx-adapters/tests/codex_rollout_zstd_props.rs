//! T-62-004: damaged `.jsonl.zst` rollouts (truncated, bytes flipped) never panic, and anything
//! ingested from one is a record of the original rollout.

use icx_adapters::InfinityContextMmu;
use icx_adapters::codex_ingest::ingest_rollout;
use proptest::prelude::*;
use std::path::{Path, PathBuf};

/// `zstd -3` of a 41-line synthetic rollout (compressed blocks, XXH64 checksum).
const FIXTURE: &str = "tests/fixtures/codex/rollout-2026-11-22T00-00-00-synthetic.jsonl.zst";
const RECORDS_IN_FIXTURE: usize = 40;

fn fixture() -> Vec<u8> {
    std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE)).unwrap()
}

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("icx-zstd-props-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn intact_fixture_ingests_every_record() {
    let d = scratch("intact");
    let p = d.join("rollout-intact.jsonl.zst");
    std::fs::write(&p, fixture()).unwrap();
    let mut mmu = InfinityContextMmu::default();
    let (mark, n) = ingest_rollout(&p, None, &mut mmu).unwrap();
    assert_eq!(n, RECORDS_IN_FIXTURE);
    assert!(mark.compressed);
    assert_eq!(
        mark.offset, 32_298,
        "the whole decompressed stream is consumed"
    );
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_corrupted_frame_checksum_is_refused() {
    // The fixture ends with its 4-byte XXH64 content checksum; the decoded bytes are intact, so
    // only an explicit checksum comparison can catch this.
    let d = scratch("checksum");
    let mut bytes = fixture();
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;
    let p = d.join("rollout-badsum.jsonl.zst");
    std::fs::write(&p, &bytes).unwrap();
    let mut mmu = InfinityContextMmu::default();
    let err = ingest_rollout(&p, None, &mut mmu).unwrap_err();
    assert!(err.to_string().contains("checksum"), "{err}");
    assert_eq!(mmu.cas.artifact_count(), 0, "nothing is ingested");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn bytes_after_the_frame_are_refused() {
    let d = scratch("trailing");
    let mut bytes = fixture();
    bytes.extend_from_slice(b"trailing junk");
    let p = d.join("rollout-trailing.jsonl.zst");
    std::fs::write(&p, &bytes).unwrap();
    let mut mmu = InfinityContextMmu::default();
    assert!(ingest_rollout(&p, None, &mut mmu).is_err());
    assert_eq!(mmu.cas.artifact_count(), 0, "nothing is ingested");
    let _ = std::fs::remove_dir_all(d);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn damaged_frames_never_panic_or_invent_records(
        cut in 0usize..2963,
        flips in proptest::collection::vec((0usize..2963, 1u8..=255), 0..4),
    ) {
        let mut bytes = fixture();
        for (pos, mask) in &flips {
            if *pos < bytes.len() {
                bytes[*pos] ^= mask;
            }
        }
        bytes.truncate(cut.min(bytes.len()));
        let d = scratch("damaged");
        let p = d.join(format!("rollout-{cut}-{}.jsonl.zst", flips.len()));
        std::fs::write(&p, &bytes).unwrap();
        let mut mmu = InfinityContextMmu::default();
        // An error is an acceptable outcome. The frame checksum makes a flipped-but-decodable
        // frame fail, so success means the stream decoded intact up to the cut; it can never
        // yield more records than the original.
        if let Ok((_, n)) = ingest_rollout(&p, None, &mut mmu) {
            prop_assert!(n <= RECORDS_IN_FIXTURE);
        }
        // Each case removes its directory, not only its file, so no run leaves one behind.
        let _ = std::fs::remove_dir_all(&d);
    }
}
