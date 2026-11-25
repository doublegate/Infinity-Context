//! Bookmarked ingestion of Codex rollout files from `$CODEX_HOME/sessions` (T-62-003, T-62-004).
//!
//! Codex writes one `rollout-<ts>-<thread>[_<id>].jsonl` per thread and, behind its
//! `local_thread_store_compression` feature, recompresses files older than 7 days to
//! `.jsonl.zst` (one zstd frame). It decompresses a file again when it appends to it. A rollout is
//! therefore tracked by its *logical* path (without `.zst`) and by a hash of its first bytes, so
//! ingestion resumes at the same logical offset whichever form the file is in, and a replaced
//! file starts again from the beginning.
//!
//! Compressed input is decoded by a pure-Rust decoder. At end of stream the frame's content
//! checksum is compared when the frame carries one (Codex's frames do not), and bytes after the
//! frame are refused. The decompressed size is measured in a first pass and refused above
//! [`MAX_DECOMPRESSED_ROLLOUT_BYTES`] before anything is ingested, so a decompression bomb or a
//! frame that fails to decode ingests nothing.

use crate::codex::CodexRolloutTailer;
use crate::mmu::InfinityContextMmu;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Largest decompressed size accepted for one `.jsonl.zst` rollout (1 GiB). Codex caps each stored
/// tool output at 64 KiB, so a real rollout is far smaller.
pub const MAX_DECOMPRESSED_ROLLOUT_BYTES: u64 = 1 << 30;

/// Bytes at the start of a rollout hashed to recognise the same rollout across forms. The first
/// record (`session_meta`) carries the thread id and timestamp, so it is unique per rollout.
const PREFIX_HASH_BYTES: u64 = 64 * 1024;

/// Bound on the first record read to find the session's `cwd`.
const MAX_SESSION_META_BYTES: usize = 4 * 1024 * 1024;

/// File name of the bookmark state inside a workspace directory.
pub const ROLLOUT_STATE_FILE: &str = "codex-rollouts.json";

const STATE_VERSION: u32 = 1;

/// Why one rollout could not be ingested.
#[derive(Debug, Error)]
pub enum RolloutIngestError {
    #[error("not a regular file (symlinks and special files are refused)")]
    NotRegularFile,
    #[error("decompressed size exceeds {limit} bytes")]
    DecompressedTooLarge { limit: u64 },
    #[error("zstd decode failed: {0}")]
    Decode(String),
    #[error("{0}")]
    Io(#[from] io::Error),
}

/// Where ingestion of one logical rollout stopped.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutMark {
    /// Logical bytes (decompressed, for `.zst`) consumed through the last complete line.
    pub offset: u64,
    /// BLAKE3 (hex) of the first `min(offset, 64 KiB)` logical bytes.
    pub prefix_hash: String,
    /// Size of the source file when the mark was taken.
    pub source_len: u64,
    /// Modification time of the source file (nanoseconds since the epoch), when available.
    pub source_mtime_nanos: Option<u64>,
    /// Whether the source file was `.jsonl.zst`.
    pub compressed: bool,
    /// Whether tool-output records up to `offset` were ingested. A later run that asks for tool
    /// outputs reads a rollout again from the start when this is `false`. Marks written before
    /// this field existed always included tool outputs, so a missing field reads as `true`.
    #[serde(default = "tool_outputs_included_default")]
    pub tool_outputs_included: bool,
}

const fn tool_outputs_included_default() -> bool {
    true
}

/// Bookmarks for every rollout ingested into one workspace.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutIngestState {
    pub version: u32,
    /// Keyed by the logical rollout path (without `.zst`).
    pub rollouts: BTreeMap<String, RolloutMark>,
}

impl RolloutIngestState {
    /// Loads the state from `<ws_dir>/codex-rollouts.json`. A missing file is an empty state; an
    /// unreadable or unknown-version file is an error, so it is never silently overwritten.
    pub fn load(ws_dir: &Path) -> Result<Self, String> {
        let path = ws_dir.join(ROLLOUT_STATE_FILE);
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Ok(Self {
                    version: STATE_VERSION,
                    rollouts: BTreeMap::new(),
                });
            }
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        if !meta.file_type().is_file() {
            return Err(format!("{}: not a regular file", path.display()));
        }
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let state: Self =
            serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        if state.version != STATE_VERSION {
            return Err(format!(
                "{}: unsupported state version {}",
                path.display(),
                state.version
            ));
        }
        Ok(state)
    }

    /// Writes the state atomically, owner-only, to `<ws_dir>/codex-rollouts.json`.
    pub fn save(&self, ws_dir: &Path) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        crate::workspace::write_private_file(&ws_dir.join(ROLLOUT_STATE_FILE), &bytes)
            .map_err(|e| e.to_string())
    }
}

/// Returns the logical key of a rollout path: the path with a trailing `.zst` removed.
pub fn logical_key(path: &Path) -> String {
    let s = path.to_string_lossy();
    s.strip_suffix(".zst").unwrap_or(&s).to_owned()
}

/// Whether `name` is a Codex rollout file name (`rollout-*.jsonl` or `rollout-*.jsonl.zst`).
/// Codex writes these exact lower-case names, so the match is deliberately case-sensitive.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
pub fn is_rollout_file_name(name: &str) -> bool {
    name.starts_with("rollout-") && (name.ends_with(".jsonl") || name.ends_with(".jsonl.zst"))
}

/// Lists rollout files under `sessions_dir` (recursively, without following symlinks), sorted.
pub fn discover_rollouts(sessions_dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![sessions_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let ft = entry.file_type()?;
            if ft.is_dir() {
                stack.push(entry.path());
            } else if ft.is_file() && is_rollout_file_name(&entry.file_name().to_string_lossy()) {
                out.push(entry.path());
            }
        }
    }
    out.sort();
    Ok(out)
}

fn is_compressed(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "zst")
}

fn regular_file_meta(path: &Path) -> Result<std::fs::Metadata, RolloutIngestError> {
    let meta = std::fs::symlink_metadata(path)?;
    if !meta.file_type().is_file() {
        return Err(RolloutIngestError::NotRegularFile);
    }
    Ok(meta)
}

fn mtime_nanos(meta: &std::fs::Metadata) -> Option<u64> {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|d| u64::try_from(d.as_nanos()).ok())
}

fn zstd_reader(file: File) -> Result<impl Read, RolloutIngestError> {
    let decoder = ruzstd::decoding::StreamingDecoder::new(BufReader::new(file))
        .map_err(|e| RolloutIngestError::Decode(e.to_string()))?;
    Ok(VerifiedZstd {
        decoder,
        finished: false,
    })
}

/// A single-frame zstd stream that is checked when it ends. `ruzstd` 0.9.1 reads the frame's
/// XXH64 content checksum but never compares it with the one it computes, and stops after the
/// first frame. At end of stream this reader compares the two checksums whenever the frame
/// carries one, and refuses any bytes after the frame (Codex writes exactly one frame). Either
/// failure is an `InvalidData` error, so a corrupt file is refused before anything is ingested.
struct VerifiedZstd {
    decoder: ruzstd::decoding::StreamingDecoder<BufReader<File>, ruzstd::decoding::FrameDecoder>,
    finished: bool,
}

impl Read for VerifiedZstd {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.finished || buf.is_empty() {
            return self.decoder.read(buf);
        }
        let n = self.decoder.read(buf)?;
        if n == 0 {
            self.finished = true;
            let frame = &self.decoder.decoder;
            if let Some(expected) = frame.get_checksum_from_data()
                && frame.get_calculated_checksum() != Some(expected)
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "zstd frame checksum mismatch",
                ));
            }
            let mut extra = [0u8; 1];
            if self.decoder.get_mut().read(&mut extra)? != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unexpected data after the zstd frame",
                ));
            }
        }
        Ok(n)
    }
}

fn map_decode(e: io::Error) -> RolloutIngestError {
    if e.kind() == io::ErrorKind::UnexpectedEof || e.get_ref().is_some() {
        RolloutIngestError::Decode(e.to_string())
    } else {
        RolloutIngestError::Io(e)
    }
}

/// Opens the logical byte stream of a rollout: the file itself, or its decoded zstd frame.
fn open_logical(path: &Path) -> Result<Box<dyn Read>, RolloutIngestError> {
    let file = File::open(path)?;
    if is_compressed(path) {
        Ok(Box::new(zstd_reader(file)?))
    } else {
        Ok(Box::new(file))
    }
}

/// Decodes a `.jsonl.zst` once to measure it, refusing it above `limit` or on any decode error.
fn measure_decompressed(path: &Path, limit: u64) -> Result<u64, RolloutIngestError> {
    let reader = open_logical(path)?;
    let n = io::copy(&mut reader.take(limit + 1), &mut io::sink()).map_err(map_decode)?;
    if n > limit {
        return Err(RolloutIngestError::DecompressedTooLarge { limit });
    }
    Ok(n)
}

/// Hashes the first `min(len, 64 KiB)` bytes of `reader`, returning `(hash_hex, bytes_hashed)`.
fn hash_prefix<R: Read>(reader: &mut R, len: u64) -> io::Result<(String, u64)> {
    let mut hasher = blake3::Hasher::new();
    let n = io::copy(&mut reader.take(len.min(PREFIX_HASH_BYTES)), &mut hasher)?;
    Ok((hasher.finalize().to_hex().to_string(), n))
}

/// Reads the working directory recorded in a rollout's first `session_meta` record.
pub fn session_cwd(path: &Path) -> Option<PathBuf> {
    regular_file_meta(path).ok()?;
    // Bound the decoded bytes, not only the retained ones: selection runs before the
    // decompressed-size check, so a newline-free bomb must not be decoded in full here.
    let reader = open_logical(path)
        .ok()?
        .take(MAX_SESSION_META_BYTES as u64 + 1);
    let mut reader = BufReader::new(reader);
    let mut line = Vec::new();
    let (_, ended, truncated) = CodexRolloutTailer::read_bounded_jsonl_record(
        &mut reader,
        MAX_SESSION_META_BYTES,
        &mut line,
    )
    .ok()?;
    if !ended || truncated {
        return None;
    }
    let val: serde_json::Value = serde_json::from_slice(&line).ok()?;
    if val.get("type").and_then(serde_json::Value::as_str) != Some("session_meta") {
        return None;
    }
    val.get("payload")?.get("cwd")?.as_str().map(PathBuf::from)
}

/// Whether the rollout's recorded `cwd` belongs to `project` (an absolute, canonical project
/// root): the recorded path lies beneath it as written or once symlinks are resolved, or it is
/// a linked worktree whose main repository is `project`.
pub fn rollout_belongs_to(path: &Path, project: &Path) -> bool {
    session_cwd(path).is_some_and(|cwd| cwd_belongs_to(&cwd, project))
}

fn cwd_belongs_to(cwd: &Path, project: &Path) -> bool {
    if cwd.starts_with(project) {
        return true;
    }
    // A directory that no longer exists can only match as written.
    let Ok(real) = std::fs::canonicalize(cwd) else {
        return false;
    };
    real.starts_with(project) || crate::workspace::project_root(&real) == project
}

/// Lists the rollouts under `sessions_dir` to ingest for `project` (every rollout when `project`
/// is `None`). When a rollout exists in both forms (`X.jsonl` and `X.jsonl.zst`, as after an
/// interrupted Codex recompression), only the plain file, which Codex appends to, is kept.
pub fn select_rollouts(sessions_dir: &Path, project: Option<&Path>) -> io::Result<Vec<PathBuf>> {
    let all = discover_rollouts(sessions_dir)?;
    let plain: std::collections::BTreeSet<String> = all
        .iter()
        .filter(|p| !is_compressed(p))
        .map(|p| logical_key(p))
        .collect();
    Ok(all
        .into_iter()
        .filter(|p| !(is_compressed(p) && plain.contains(&logical_key(p))))
        .filter(|p| project.is_none_or(|project| rollout_belongs_to(p, project)))
        .collect())
}

/// Ingests the records of one rollout that `prior` has not covered, returning the new mark and
/// the number of records ingested.
///
/// - An unchanged source (same form, size and mtime as `prior`) is skipped without being read.
/// - A `prior` mark resumes only if its prefix hash still matches the logical stream; otherwise
///   the rollout is ingested from the start.
/// - Ingestion stops before an incomplete trailing line, which a later call picks up.
/// - A record whose redacted output the store already holds byte for byte is skipped, so recall
///   does not index it twice. This catches repeats within rollouts. A hook's copy of the same
///   tool output usually differs in bytes (the rollout keeps what the model was shown), so it is
///   rarely matched.
pub fn ingest_rollout(
    path: &Path,
    prior: Option<&RolloutMark>,
    mmu: &mut InfinityContextMmu,
) -> Result<(RolloutMark, usize), RolloutIngestError> {
    ingest_rollout_with_limit(path, prior, mmu, MAX_DECOMPRESSED_ROLLOUT_BYTES)
}

/// [`ingest_rollout`] with an explicit decompressed-size limit (for tests).
pub fn ingest_rollout_with_limit(
    path: &Path,
    prior: Option<&RolloutMark>,
    mmu: &mut InfinityContextMmu,
    decompressed_limit: u64,
) -> Result<(RolloutMark, usize), RolloutIngestError> {
    let opts = IngestOptions {
        decompressed_limit,
        ..IngestOptions::default()
    };
    ingest_rollout_counted(path, prior, mmu, opts).map(|(mark, counts)| (mark, counts.ingested))
}

/// How [`ingest_rollout_counted`] reads a rollout.
#[derive(Clone, Copy, Debug)]
pub struct IngestOptions {
    /// Largest decompressed size accepted for a `.jsonl.zst` rollout.
    pub decompressed_limit: u64,
    /// Ingest tool-output records (`*_call_output`). `icx codex-ingest` turns this off by
    /// default: icx's Codex hook captures each tool output when the tool runs, and its copy never
    /// matches the rollout's, so both would be indexed.
    pub include_tool_outputs: bool,
}

impl Default for IngestOptions {
    fn default() -> Self {
        Self {
            decompressed_limit: MAX_DECOMPRESSED_ROLLOUT_BYTES,
            include_tool_outputs: true,
        }
    }
}

/// What [`ingest_rollout_counted`] did with one rollout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RolloutCounts {
    pub ingested: usize,
    /// Records skipped because the store already held them byte for byte.
    pub held: usize,
    /// Tool-output records left out (`include_tool_outputs: false`).
    pub tool_outputs: usize,
}

/// [`ingest_rollout_with_limit`] with explicit options, also returning what was skipped.
///
/// A rollout first read without tool outputs is read again from the start when tool outputs are
/// later requested (a backfill). Records already stored are then skipped as held, so only the
/// tool outputs are new.
pub fn ingest_rollout_counted(
    path: &Path,
    prior: Option<&RolloutMark>,
    mmu: &mut InfinityContextMmu,
    opts: IngestOptions,
) -> Result<(RolloutMark, RolloutCounts), RolloutIngestError> {
    let decompressed_limit = opts.decompressed_limit;
    let meta = regular_file_meta(path)?;
    let compressed = is_compressed(path);
    let source_len = meta.len();
    let source_mtime_nanos = mtime_nanos(&meta);
    let backfill = opts.include_tool_outputs && prior.is_some_and(|p| !p.tool_outputs_included);
    if let Some(p) = prior
        && !backfill
        && p.compressed == compressed
        && p.source_len == source_len
        && p.source_mtime_nanos == source_mtime_nanos
    {
        return Ok((p.clone(), RolloutCounts::default()));
    }
    let prior = if backfill { None } else { prior };

    let logical_len = if compressed {
        measure_decompressed(path, decompressed_limit)?
    } else {
        source_len
    };

    // Decide where to resume: the prior offset, if the same rollout still starts the same way
    // and the offset still lands just after a newline.
    let resume = match prior {
        Some(p) if p.offset > 0 && p.offset <= logical_len => {
            let mut reader = open_logical(path)?;
            let (hash, _) = hash_prefix(&mut reader, p.offset).map_err(map_decode)?;
            if hash == p.prefix_hash && byte_before_is_newline(path, p.offset)? {
                p.offset
            } else {
                0
            }
        }
        _ => 0,
    };

    let mut reader = BufReader::new(open_logical(path)?);
    if compressed {
        io::copy(&mut (&mut reader).take(resume), &mut io::sink()).map_err(map_decode)?;
    } else {
        // Plain files seek directly instead of re-reading the consumed bytes.
        let mut file = File::open(path)?;
        file.seek(SeekFrom::Start(resume))?;
        reader = BufReader::new(Box::new(file) as Box<dyn Read>);
    }
    let filter = crate::codex::RecordFilter {
        skip_held: true,
        skip_tool_outputs: !opts.include_tool_outputs,
    };
    let counts =
        CodexRolloutTailer::ingest_jsonl_records(&mut reader, mmu, filter).map_err(map_decode)?;
    let offset = resume + counts.consumed;

    let mut prefix_reader = open_logical(path)?;
    let (prefix_hash, _) = hash_prefix(&mut prefix_reader, offset).map_err(map_decode)?;
    // Tool outputs are covered only if this pass read them and everything before it did too.
    let tool_outputs_included = opts.include_tool_outputs
        && (resume == 0 || prior.is_some_and(|p| p.tool_outputs_included));
    Ok((
        RolloutMark {
            offset,
            prefix_hash,
            source_len,
            source_mtime_nanos,
            compressed,
            tool_outputs_included,
        },
        RolloutCounts {
            ingested: counts.ingested,
            held: counts.held,
            tool_outputs: counts.tool_outputs,
        },
    ))
}

/// Whether logical byte `offset - 1` of the rollout is `\n`.
fn byte_before_is_newline(path: &Path, offset: u64) -> Result<bool, RolloutIngestError> {
    let mut reader = open_logical(path)?;
    if is_compressed(path) {
        io::copy(&mut (&mut reader).take(offset - 1), &mut io::sink()).map_err(map_decode)?;
    } else {
        let mut file = File::open(path)?;
        file.seek(SeekFrom::Start(offset - 1))?;
        reader = Box::new(file);
    }
    let mut b = [0u8; 1];
    reader.read_exact(&mut b).map_err(map_decode)?;
    Ok(b[0] == b'\n')
}

/// Totals from one ingestion pass over a sessions directory.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IngestSummary {
    /// Rollouts that belong to the project.
    pub matched: usize,
    /// Rollouts with new records.
    pub updated: usize,
    /// Records ingested.
    pub records: usize,
    /// Records skipped because the store already held the same output byte for byte.
    pub already_held: usize,
    /// Tool-output records left to the Codex hook (`include_tool_outputs: false`).
    pub tool_outputs_left: usize,
    /// Rollouts refused, with the reason.
    pub refused: Vec<(PathBuf, String)>,
}

/// Ingests every rollout under `sessions_dir` whose session `cwd` is within `project` (or every
/// rollout when `project` is `None`), updating `state` in place.
pub fn ingest_sessions_dir(
    sessions_dir: &Path,
    project: Option<&Path>,
    state: &mut RolloutIngestState,
    mmu: &mut InfinityContextMmu,
    opts: IngestOptions,
) -> io::Result<IngestSummary> {
    let mut summary = IngestSummary::default();
    for path in select_rollouts(sessions_dir, project)? {
        ingest_selected(&path, state, mmu, &mut summary, opts);
    }
    Ok(summary)
}

/// Ingests one rollout chosen by [`select_rollouts`], updating its bookmark in `state` and the
/// totals in `summary`. Lets a caller ingest in batches and release the workspace lock between
/// them.
pub fn ingest_selected(
    path: &Path,
    state: &mut RolloutIngestState,
    mmu: &mut InfinityContextMmu,
    summary: &mut IngestSummary,
    opts: IngestOptions,
) {
    summary.matched += 1;
    let key = logical_key(path);
    match ingest_rollout_counted(path, state.rollouts.get(&key), mmu, opts) {
        Ok((mark, counts)) => {
            if counts.ingested > 0 {
                summary.updated += 1;
                summary.records += counts.ingested;
            }
            summary.already_held += counts.held;
            summary.tool_outputs_left += counts.tool_outputs;
            state.rollouts.insert(key, mark);
        }
        Err(e) => summary.refused.push((path.to_path_buf(), e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "icx-codex-ingest-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn meta_line(cwd: &str) -> String {
        serde_json::json!({
            "timestamp": "2026-11-22T00:00:00Z",
            "type": "session_meta",
            "payload": {"id": "t1", "cwd": cwd}
        })
        .to_string()
    }

    fn output_line(call: &str, text: &str) -> String {
        serde_json::json!({
            "timestamp": "2026-11-22T00:00:01Z",
            "type": "response_item",
            "payload": {"type": "function_call_output", "call_id": call, "output": text}
        })
        .to_string()
    }

    fn rollout_text(cwd: &str, outputs: &[(&str, &str)]) -> String {
        let mut s = meta_line(cwd);
        s.push('\n');
        for (c, t) in outputs {
            s.push_str(&output_line(c, t));
            s.push('\n');
        }
        s
    }

    /// Encodes `data` as a single zstd frame of raw (uncompressed) blocks with no checksum. The
    /// decoder under test does not need a compressor, so the tests build valid frames by hand.
    fn zstd_raw_frame(data: &[u8]) -> Vec<u8> {
        let mut out = vec![0x28, 0xB5, 0x2F, 0xFD];
        // Frame header descriptor: single segment, 8-byte content size, no checksum.
        out.push(0b1110_0000);
        out.extend_from_slice(&(data.len() as u64).to_le_bytes());
        let chunks: Vec<&[u8]> = if data.is_empty() {
            vec![&[][..]]
        } else {
            data.chunks(64 * 1024).collect()
        };
        for (i, chunk) in chunks.iter().enumerate() {
            let last = u32::from(i + 1 == chunks.len());
            // Block header: last flag, block type 0 (raw), 21-bit size.
            let header = last | ((chunk.len() as u32) << 3);
            out.extend_from_slice(&header.to_le_bytes()[..3]);
            out.extend_from_slice(chunk);
        }
        out
    }

    fn write(path: &Path, bytes: &[u8]) {
        let mut f = File::create(path).unwrap();
        f.write_all(bytes).unwrap();
    }

    fn mmu() -> InfinityContextMmu {
        InfinityContextMmu::default()
    }

    #[test]
    fn plain_rollout_resumes_after_append_and_skips_partial_line() {
        let d = tmpdir("plain");
        let p = d.join("rollout-a.jsonl");
        let mut text = rollout_text("/proj", &[("c1", "first output")]);
        text.push_str("{\"partial\":"); // still being written
        write(&p, text.as_bytes());
        let mut m = mmu();
        let (mark, n) = ingest_rollout(&p, None, &mut m).unwrap();
        assert_eq!(
            n, 1,
            "the session_meta line has no body; one output is ingested"
        );
        assert!(
            mark.offset < text.len() as u64,
            "the partial line is not consumed"
        );

        let mut text2 = rollout_text("/proj", &[("c1", "first output"), ("c2", "second")]);
        text2.push_str(&output_line("c3", "third"));
        text2.push('\n');
        write(&p, text2.as_bytes());
        let (mark2, n2) = ingest_rollout(&p, Some(&mark), &mut m).unwrap();
        assert_eq!(n2, 2, "only the two new outputs are ingested");
        assert_eq!(mark2.offset, text2.len() as u64);

        let (mark3, n3) = ingest_rollout(&p, Some(&mark2), &mut m).unwrap();
        assert_eq!(n3, 0, "an unchanged file ingests nothing");
        assert_eq!(mark3, mark2);
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn compressed_rollout_is_decoded_and_resumes_across_forms() {
        let d = tmpdir("zst");
        let plain = d.join("rollout-b.jsonl");
        let zst = d.join("rollout-b.jsonl.zst");
        let first = rollout_text("/proj", &[("c1", "alpha"), ("c2", "beta")]);
        write(&plain, first.as_bytes());
        let mut m = mmu();
        let (mark, n) = ingest_rollout(&plain, None, &mut m).unwrap();
        assert_eq!(n, 2);

        // Codex recompresses the file: same logical bytes, now as .zst.
        std::fs::remove_file(&plain).unwrap();
        write(&zst, &zstd_raw_frame(first.as_bytes()));
        assert_eq!(logical_key(&zst), logical_key(&plain));
        let (zmark, zn) = ingest_rollout(&zst, Some(&mark), &mut m).unwrap();
        assert_eq!(zn, 0, "no record is ingested twice after recompression");
        assert!(zmark.compressed);
        assert_eq!(zmark.offset, mark.offset);

        // Codex decompresses on append: the plain file returns, longer.
        std::fs::remove_file(&zst).unwrap();
        let mut longer = first.clone();
        longer.push_str(&output_line("c3", "gamma"));
        longer.push('\n');
        write(&plain, longer.as_bytes());
        let (pmark, pn) = ingest_rollout(&plain, Some(&zmark), &mut m).unwrap();
        assert_eq!(pn, 1, "only the appended record is ingested");
        assert_eq!(pmark.offset, longer.len() as u64);
        assert_eq!(
            session_cwd(&plain).as_deref(),
            Some(Path::new("/proj")),
            "session cwd read from the first record"
        );
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn compressed_rollout_from_scratch_ingests_every_record() {
        let d = tmpdir("zst-new");
        let zst = d.join("rollout-c.jsonl.zst");
        let text = rollout_text(
            "/proj/sub",
            &[("c1", "one"), ("c2", "two"), ("c3", "three")],
        );
        write(&zst, &zstd_raw_frame(text.as_bytes()));
        let mut m = mmu();
        let (mark, n) = ingest_rollout(&zst, None, &mut m).unwrap();
        assert_eq!(n, 3);
        assert_eq!(mark.offset, text.len() as u64);
        assert!(rollout_belongs_to(&zst, Path::new("/proj")));
        assert!(!rollout_belongs_to(&zst, Path::new("/other")));
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn oversized_decompression_is_refused_before_ingesting() {
        let d = tmpdir("bomb");
        let zst = d.join("rollout-d.jsonl.zst");
        let big = "x".repeat(10_000);
        let text = rollout_text("/proj", &[("c1", big.as_str())]);
        write(&zst, &zstd_raw_frame(text.as_bytes()));
        let mut m = mmu();
        let err = ingest_rollout_with_limit(&zst, None, &mut m, 1_000).unwrap_err();
        assert!(matches!(
            err,
            RolloutIngestError::DecompressedTooLarge { limit: 1_000 }
        ));
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn truncated_and_corrupt_frames_are_refused() {
        let d = tmpdir("corrupt");
        let text = rollout_text("/proj", &[("c1", "payload")]);
        let frame = zstd_raw_frame(text.as_bytes());
        let mut m = mmu();
        // Every truncation of a valid frame, and a bad magic number, must error and never panic.
        for cut in [1usize, 4, 5, 13, 16, frame.len() / 2, frame.len() - 1] {
            let p = d.join(format!("rollout-t{cut}.jsonl.zst"));
            write(&p, &frame[..cut]);
            assert!(
                ingest_rollout(&p, None, &mut m).is_err(),
                "truncated at {cut} bytes must be refused"
            );
        }
        let bad = d.join("rollout-bad.jsonl.zst");
        let mut wrong = frame.clone();
        wrong[0] ^= 0xFF;
        write(&bad, &wrong);
        assert!(ingest_rollout(&bad, None, &mut m).is_err());
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn when_both_forms_exist_only_the_plain_rollout_is_selected() {
        let d = tmpdir("both-forms");
        let text = rollout_text("/proj", &[("c1", "one")]);
        write(&d.join("rollout-h.jsonl"), text.as_bytes());
        write(
            &d.join("rollout-h.jsonl.zst"),
            &zstd_raw_frame(text.as_bytes()),
        );
        write(
            &d.join("rollout-i.jsonl.zst"),
            &zstd_raw_frame(text.as_bytes()),
        );
        let names: Vec<String> = select_rollouts(&d, None)
            .unwrap()
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["rollout-h.jsonl", "rollout-i.jsonl.zst"]);
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn session_selection_decodes_a_bounded_prefix_only() {
        // A compressed rollout whose first line never ends: selection must give up after the
        // bounded prefix instead of decoding the whole stream.
        let d = tmpdir("meta-bomb");
        let p = d.join("rollout-j.jsonl.zst");
        let huge = vec![b'x'; MAX_SESSION_META_BYTES * 3];
        write(&p, &zstd_raw_frame(&huge));
        assert_eq!(session_cwd(&p), None);
        assert!(!rollout_belongs_to(&p, Path::new("/proj")));
        std::fs::remove_dir_all(d).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_and_linked_worktree_cwds_match_their_project() {
        let d = tmpdir("cwd-match");
        let main = d.join("main");
        std::fs::create_dir_all(&main).unwrap();
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .args(args)
                .current_dir(&main)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        };
        let have_git = git(&["init", "-q"])
            && git(&[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "init",
            ]);
        let project = std::fs::canonicalize(&main).unwrap();

        // A symlink to the project: Codex may record the logical path.
        let link = d.join("link");
        std::os::unix::fs::symlink(&main, &link).unwrap();
        assert!(
            cwd_belongs_to(&link, &project),
            "a symlinked cwd resolves to the project"
        );

        if have_git {
            let wt = d.join("wt");
            assert!(git(&["worktree", "add", "-q", wt.to_str().unwrap()]));
            assert!(
                cwd_belongs_to(&wt, &project),
                "a linked worktree outside the main repository belongs to it"
            );
        }
        assert!(!cwd_belongs_to(&d.join("elsewhere"), &project));
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn output_already_ingested_by_the_hook_is_not_indexed_twice() {
        let d = tmpdir("dedup");
        let p = d.join("rollout-g.jsonl");
        let output = "cargo test output\ntest a ... ok\ntest b ... ok\n";
        let mut m = mmu();
        // The Codex PostToolUse hook ingests the output when the tool runs.
        let hook = serde_json::json!({
            "tool_name": "Bash",
            "tool_input": {"command": "cargo test"},
            "tool_response": output,
        });
        crate::codex::CodexAdapter::handle_hook("PostToolUse", &hook.to_string(), &mut m).unwrap();
        let hdc_before = m.hdc_index.len();
        // The same output then appears in the rollout, next to one the hook never saw.
        write(
            &p,
            rollout_text("/proj", &[("c1", output), ("c2", "only in the rollout")]).as_bytes(),
        );
        let (_, counts) =
            ingest_rollout_counted(&p, None, &mut m, IngestOptions::default()).unwrap();
        assert_eq!(
            (counts.ingested, counts.held),
            (1, 1),
            "an exact copy is skipped, the new one ingested"
        );
        assert_eq!(
            m.hdc_index.len(),
            hdc_before + 1,
            "recall gains one entry, not two"
        );
        std::fs::remove_dir_all(d).unwrap();
    }

    fn message_line(text: &str) -> String {
        serde_json::json!({
            "timestamp": "2026-11-22T00:00:02Z",
            "type": "response_item",
            "payload": {"type": "message", "role": "assistant", "content": text}
        })
        .to_string()
    }

    #[test]
    fn tool_outputs_can_be_left_to_the_hook_and_backfilled_later() {
        let d = tmpdir("tool-skip");
        let p = d.join("rollout-k.jsonl");
        let mut text = rollout_text(
            "/proj",
            &[("c1", "tool output one"), ("c2", "tool output two")],
        );
        text.push_str(&message_line("the assistant's reply"));
        text.push('\n');
        write(&p, text.as_bytes());
        let mut m = mmu();
        let skip = IngestOptions {
            include_tool_outputs: false,
            ..IngestOptions::default()
        };
        let (mark, counts) = ingest_rollout_counted(&p, None, &mut m, skip).unwrap();
        assert_eq!(
            (counts.ingested, counts.tool_outputs),
            (1, 2),
            "only the message"
        );
        assert!(!mark.tool_outputs_included);
        assert_eq!(mark.offset, text.len() as u64, "the whole file is consumed");

        // Unchanged file, same options: nothing to do.
        let (same, again) = ingest_rollout_counted(&p, Some(&mark), &mut m, skip).unwrap();
        assert_eq!(again, RolloutCounts::default());

        // A backfill reads the file again; the message is already held, the tool outputs are new.
        let (full, back) =
            ingest_rollout_counted(&p, Some(&same), &mut m, IngestOptions::default()).unwrap();
        assert_eq!((back.ingested, back.held, back.tool_outputs), (2, 1, 0));
        assert!(full.tool_outputs_included);
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn marks_written_before_the_tool_output_field_count_as_complete() {
        let old = r#"{"offset":10,"prefix_hash":"ab","source_len":10,"source_mtime_nanos":null,"compressed":false}"#;
        let mark: RolloutMark = serde_json::from_str(old).unwrap();
        assert!(mark.tool_outputs_included);
    }

    #[test]
    fn replaced_rollout_restarts_from_the_beginning() {
        let d = tmpdir("replaced");
        let p = d.join("rollout-e.jsonl");
        write(&p, rollout_text("/proj", &[("c1", "old")]).as_bytes());
        let mut m = mmu();
        let (mark, _) = ingest_rollout(&p, None, &mut m).unwrap();
        // A different rollout of the same name and a longer length: the prefix hash differs.
        let replacement = rollout_text("/elsewhere", &[("c9", "new one"), ("c8", "and more")]);
        write(&p, replacement.as_bytes());
        let (_, n) = ingest_rollout(&p, Some(&mark), &mut m).unwrap();
        assert_eq!(n, 2, "a replaced file is ingested in full");
        std::fs::remove_dir_all(d).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_rollout_is_refused() {
        let d = tmpdir("symlink");
        let target = d.join("target.jsonl");
        write(&target, rollout_text("/proj", &[("c1", "x")]).as_bytes());
        let link = d.join("rollout-f.jsonl");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let mut m = mmu();
        assert!(matches!(
            ingest_rollout(&link, None, &mut m),
            Err(RolloutIngestError::NotRegularFile)
        ));
        // Discovery does not list it either.
        assert_eq!(discover_rollouts(&d).unwrap(), Vec::<PathBuf>::new());
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn sessions_dir_ingest_filters_by_project_and_round_trips_state() {
        let d = tmpdir("dir");
        let sessions = d.join("sessions/2026/11/22");
        std::fs::create_dir_all(&sessions).unwrap();
        write(
            &sessions.join("rollout-mine.jsonl"),
            rollout_text("/proj/app", &[("c1", "mine")]).as_bytes(),
        );
        write(
            &sessions.join("rollout-theirs.jsonl.zst"),
            &zstd_raw_frame(rollout_text("/other", &[("c2", "theirs")]).as_bytes()),
        );
        write(&sessions.join("notes.txt"), b"not a rollout");
        let ws = d.join("ws");
        std::fs::create_dir_all(&ws).unwrap();

        let mut state = RolloutIngestState::load(&ws).unwrap();
        let mut m = mmu();
        let s = ingest_sessions_dir(
            &d.join("sessions"),
            Some(Path::new("/proj")),
            &mut state,
            &mut m,
            IngestOptions::default(),
        )
        .unwrap();
        assert_eq!((s.matched, s.updated, s.records), (1, 1, 1));
        assert_eq!(s.refused, Vec::new());
        state.save(&ws).unwrap();

        let mut reloaded = RolloutIngestState::load(&ws).unwrap();
        assert_eq!(reloaded, state);
        let again = ingest_sessions_dir(
            &d.join("sessions"),
            Some(Path::new("/proj")),
            &mut reloaded,
            &mut m,
            IngestOptions::default(),
        )
        .unwrap();
        assert_eq!(again.records, 0, "a second pass ingests nothing new");

        let all = ingest_sessions_dir(
            &d.join("sessions"),
            None,
            &mut reloaded,
            &mut m,
            IngestOptions::default(),
        )
        .unwrap();
        assert_eq!(
            (all.matched, all.records),
            (2, 1),
            "--all-projects adds the other rollout"
        );
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn state_file_with_unknown_version_is_refused() {
        let d = tmpdir("state");
        write(
            &d.join(ROLLOUT_STATE_FILE),
            br#"{"version":99,"rollouts":{}}"#,
        );
        assert!(RolloutIngestState::load(&d).is_err());
        write(&d.join(ROLLOUT_STATE_FILE), b"not json");
        assert!(RolloutIngestState::load(&d).is_err());
        std::fs::remove_dir_all(d).unwrap();
    }
}
