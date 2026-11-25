// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later AND AGPL-3.0-or-later

//! The lexical index (v1.5.0, T-71-001) through the MMU and the workspace store: built only when
//! enabled, backfilled in bounded batches, never trusted beyond the sealed CAS, and invisible to
//! the store schema version.

use icx_adapters::InfinityContextMmu;
use icx_core::AgentKind;
use rusqlite::Connection;
use std::path::PathBuf;

fn workspace(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "icx-lexical-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn db(dir: &std::path::Path) -> Connection {
    Connection::open(dir.join("icx.sqlite3")).unwrap()
}

fn has_tables(dir: &std::path::Path) -> bool {
    db(dir)
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name IN ('lex_words', 'lex_trigrams')",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
        == 2
}

#[test]
fn enabled_index_finds_words_identifier_parts_and_substrings() {
    let dir = workspace("on");
    let mut mmu = InfinityContextMmu::default();
    mmu.set_lexical_index(true);
    let parse = mmu
        .ingest_payload(
            AgentKind::ClaudeCode,
            "Read:src/net/http.rs",
            "pub fn parseHttpRequest(buf: &[u8]) -> Request { todo!() }\n",
            false,
        )
        .artifact_id;
    let err = mmu
        .ingest_payload(
            AgentKind::ClaudeCode,
            "Bash:cargo build",
            "error[E0502]: cannot borrow `chars` as immutable\n",
            false,
        )
        .artifact_id;
    mmu.save_workspace_dir(&dir).unwrap();
    assert!(has_tables(&dir));

    let ids = |q: &str| -> Vec<_> {
        mmu.lexical_matches(q, 5)
            .into_iter()
            .map(|(id, _)| id)
            .collect()
    };
    assert_eq!(ids("request parsing"), [parse]);
    assert_eq!(ids("borrow immutable"), [err]);
    assert_eq!(mmu.substring_matches("E0502", 5), [err]);
    assert_eq!(mmu.substring_matches("Http", 5), [parse]);
    assert_eq!(mmu.substring_matches("not-there", 5), []);

    // The store schema version is unchanged, so older builds still open the store.
    let schema: i64 = db(&dir)
        .query_row("SELECT v FROM meta WHERE k = 'schema'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(schema, 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn disabled_index_builds_nothing_and_answers_nothing() {
    let dir = workspace("off");
    let mut mmu = InfinityContextMmu::default();
    mmu.set_lexical_index(false);
    mmu.ingest_payload(AgentKind::Codex, "log", "alpha beta gamma\n", false);
    mmu.save_workspace_dir(&dir).unwrap();
    assert!(!has_tables(&dir));
    assert_eq!(mmu.lexical_matches("alpha", 5), []);
    assert_eq!(mmu.substring_matches("alpha", 5), []);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn turning_the_index_on_backfills_existing_artifacts_in_batches() {
    let dir = workspace("backfill");
    let mut mmu = InfinityContextMmu::default();
    mmu.set_lexical_index(false);
    let n = icx_adapters::lexical::BACKFILL_BATCH + 50;
    for i in 0..n {
        mmu.ingest_payload(
            AgentKind::Codex,
            "log",
            &format!("entry{i} marker{i}\n"),
            false,
        );
    }
    mmu.save_workspace_dir(&dir).unwrap();

    let mut on = InfinityContextMmu::default();
    on.load_workspace_dir(&dir).unwrap();
    on.set_lexical_index(true);
    let fresh = on
        .ingest_payload(AgentKind::Codex, "log", "freshly added line\n", false)
        .artifact_id;
    on.save_workspace_dir(&dir).unwrap();
    let rows = |dir: &std::path::Path| -> i64 {
        db(dir)
            .query_row("SELECT count(*) FROM lex_words", [], |r| r.get(0))
            .unwrap()
    };
    // The new artifact plus one batch of older ones.
    assert_eq!(
        rows(&dir),
        i64::try_from(icx_adapters::lexical::BACKFILL_BATCH).unwrap() + 1
    );
    assert_eq!(on.lexical_matches("freshly", 5)[0].0, fresh);
    // The next save finishes the backfill.
    on.ingest_payload(AgentKind::Codex, "log", "one more\n", false);
    on.save_workspace_dir(&dir).unwrap();
    assert_eq!(rows(&dir), i64::try_from(n).unwrap() + 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rows_the_sealed_cas_does_not_hold_are_ignored() {
    let dir = workspace("planted");
    let mut mmu = InfinityContextMmu::default();
    mmu.set_lexical_index(true);
    mmu.ingest_payload(AgentKind::Codex, "log", "genuine content here\n", false);
    mmu.save_workspace_dir(&dir).unwrap();
    // A row written behind icx's back names an artifact that does not exist.
    db(&dir)
        .execute(
            "INSERT INTO lex_words (rowid, body, parts) VALUES (424242, 'planted content', '')",
            [],
        )
        .unwrap();
    db(&dir)
        .execute(
            "INSERT INTO lex_trigrams (rowid, body) VALUES (424242, 'planted content')",
            [],
        )
        .unwrap();
    assert_eq!(mmu.lexical_matches("planted", 5), []);
    assert_eq!(mmu.substring_matches("planted", 5), []);
    // Genuine rows still answer.
    assert_eq!(mmu.lexical_matches("genuine", 5).len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_broken_index_never_fails_a_save() {
    let dir = workspace("broken");
    let mut mmu = InfinityContextMmu::default();
    mmu.set_lexical_index(false);
    mmu.ingest_payload(AgentKind::Codex, "log", "first save\n", false);
    mmu.save_workspace_dir(&dir).unwrap();
    // An ordinary table under the index's name makes every index write fail.
    db(&dir)
        .execute_batch("CREATE TABLE lex_words (x INTEGER);")
        .unwrap();
    mmu.set_lexical_index(true);
    let id = mmu
        .ingest_payload(AgentKind::Codex, "log", "second save survives\n", false)
        .artifact_id;
    mmu.save_workspace_dir(&dir).unwrap();
    // The save committed: a fresh load sees the new artifact.
    let mut fresh = InfinityContextMmu::default();
    fresh.load_workspace_dir(&dir).unwrap();
    assert_eq!(fresh.deref_text(id).unwrap(), "second save survives\n");
    // And nothing was half-written into the index.
    assert_eq!(mmu.lexical_matches("survives", 5), []);
    let _ = std::fs::remove_dir_all(&dir);
}
