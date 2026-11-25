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
            "SELECT count(*) FROM sqlite_master WHERE name IN ('lex_ids', 'lex_words', 'lex_trigrams')",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
        == 3
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
    // Tables left by an earlier run with the index on are not read once it is off (#97).
    mmu.set_lexical_index(true);
    mmu.ingest_payload(AgentKind::Codex, "log", "delta epsilon\n", false);
    mmu.save_workspace_dir(&dir).unwrap();
    assert_eq!(mmu.lexical_matches("alpha", 5).len(), 1);
    mmu.set_lexical_index(false);
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
            .query_row("SELECT count(*) FROM lex_ids", [], |r| r.get(0))
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
    // Rows written behind icx's back name an artifact that does not exist.
    db(&dir)
        .execute_batch(
            "INSERT INTO lex_ids (n, id) VALUES (424242, 424242);
             INSERT INTO lex_words (rowid, body, parts) VALUES (424242, 'planted content', '');
             INSERT INTO lex_trigrams (rowid, body) VALUES (424242, 'planted content');",
        )
        .unwrap();
    assert_eq!(mmu.lexical_matches("planted", 5), []);
    // Many planted rows that outrank the genuine one cannot hide it (over-fetch).
    for i in 0..20 {
        let conn = db(&dir);
        conn.execute("INSERT INTO lex_ids (n, id) VALUES (?1, ?1)", [500_000 + i])
            .unwrap();
        conn.execute(
            "INSERT INTO lex_words (rowid, body, parts) VALUES (?1, 'genuine genuine genuine', '')",
            [500_000 + i],
        )
        .unwrap();
    }
    assert_eq!(mmu.lexical_matches("genuine", 1).len(), 1);
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
    // A view under the index's name cannot be dropped or written as a table, so every index
    // write fails. (A stray plain table is simply rebuilt: half a pair is dropped first.)
    db(&dir)
        .execute_batch("CREATE VIEW lex_words AS SELECT 1 AS x;")
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

#[test]
fn an_undroppable_index_never_fails_a_rewrite_save() {
    // The CAS-rewrite path drops the lexical tables; if that fails, the save must still commit
    // (review of #97).
    let dir = workspace("rewrite");
    let mut mmu = InfinityContextMmu::default();
    mmu.set_lexical_index(false);
    mmu.ingest_payload(AgentKind::Codex, "log", "row one\n", false);
    mmu.ingest_payload(AgentKind::Codex, "log", "row two\n", false);
    mmu.save_workspace_dir(&dir).unwrap();
    // A missing row forces the rewrite; a view under the index's name cannot be dropped as a
    // table.
    let conn = db(&dir);
    conn.execute_batch(
        "DELETE FROM artifacts WHERE rowid = (SELECT min(rowid) FROM artifacts);
         CREATE VIEW lex_words AS SELECT 1 AS x;",
    )
    .unwrap();
    drop(conn);
    let id = mmu
        .ingest_payload(AgentKind::Codex, "log", "row three\n", false)
        .artifact_id;
    mmu.save_workspace_dir(&dir).unwrap();
    let mut fresh = InfinityContextMmu::default();
    fresh.load_workspace_dir(&dir).unwrap();
    assert_eq!(fresh.deref_text(id).unwrap(), "row three\n");
    assert_eq!(
        fresh.cas.artifact_count(),
        3,
        "the rewrite restored every row"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// T-71-001 measurement of the production index code on a real store (run with
/// `ICX_MEASURE_STORE=<path/to/icx.sqlite3> cargo test --release -p icx-adapters --test
/// lexical_index measure -- --ignored --nocapture`). The store is opened read-only; each
/// artifact's text is rebuilt from its chunks and indexed with `lexical::insert` into a scratch
/// database in one transaction, as a backfill does (no `optimize`, no `VACUUM`). Prints sizes and
/// timings only, never store content.
#[test]
#[ignore = "measurement; needs ICX_MEASURE_STORE"]
fn measure_lexical_index_on_a_store() {
    use icx_adapters::lexical;
    use icx_core::ArtifactId;
    let Ok(store) = std::env::var("ICX_MEASURE_STORE") else {
        eprintln!("set ICX_MEASURE_STORE to a workspace icx.sqlite3");
        return;
    };
    let src =
        Connection::open_with_flags(&store, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let mut chunks = std::collections::HashMap::new();
    let mut stmt = src.prepare("SELECT id, bytes FROM chunks").unwrap();
    for row in stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?)))
        .unwrap()
    {
        let (id, bytes) = row.unwrap();
        chunks.insert(id, bytes);
    }
    let mut docs: Vec<(ArtifactId, String)> = Vec::new();
    let mut stmt = src.prepare("SELECT id, meta FROM artifacts").unwrap();
    for row in stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
        .unwrap()
    {
        let (id, meta) = row.unwrap();
        let meta: serde_json::Value = serde_json::from_str(&meta).unwrap();
        let mut text = Vec::new();
        for c in meta["chunk_ids"].as_array().unwrap() {
            let c = i64::from_ne_bytes(c.as_u64().unwrap().to_ne_bytes());
            text.extend_from_slice(chunks.get(&c).map_or(&[][..], Vec::as_slice));
        }
        docs.push((
            ArtifactId(u64::from_ne_bytes(id.to_ne_bytes())),
            String::from_utf8_lossy(&text).into_owned(),
        ));
    }
    let bytes: usize = docs.iter().map(|(_, t)| t.len()).sum();
    println!("artifacts={} text={:.1} MB", docs.len(), bytes as f64 / 1e6);

    let path = workspace("measure").with_extension("sqlite3");
    let mut out = Connection::open(&path).unwrap();
    out.execute_batch("CREATE TABLE meta (k TEXT PRIMARY KEY, v BLOB NOT NULL);")
        .unwrap();
    lexical::ensure_tables(&out).unwrap();
    let started = std::time::Instant::now();
    let tx = out.transaction().unwrap();
    for (id, text) in &docs {
        lexical::insert(&tx, *id, text).unwrap();
    }
    tx.commit().unwrap();
    let per = started.elapsed().as_secs_f64() * 1e3 / docs.len().max(1) as f64;
    let size = std::fs::metadata(&path).unwrap().len() as f64 / 1e6;
    println!(
        "both tables: {size:.1} MB, insert {per:.3} ms/artifact (parts computed, both tables)"
    );

    let median = |f: &mut dyn FnMut()| {
        let mut xs: Vec<f64> = (0..50)
            .map(|_| {
                let t = std::time::Instant::now();
                f();
                t.elapsed().as_secs_f64() * 1e3
            })
            .collect();
        xs.sort_by(f64::total_cmp);
        (xs[25], xs[49])
    };
    for q in [
        "cargo test failed",
        "error borrow",
        "session hook",
        "icx install matcher claude",
    ] {
        let (m, worst) = median(&mut || {
            lexical::search_words(&out, q, 20).unwrap();
        });
        let terms = lexical::words_query(q).map_or(0, |e| e.matches(" OR ").count() + 1);
        println!("bm25 {terms} terms: median {m:.2} ms, worst {worst:.2} ms");
    }
    for needle in ["error", "install", "workspace"] {
        let (m, worst) = median(&mut || {
            lexical::trigram_candidates(&out, needle, 160).unwrap();
        });
        println!(
            "trigram {} chars: median {m:.2} ms, worst {worst:.2} ms",
            needle.len()
        );
    }
    drop(out);
    let _ = std::fs::remove_file(&path);
}
