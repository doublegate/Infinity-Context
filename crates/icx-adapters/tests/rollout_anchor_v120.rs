//! A localized same-size rewrite must invalidate the sampled rollout anchor.
use icx_adapters::{CodexRolloutTailer, InfinityContextMmu};
use icx_core::ArtifactId;
use std::io::{Seek, SeekFrom, Write};

#[test]
fn rewrite_in_two_thirds_anchor_window_replays_changed_output() {
    let root = std::path::Path::new("/tmp/Infinity-Context/v120/rollout-anchor-tests");
    std::fs::create_dir_all(root).unwrap();
    let path = root.join(format!("{}.jsonl", std::process::id()));
    let old =
        "{\"type\":\"function_call_output\",\"name\":\"exec\",\"output\":\"old0000000000\"}\n";
    let new = old.replace("old0000000000", "new0000000000");
    assert_eq!(old.len(), new.len());
    std::fs::write(&path, old.repeat(600)).unwrap();
    let mut mmu = InfinityContextMmu::default();
    let mut tailer = CodexRolloutTailer::new();
    assert_eq!(tailer.poll_file(&path, &mut mmu).unwrap(), 600);
    let mut file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    // This row lies inside the 2*len/3 window, outside the other three 4 KiB windows.
    file.seek(SeekFrom::Start((401 * old.len()) as u64))
        .unwrap();
    file.write_all(new.as_bytes()).unwrap();
    file.sync_all().unwrap();
    assert_eq!(tailer.poll_file(&path, &mut mmu).unwrap(), 600);
    let aid = ArtifactId::hash_bytes(b"new0000000000").0;
    assert_eq!(mmu.deref_text(aid).unwrap(), "new0000000000");
    std::fs::remove_file(path).unwrap();
}
