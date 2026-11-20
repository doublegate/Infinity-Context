//! SQLite-backed workspace store (v1.1.0; ICX-AUD-001, ICX-AUD-047).
//!
//! One `icx.sqlite3` file replaces `cas_store.json`, `mmu_state.json` and
//! `bitemporal_graph.json`. Every save is a single transaction, so a crash can no longer leave a
//! mixed-generation workspace. CAS chunks and artifacts are append-only rows, inserted once; the
//! derived state (DAG, epochs, graph, HDC index snapshot) is replaced atomically each save.
//! A monotonically increasing `generation` lets long-lived processes skip reloading when no other
//! writer has touched the store.

use icx_core::cas::StoredArtifact;
use icx_core::chunker::ChunkerConfig;
use icx_core::{ArtifactId, Blake3Digest, IcxError};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::BTreeSet;
use std::path::Path;

/// Store file name inside a workspace directory.
pub const DB_FILE: &str = "icx.sqlite3";
const SCHEMA_VERSION: i64 = 1;

/// Raw persisted workspace contents read in one transaction.
#[derive(Debug, Default)]
pub(crate) struct StoredWorkspace {
    pub generation: u64,
    pub chunk_config: Option<ChunkerConfig>,
    pub chunks: Vec<(ArtifactId, Blake3Digest, Vec<u8>)>,
    pub artifacts: Vec<StoredArtifact>,
    pub graph_json: Option<Vec<u8>>,
    pub mmu_state_json: Option<Vec<u8>>,
    pub hdc_snapshot: Option<Vec<u8>>,
    pub hdc_snapshot_events: Option<u64>,
    pub seal: Option<Vec<u8>>,
    pub chunk_config_raw: Option<Vec<u8>>,
    /// Digests of icx-emitted blocks (append-only `emitted` table, PR #20 review).
    pub emitted: BTreeSet<[u8; 32]>,
}

/// Digest of every persisted component: chunker config, graph, MMU state, HDC snapshot and its
/// event count, each artifact's `(id, full BLAKE3 digest)` in id order, and every emitted-block
/// digest in sorted order. Chunks are covered
/// transitively: `CasStore::from_parts` verifies every chunk digest and reassembles each artifact
/// to its full digest.
pub(crate) fn state_digest(
    chunk_config_json: &[u8],
    graph_json: &[u8],
    mmu_state_json: &[u8],
    hdc_snapshot: &[u8],
    hdc_snapshot_events: u64,
    artifacts: impl Iterator<Item = (ArtifactId, Blake3Digest)>,
    emitted: &BTreeSet<[u8; 32]>,
) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    for part in [chunk_config_json, graph_json, mmu_state_json, hdc_snapshot] {
        h.update(&(part.len() as u64).to_le_bytes());
        h.update(part);
    }
    h.update(&hdc_snapshot_events.to_le_bytes());
    for (id, digest) in artifacts {
        h.update(&id.0.to_le_bytes());
        h.update(&digest.0);
    }
    h.update(&(emitted.len() as u64).to_le_bytes());
    for d in emitted {
        h.update(d);
    }
    *h.finalize().as_bytes()
}

/// Keyed MAC binding a store generation and its full content digest to its canonical path.
pub(crate) fn store_seal(
    key: &[u8; 32],
    canonical_dir: &Path,
    generation: u64,
    state_digest: &[u8; 32],
) -> [u8; 32] {
    let mut h = blake3::Hasher::new_keyed(key);
    h.update(b"ICX_STORE_SEAL_V2\0");
    h.update(canonical_dir.as_os_str().as_encoded_bytes());
    h.update(&generation.to_le_bytes());
    h.update(state_digest);
    *h.finalize().as_bytes()
}

/// Derived state written atomically alongside new CAS rows.
#[derive(Debug)]
pub(crate) struct DerivedState<'a> {
    pub chunk_config: ChunkerConfig,
    pub graph_json: &'a [u8],
    pub mmu_state_json: &'a [u8],
    pub hdc_snapshot: &'a [u8],
    pub hdc_snapshot_events: u64,
    /// Emitted-block digests; only those not yet persisted are inserted.
    pub emitted: &'a BTreeSet<[u8; 32]>,
    /// `(seal key, canonical workspace dir)`; when set, the new generation is MAC-stamped.
    pub seal: Option<(&'a [u8; 32], &'a Path)>,
}

fn db_err(e: impl std::fmt::Display) -> IcxError {
    IcxError::Io(format!("workspace store: {e}"))
}

/// Bit-preserving `u64` <-> SQLite `INTEGER` (`i64`) conversion for 64-bit artifact ids.
const fn id_to_sql(id: ArtifactId) -> i64 {
    i64::from_ne_bytes(id.0.to_ne_bytes())
}
const fn id_from_sql(v: i64) -> ArtifactId {
    ArtifactId(u64::from_ne_bytes(v.to_ne_bytes()))
}

/// Creates `dir` with mode `0700` (Unix) if missing.
pub(crate) fn ensure_private_dir(dir: &Path) -> Result<(), IcxError> {
    if dir.is_dir() {
        return Ok(());
    }
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(dir).map_err(db_err)
}

/// Returns true if a SQLite store exists in `dir`.
pub(crate) fn exists(dir: &Path) -> bool {
    dir.join(DB_FILE).is_file()
}

/// Opens (creating if needed) the workspace store with WAL journaling and owner-only permissions.
pub(crate) fn open(dir: &Path) -> Result<Connection, IcxError> {
    ensure_private_dir(dir)?;
    let path = dir.join(DB_FILE);
    let is_new = !path.exists();
    if is_new {
        // Pre-create the file 0600 so SQLite (which inherits the db file's mode for -wal/-shm)
        // never exposes workspace content under a permissive umask.
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        match opts.open(&path) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(db_err(e)),
        }
    }
    tighten_store_file_modes(dir)?;
    let conn = Connection::open(&path).map_err(db_err)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(db_err)?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(db_err)?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(db_err)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS meta (k TEXT PRIMARY KEY, v BLOB NOT NULL);
         CREATE TABLE IF NOT EXISTS chunks (id INTEGER PRIMARY KEY, digest BLOB NOT NULL, bytes BLOB NOT NULL);
         CREATE TABLE IF NOT EXISTS artifacts (id INTEGER PRIMARY KEY, meta TEXT NOT NULL);
         CREATE TABLE IF NOT EXISTS emitted (digest BLOB PRIMARY KEY) WITHOUT ROWID;",
    )
    .map_err(db_err)?;
    let version: Option<i64> = conn
        .query_row("SELECT v FROM meta WHERE k = 'schema'", [], |r| r.get(0))
        .optional()
        .map_err(db_err)?;
    match version {
        None => {
            conn.execute(
                "INSERT INTO meta (k, v) VALUES ('schema', ?1)",
                params![SCHEMA_VERSION],
            )
            .map_err(db_err)?;
        }
        Some(SCHEMA_VERSION) => {}
        Some(other) => {
            return Err(db_err(format!(
                "unsupported schema version {other} (this build supports {SCHEMA_VERSION})"
            )));
        }
    }
    Ok(conn)
}

/// Resets an existing store, WAL or shared-memory file to `0600` if its mode was widened (PR #20
/// review). Symlinks are left alone; `workspace::verify` refuses those separately.
fn tighten_store_file_modes(dir: &Path) -> Result<(), IcxError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for suffix in ["", "-wal", "-shm"] {
            let p = dir.join(format!("{DB_FILE}{suffix}"));
            let Ok(meta) = std::fs::symlink_metadata(&p) else {
                continue;
            };
            if meta.file_type().is_file() && meta.permissions().mode() & 0o077 != 0 {
                std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600))
                    .map_err(db_err)?;
            }
        }
    }
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

fn meta_blob(conn: &Connection, key: &str) -> Result<Option<Vec<u8>>, IcxError> {
    conn.query_row("SELECT v FROM meta WHERE k = ?1", params![key], |r| {
        r.get(0)
    })
    .optional()
    .map_err(db_err)
}

fn meta_u64(conn: &Connection, key: &str) -> Result<Option<u64>, IcxError> {
    let v: Option<i64> = conn
        .query_row("SELECT v FROM meta WHERE k = ?1", params![key], |r| {
            r.get(0)
        })
        .optional()
        .map_err(db_err)?;
    Ok(v.map(|x| u64::try_from(x).unwrap_or(0)))
}

/// Current store generation (0 if the store has never been saved).
pub(crate) fn generation(conn: &Connection) -> Result<u64, IcxError> {
    Ok(meta_u64(conn, "generation")?.unwrap_or(0))
}

/// Reads the complete workspace in one read transaction.
pub(crate) fn load(conn: &mut Connection) -> Result<StoredWorkspace, IcxError> {
    let tx = conn.transaction().map_err(db_err)?;
    let mut out = StoredWorkspace {
        generation: generation(&tx)?,
        ..StoredWorkspace::default()
    };
    if let Some(cfg) = meta_blob(&tx, "chunk_config")? {
        out.chunk_config = Some(serde_json::from_slice(&cfg).map_err(db_err)?);
        out.chunk_config_raw = Some(cfg);
    }
    {
        let mut stmt = tx
            .prepare("SELECT id, digest, bytes FROM chunks ORDER BY id")
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Vec<u8>>(1)?,
                    r.get::<_, Vec<u8>>(2)?,
                ))
            })
            .map_err(db_err)?;
        for row in rows {
            let (id, digest, bytes) = row.map_err(db_err)?;
            let digest: [u8; 32] = digest
                .try_into()
                .map_err(|_| db_err("chunk digest is not 32 bytes"))?;
            out.chunks
                .push((id_from_sql(id), Blake3Digest(digest), bytes));
        }
    }
    {
        let mut stmt = tx
            .prepare("SELECT meta FROM artifacts ORDER BY id")
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(db_err)?;
        for row in rows {
            let json = row.map_err(db_err)?;
            out.artifacts
                .push(serde_json::from_str(&json).map_err(db_err)?);
        }
    }
    {
        let mut stmt = tx.prepare("SELECT digest FROM emitted").map_err(db_err)?;
        let rows = stmt
            .query_map([], |r| r.get::<_, Vec<u8>>(0))
            .map_err(db_err)?;
        for row in rows {
            let d: [u8; 32] = row
                .map_err(db_err)?
                .try_into()
                .map_err(|_| db_err("emitted digest is not 32 bytes"))?;
            out.emitted.insert(d);
        }
    }
    out.graph_json = meta_blob(&tx, "graph")?;
    out.mmu_state_json = meta_blob(&tx, "mmu_state")?;
    out.hdc_snapshot = meta_blob(&tx, "hdc_index")?;
    out.hdc_snapshot_events = meta_u64(&tx, "hdc_index_events")?;
    out.seal = meta_blob(&tx, "seal")?;
    tx.commit().map_err(db_err)?;
    // SQLite orders INTEGER ids as signed i64; the in-memory store orders them as u64.
    out.artifacts.sort_by_key(|a| a.id.0);
    Ok(out)
}

/// Ids of CAS rows already present in the store (tracked in memory so saves stay incremental).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PersistedIds {
    pub chunks: BTreeSet<ArtifactId>,
    pub artifacts: BTreeSet<ArtifactId>,
    pub emitted: BTreeSet<[u8; 32]>,
}

/// Writes new CAS rows (those not in `persisted`) plus all derived state in one transaction and
/// returns the new generation. `persisted` is updated only after a successful commit.
pub(crate) fn save<'c>(
    conn: &mut Connection,
    chunks: impl Iterator<Item = (ArtifactId, &'c Blake3Digest, &'c [u8])>,
    artifacts: impl Iterator<Item = &'c StoredArtifact>,
    persisted: &mut PersistedIds,
    derived: &DerivedState<'_>,
    expected_generation: u64,
) -> Result<u64, IcxError> {
    let tx = conn.transaction().map_err(db_err)?;
    // Optimistic concurrency (PR #20 review): the caller's state must describe the generation on
    // disk. Sealing a snapshot that predates another writer's commit would cover only the
    // caller's rows while the tables hold both, producing a store that cannot load.
    let on_disk = generation(&tx)?;
    if on_disk != expected_generation {
        return Err(db_err(format!(
            "store changed since it was loaded (generation {on_disk}, expected \
             {expected_generation}); reload before saving"
        )));
    }
    // Rows are append-only, so a table smaller than the cache means rows were removed behind
    // this process (restore, manual edit). Re-insert everything rather than seal a store that
    // can no longer load (PR #20 review).
    let count = |table: &str| -> Result<usize, IcxError> {
        let n: i64 = tx
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .map_err(db_err)?;
        Ok(usize::try_from(n).unwrap_or(0))
    };
    if count("chunks")? != persisted.chunks.len()
        || count("artifacts")? != persisted.artifacts.len()
        || count("emitted")? != persisted.emitted.len()
    {
        *persisted = PersistedIds::default();
    }
    let mut new_chunks = Vec::new();
    let mut new_artifacts = Vec::new();
    let mut new_emitted = Vec::new();
    let mut all_artifacts = Vec::new();
    {
        let mut ins_chunk = tx
            .prepare_cached("INSERT OR IGNORE INTO chunks (id, digest, bytes) VALUES (?1, ?2, ?3)")
            .map_err(db_err)?;
        for (id, digest, bytes) in chunks {
            if persisted.chunks.contains(&id) {
                continue;
            }
            ins_chunk
                .execute(params![id_to_sql(id), digest.0.as_slice(), bytes])
                .map_err(db_err)?;
            new_chunks.push(id);
        }
        let mut ins_art = tx
            .prepare_cached("INSERT OR IGNORE INTO artifacts (id, meta) VALUES (?1, ?2)")
            .map_err(db_err)?;
        for art in artifacts {
            all_artifacts.push((art.id, art.full_digest));
            if persisted.artifacts.contains(&art.id) {
                continue;
            }
            let json = serde_json::to_string(art).map_err(db_err)?;
            ins_art
                .execute(params![id_to_sql(art.id), json])
                .map_err(db_err)?;
            new_artifacts.push(art.id);
        }
        let mut ins_emitted = tx
            .prepare_cached("INSERT OR IGNORE INTO emitted (digest) VALUES (?1)")
            .map_err(db_err)?;
        for d in derived.emitted.difference(&persisted.emitted) {
            ins_emitted.execute(params![d.as_slice()]).map_err(db_err)?;
            new_emitted.push(*d);
        }
    }
    let next_gen = on_disk.saturating_add(1);
    let cfg = serde_json::to_vec(&derived.chunk_config).map_err(db_err)?;
    let mut upsert = tx
        .prepare_cached(
            "INSERT INTO meta (k, v) VALUES (?1, ?2) ON CONFLICT(k) DO UPDATE SET v = excluded.v",
        )
        .map_err(db_err)?;
    upsert
        .execute(params!["chunk_config", cfg])
        .map_err(db_err)?;
    upsert
        .execute(params!["graph", derived.graph_json])
        .map_err(db_err)?;
    upsert
        .execute(params!["mmu_state", derived.mmu_state_json])
        .map_err(db_err)?;
    upsert
        .execute(params!["hdc_index", derived.hdc_snapshot])
        .map_err(db_err)?;
    upsert
        .execute(params![
            "hdc_index_events",
            i64::try_from(derived.hdc_snapshot_events).unwrap_or(i64::MAX)
        ])
        .map_err(db_err)?;
    upsert
        .execute(params![
            "generation",
            i64::try_from(next_gen).unwrap_or(i64::MAX)
        ])
        .map_err(db_err)?;
    if let Some((key, dir)) = derived.seal {
        all_artifacts.sort_by_key(|(id, _)| id.0);
        let digest = state_digest(
            &cfg,
            derived.graph_json,
            derived.mmu_state_json,
            derived.hdc_snapshot,
            derived.hdc_snapshot_events,
            all_artifacts.into_iter(),
            derived.emitted,
        );
        upsert
            .execute(params![
                "seal",
                store_seal(key, dir, next_gen, &digest).as_slice()
            ])
            .map_err(db_err)?;
    }
    drop(upsert);
    tx.commit().map_err(db_err)?;
    persisted.chunks.extend(new_chunks);
    persisted.artifacts.extend(new_artifacts);
    persisted.emitted.extend(new_emitted);
    Ok(next_gen)
}
