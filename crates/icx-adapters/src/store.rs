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
}

/// Keyed MAC binding a store generation to its canonical workspace path.
pub(crate) fn store_seal(key: &[u8; 32], canonical_dir: &Path, generation: u64) -> [u8; 32] {
    let mut h = blake3::Hasher::new_keyed(key);
    h.update(b"ICX_STORE_SEAL_V1\0");
    h.update(canonical_dir.as_os_str().as_encoded_bytes());
    h.update(&generation.to_le_bytes());
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
         CREATE TABLE IF NOT EXISTS artifacts (id INTEGER PRIMARY KEY, meta TEXT NOT NULL);",
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
    out.graph_json = meta_blob(&tx, "graph")?;
    out.mmu_state_json = meta_blob(&tx, "mmu_state")?;
    out.hdc_snapshot = meta_blob(&tx, "hdc_index")?;
    out.hdc_snapshot_events = meta_u64(&tx, "hdc_index_events")?;
    out.seal = meta_blob(&tx, "seal")?;
    tx.commit().map_err(db_err)?;
    Ok(out)
}

/// Ids of CAS rows already present in the store (tracked in memory so saves stay incremental).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PersistedIds {
    pub chunks: BTreeSet<ArtifactId>,
    pub artifacts: BTreeSet<ArtifactId>,
}

/// Writes new CAS rows (those not in `persisted`) plus all derived state in one transaction and
/// returns the new generation. `persisted` is updated only after a successful commit.
pub(crate) fn save<'c>(
    conn: &mut Connection,
    chunks: impl Iterator<Item = (ArtifactId, &'c Blake3Digest, &'c [u8])>,
    artifacts: impl Iterator<Item = &'c StoredArtifact>,
    persisted: &mut PersistedIds,
    derived: &DerivedState<'_>,
) -> Result<u64, IcxError> {
    let tx = conn.transaction().map_err(db_err)?;
    let mut new_chunks = Vec::new();
    let mut new_artifacts = Vec::new();
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
            if persisted.artifacts.contains(&art.id) {
                continue;
            }
            let json = serde_json::to_string(art).map_err(db_err)?;
            ins_art
                .execute(params![id_to_sql(art.id), json])
                .map_err(db_err)?;
            new_artifacts.push(art.id);
        }
    }
    let next_gen = generation(&tx)?.saturating_add(1);
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
        upsert
            .execute(params!["seal", store_seal(key, dir, next_gen).as_slice()])
            .map_err(db_err)?;
    }
    drop(upsert);
    tx.commit().map_err(db_err)?;
    persisted.chunks.extend(new_chunks);
    persisted.artifacts.extend(new_artifacts);
    Ok(next_gen)
}
