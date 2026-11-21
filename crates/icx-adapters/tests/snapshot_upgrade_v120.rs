//! Older derived indexes are rebuilt from the preserved event/CAS source, not reinterpreted.
use icx_adapters::InfinityContextMmu;
use icx_core::AgentKind;

#[test]
fn legacy03_snapshot_is_rebuilt_and_saved_as04_without_changing_cas() {
    let base = std::env::temp_dir()
        .join(format!(
            "icx-v120-tests-{}",
            std::env::var("USER").unwrap_or_default()
        ))
        .join(format!("snapshot-upgrade-{}", std::process::id()));
    let mut mmu = InfinityContextMmu::default();
    let receipt = mmu.ingest_payload(AgentKind::Codex, "modern", "preserved_unique_v120", false);
    mmu.save_workspace_dir(&base).unwrap();
    // Unsealed synthetic test store, owned by this test. Inject a real old-writer snapshot;
    // its one-entry event count matches, so the coordinator must check lexical completeness.
    let connection = rusqlite::Connection::open(base.join("icx.sqlite3")).unwrap();
    connection
        .execute(
            "UPDATE meta SET v=?1 WHERE k='hdc_index'",
            [include_bytes!("fixtures/legacy03.bin").as_slice()],
        )
        .unwrap();
    drop(connection);
    let mut loaded = InfinityContextMmu::default();
    loaded.load_workspace_dir(&base).unwrap();
    assert!(loaded.hdc_index.has_exact_lexical_index());
    assert_eq!(
        loaded
            .hdc_index
            .query_associative("preserved_unique_v120", 1)[0]
            .artifact_id,
        receipt.artifact_id
    );
    assert_eq!(
        loaded.deref_text(receipt.artifact_id).unwrap(),
        "preserved_unique_v120"
    );
    loaded.save_workspace_dir(&base).unwrap();
    let connection = rusqlite::Connection::open(base.join("icx.sqlite3")).unwrap();
    let bytes: Vec<u8> = connection
        .query_row("SELECT v FROM meta WHERE k='hdc_index'", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(&bytes[..8], b"ICXHDC04");
    drop(connection);
    std::fs::remove_dir_all(base).unwrap();
}
