//! v1.1.0 SQLite workspace store: round trip, atomic generations, and legacy JSON migration
//! (ICX-AUD-001, ICX-AUD-047).

use icx_adapters::mmu::InfinityContextMmu;
use icx_core::AgentKind;
use std::path::PathBuf;

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "icx_store_test_{tag}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn populate(mmu: &mut InfinityContextMmu, n: usize) -> Vec<icx_core::ArtifactId> {
    (0..n)
        .map(|i| {
            mmu.ingest_payload(
                AgentKind::ClaudeCode,
                &format!("Bash:cargo test -p crate{i}"),
                &format!("test m{i}::case ... ok\nerror[E0{i:03}]: mismatch at line {i}\n"),
                i % 7 == 0,
            )
            .artifact_id
        })
        .collect()
}

#[test]
fn sqlite_store_round_trips_cas_index_dag_and_graph() {
    let dir = tmp_dir("roundtrip");
    let mut a = InfinityContextMmu::default();
    let ids = populate(&mut a, 40);
    a.save_workspace_dir(&dir).expect("save");
    assert!(
        dir.join("icx.sqlite3").is_file(),
        "single SQLite store file"
    );
    assert!(
        !dir.join("cas_store.json").exists(),
        "no legacy JSON written"
    );

    let mut b = InfinityContextMmu::default();
    b.load_workspace_dir(&dir).expect("load");
    for id in &ids {
        assert_eq!(a.deref_text(*id).unwrap(), b.deref_text(*id).unwrap());
    }
    assert_eq!(a.dag.node_count(), b.dag.node_count());
    assert_eq!(a.graph.edges(), b.graph.edges());
    assert_eq!(
        a.hdc_index.query_associative("E0017 mismatch", 5),
        b.hdc_index.query_associative("E0017 mismatch", 5)
    );
    assert_eq!(a.hdc_index.encode_snapshot(), b.hdc_index.encode_snapshot());

    // Subsequent ingest + save on the restored MMU keeps working.
    let extra = b
        .ingest_payload(AgentKind::Codex, "Bash:ls", "a\nb\nc\n", false)
        .artifact_id;
    b.save_workspace_dir(&dir).expect("save 2");
    let mut c = InfinityContextMmu::default();
    c.load_workspace_dir(&dir).expect("load 2");
    assert!(c.deref_text(extra).is_ok());
    assert_eq!(c.cas.artifact_count(), ids.len() + 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sqlite_store_generation_advances_once_per_save() {
    let dir = tmp_dir("generation");
    let mut a = InfinityContextMmu::default();
    populate(&mut a, 3);
    a.save_workspace_dir(&dir).unwrap();
    let g1 = InfinityContextMmu::workspace_generation(&dir).unwrap();
    a.save_workspace_dir(&dir).unwrap();
    let g2 = InfinityContextMmu::workspace_generation(&dir).unwrap();
    assert_eq!(g2, g1 + 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn legacy_json_workspace_is_migrated_without_deleting_it() {
    let dir = tmp_dir("legacy");
    let mut a = InfinityContextMmu::default();
    let ids = populate(&mut a, 12);
    a.save_workspace_dir_legacy_json(&dir).expect("legacy save");
    assert!(dir.join("cas_store.json").is_file());

    let mut b = InfinityContextMmu::default();
    b.load_workspace_dir(&dir).expect("migrating load");
    for id in &ids {
        assert_eq!(a.deref_text(*id).unwrap(), b.deref_text(*id).unwrap());
    }
    assert!(
        dir.join("icx.sqlite3").is_file(),
        "migration writes the SQLite store"
    );
    assert!(
        dir.join("cas_store.json").is_file(),
        "legacy files are preserved"
    );
    assert_eq!(
        a.hdc_index.query_associative("cargo test crate3", 3),
        b.hdc_index.query_associative("cargo test crate3", 3)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn sqlite_store_files_are_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tmp_dir("perms");
    let mut a = InfinityContextMmu::default();
    populate(&mut a, 2);
    a.save_workspace_dir(&dir).unwrap();
    let dir_mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
    let db_mode = std::fs::metadata(dir.join("icx.sqlite3"))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(dir_mode, 0o700, "workspace directory must be 0700");
    assert_eq!(db_mode, 0o600, "store must be 0600");
    let _ = std::fs::remove_dir_all(&dir);
}
