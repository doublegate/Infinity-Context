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

#[test]
fn wall_clock_mode_records_real_monotonic_timestamps() {
    // ICX-AUD-048: bi-temporal edges used a synthetic counter starting at a constant.
    let mut mmu = InfinityContextMmu::default();
    mmu.use_wall_clock(true);
    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    for i in 0..3 {
        mmu.ingest_payload(
            AgentKind::ClaudeCode,
            "Bash:x",
            &format!("status ok {i}\n"),
            false,
        );
    }
    let times: Vec<u64> = mmu.graph.edges().iter().map(|e| e.t_tx_start).collect();
    assert_ne!(times.len(), 0, "graph recorded edges");
    assert!(
        times.iter().all(|&t| t >= before && t < before + 60_000),
        "{times:?}"
    );
    assert!(times.windows(2).all(|w| w[1] >= w[0]), "{times:?}");
}

#[test]
fn saving_one_mmu_to_a_second_directory_writes_all_content_there() {
    // PR #20 review: the persisted-id cache must be per destination store.
    let (a_dir, b_dir) = (tmp_dir("dest_a"), tmp_dir("dest_b"));
    let mut mmu = InfinityContextMmu::default();
    let ids = populate(&mut mmu, 12);
    mmu.save_workspace_dir(&a_dir).expect("save A");
    mmu.save_workspace_dir(&b_dir).expect("save B");
    let mut fresh = InfinityContextMmu::default();
    fresh.load_workspace_dir(&b_dir).expect("load B");
    for id in ids {
        assert!(fresh.deref_text(id).is_ok(), "artifact {id} missing from B");
    }
    let _ = std::fs::remove_dir_all(&a_dir);
    let _ = std::fs::remove_dir_all(&b_dir);
}

#[cfg(unix)]
#[test]
fn reopening_a_store_tightens_widened_file_modes() {
    // PR #20 review (Antigravity): an existing store whose mode was widened is re-tightened.
    use std::os::unix::fs::PermissionsExt;
    let dir = tmp_dir("widened");
    let mut a = InfinityContextMmu::default();
    populate(&mut a, 2);
    a.save_workspace_dir(&dir).unwrap();
    for f in ["icx.sqlite3", "icx.sqlite3-wal", "icx.sqlite3-shm"] {
        let p = dir.join(f);
        if p.exists() {
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o644)).unwrap();
        }
    }
    a.save_workspace_dir(&dir).unwrap();
    for f in ["icx.sqlite3", "icx.sqlite3-wal", "icx.sqlite3-shm"] {
        let p = dir.join(f);
        if p.exists() {
            let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "{f} left at {mode:o}");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn emitted_block_digests_live_in_their_own_table_not_the_state_blob() {
    // PR #20 review (Antigravity): the emitted-block set grows with every stub handed to an agent,
    // so it is stored incrementally instead of being re-serialized into `mmu_state` on each save.
    let dir = tmp_dir("emitted");
    let mut a = InfinityContextMmu::default();
    let blocks: Vec<String> = (0..50)
        .map(|i| format!("⟪ICX:STUB│#icx:{i:016x}⟫"))
        .collect();
    for b in &blocks {
        a.record_emitted(b);
    }
    a.save_workspace_dir(&dir).unwrap();
    let conn = rusqlite::Connection::open(dir.join("icx.sqlite3")).unwrap();
    let rows: i64 = conn
        .query_row("SELECT count(*) FROM emitted", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 50);
    let state: Vec<u8> = conn
        .query_row("SELECT v FROM meta WHERE k = 'mmu_state'", [], |r| r.get(0))
        .unwrap();
    assert!(
        !String::from_utf8_lossy(&state).contains("emitted_blocks"),
        "state blob still carries emitted digests"
    );
    drop(conn);
    a.record_emitted("⟪ICX:STUB│late⟫");
    a.save_workspace_dir(&dir).unwrap();
    let mut b = InfinityContextMmu::default();
    b.load_workspace_dir(&dir).unwrap();
    assert!(blocks.iter().all(|x| b.is_emitted(x)));
    assert!(b.is_emitted("⟪ICX:STUB│late⟫"));
    assert!(!b.is_emitted("⟪ICX:STUB│never⟫"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn loading_an_empty_directory_clears_emitted_block_digests() {
    // PR #20 review (Antigravity): a load that resets the MMU must not keep trust in blocks
    // emitted for a previous workspace.
    let dir = tmp_dir("emptyload");
    std::fs::create_dir_all(&dir).unwrap();
    let mut mmu = InfinityContextMmu::default();
    mmu.record_emitted("⟪ICX:STUB│old⟫");
    mmu.load_workspace_dir(&dir).unwrap();
    assert!(!mmu.is_emitted("⟪ICX:STUB│old⟫"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_save_repairs_rows_deleted_behind_a_long_lived_mmu() {
    // PR #20 review (Antigravity): if the persisted-id cache drifts from the table contents,
    // the next save rewrites the missing rows instead of sealing a store that cannot load.
    let dir = tmp_dir("drift");
    let mut mmu = InfinityContextMmu::default();
    let ids = populate(&mut mmu, 6);
    mmu.save_workspace_dir(&dir).unwrap();
    let conn = rusqlite::Connection::open(dir.join("icx.sqlite3")).unwrap();
    conn.execute(
        "DELETE FROM chunks WHERE id = (SELECT min(id) FROM chunks)",
        [],
    )
    .unwrap();
    conn.execute("DELETE FROM emitted", []).unwrap();
    drop(conn);
    mmu.save_workspace_dir(&dir).unwrap();
    let mut fresh = InfinityContextMmu::default();
    fresh
        .load_workspace_dir(&dir)
        .expect("repaired store loads");
    for id in ids {
        assert!(fresh.deref_text(id).is_ok());
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn saving_a_stale_snapshot_over_a_newer_store_is_refused() {
    // PR #20 review (Antigravity): sealing in-memory state that predates another writer's commit
    // would write a seal that does not cover the rows on disk. The save must refuse instead.
    let dir = tmp_dir("stale");
    let mut a = InfinityContextMmu::default();
    populate(&mut a, 3);
    a.save_workspace_dir(&dir).unwrap();
    let mut b = InfinityContextMmu::default();
    b.load_workspace_dir(&dir).unwrap();
    b.ingest_payload(AgentKind::Codex, "Bash:ls", "from b\n", false);
    b.save_workspace_dir(&dir).unwrap();
    a.ingest_payload(AgentKind::Codex, "Bash:pwd", "from a\n", false);
    let err = a.save_workspace_dir(&dir).unwrap_err();
    assert!(err.to_string().contains("changed"), "{err}");
    // After reloading, A can save again.
    a.load_workspace_dir(&dir).unwrap();
    a.ingest_payload(AgentKind::Codex, "Bash:pwd", "from a\n", false);
    a.save_workspace_dir(&dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn extra_rows_in_the_tables_are_removed_before_sealing() {
    // PR #20 review (Antigravity): rows on disk that the in-memory state does not hold (e.g. a
    // planted `emitted` digest) must not survive a save whose seal does not cover them.
    let dir = tmp_dir("extra");
    let mut mmu = InfinityContextMmu::default();
    populate(&mut mmu, 3);
    mmu.save_workspace_dir(&dir).unwrap();
    let conn = rusqlite::Connection::open(dir.join("icx.sqlite3")).unwrap();
    conn.execute("INSERT INTO emitted (digest) VALUES (zeroblob(32))", [])
        .unwrap();
    drop(conn);
    mmu.save_workspace_dir(&dir).unwrap();
    let conn = rusqlite::Connection::open(dir.join("icx.sqlite3")).unwrap();
    let planted: i64 = conn
        .query_row(
            "SELECT count(*) FROM emitted WHERE digest = zeroblob(32)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(planted, 0, "planted emitted row survived a save");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn state_built_on_a_new_workspace_survives_a_store_appearing_empty() {
    // PR #20 review (Antigravity): a first flush that fails after creating an empty store must
    // not make the retry's reload wipe the unsaved state.
    let dir = tmp_dir("firstflush");
    std::fs::create_dir_all(&dir).unwrap();
    let mut mmu = InfinityContextMmu::default();
    mmu.load_workspace_dir(&dir).unwrap();
    let id = mmu
        .ingest_payload(AgentKind::Codex, "Bash:ls", "kept across retry\n", false)
        .artifact_id;
    std::fs::write(dir.join("icx.sqlite3"), b"").unwrap(); // empty store, generation 0
    mmu.load_workspace_dir(&dir).unwrap();
    assert!(
        mmu.deref_text(id).is_ok(),
        "unsaved state was replaced by an empty store"
    );
    mmu.save_workspace_dir(&dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
