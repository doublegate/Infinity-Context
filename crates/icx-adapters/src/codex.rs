//! OpenAI Codex CLI (`0.169.0+`) native lifecycle hook adapter (`CodexAdapter`).
//!
//! Strictly conforms to Codex's embedded `additionalProperties: false` JSON schemas
//! (`session-start.command.output`, `user-prompt-submit.command.output`,
//! `post-tool-use.command.output`, `pre-compact.command.output`).

use crate::mmu::{ICX_AHTL_LEGEND, InfinityContextMmu};
use crate::{AdapterError, AgentHookAdapter};
use icx_core::AgentKind;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Native hook adapter for OpenAI Codex CLI (`0.169.0+`).
#[derive(Clone, Debug, Default)]
pub struct CodexAdapter;

impl AgentHookAdapter for CodexAdapter {
    fn agent_kind(&self) -> AgentKind {
        AgentKind::Codex
    }

    fn handle_hook_event(
        &self,
        event_name: &str,
        stdin_json: &str,
        mmu: &mut InfinityContextMmu,
    ) -> Result<String, AdapterError> {
        Ok(Self::handle_hook(event_name, stdin_json, mmu)?)
    }
}

impl CodexAdapter {
    /// Handles a Codex lifecycle hook event JSON payload and emits schema-validated output JSON.
    pub fn handle_hook(
        event_name: &str,
        stdin_json: &str,
        mmu: &mut InfinityContextMmu,
    ) -> Result<String, serde_json::Error> {
        let payload: Value = if stdin_json.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(stdin_json)?
        };

        let response = match event_name {
            "SessionStart" | "session-start" => {
                let tail = mmu.build_ephemeral_tail_slot(AgentKind::Codex, "session_start", 3);
                json!({
                    "continue": true,
                    "hookSpecificOutput": {
                        "hookEventName": "SessionStart",
                        "additionalContext": format!("{ICX_AHTL_LEGEND}\n{tail}")
                    },
                    "stopReason": null,
                    "suppressOutput": false,
                    "systemMessage": null
                })
            }
            "UserPromptSubmit" | "user-prompt-submit" => {
                let prompt = payload
                    .get("prompt")
                    .and_then(Value::as_str)
                    .unwrap_or("codex_query");
                let tail = mmu.build_ephemeral_tail_slot(AgentKind::Codex, prompt, 4);
                json!({
                    "continue": true,
                    "hookSpecificOutput": {
                        "hookEventName": "UserPromptSubmit",
                        "additionalContext": tail
                    },
                    "stopReason": null,
                    "suppressOutput": false,
                    "systemMessage": null
                })
            }
            "PreToolUse" | "pre-tool-use" => {
                // ICX-AUD-009: no stdin echo; icx makes no PreToolUse decision.
                json!({
                    "continue": true,
                    "hookSpecificOutput": { "hookEventName": "PreToolUse" },
                    "stopReason": null,
                    "suppressOutput": false,
                    "systemMessage": null
                })
            }
            "PostToolUse" | "post-tool-use" => {
                let tool_name = payload
                    .get("tool_name")
                    .or_else(|| payload.get("toolName"))
                    .and_then(Value::as_str)
                    .unwrap_or("codex_tool");
                if InfinityContextMmu::is_icx_pager_tool(tool_name) {
                    json!({
                        "continue": true,
                        "reason": null,
                        "stopReason": null,
                        "suppressOutput": false,
                        "systemMessage": null
                    })
                } else {
                    let subject = InfinityContextMmu::extract_hook_subject(&payload, "codex_tool");
                    let raw_output = match payload.get("tool_response").filter(|v| !v.is_null()) {
                        Some(Value::String(s)) => s.clone(),
                        Some(other) => other.to_string(),
                        None => String::new(),
                    };
                    let exit_code = InfinityContextMmu::extract_exit_code(&payload);
                    let explicit_sub_id = ["subagent_id", "agent_id"]
                        .into_iter()
                        .find_map(|k| payload.get(k).and_then(Value::as_str))
                        .map(str::to_owned);
                    let res = mmu.ingest_payload_for_subagent(
                        AgentKind::Codex,
                        &subject,
                        &raw_output,
                        exit_code,
                        false,
                        explicit_sub_id.as_deref(),
                    );
                    let hso = if tool_name.starts_with("mcp__") && res.was_virtualized {
                        let total_lines = mmu
                            .cas
                            .get_meta(res.artifact_id)
                            .map_or(1, |m| m.total_lines.max(1));
                        let stub = format!(
                            "⟪ICX:STUB│{}│raw={}B→{}B│lines=1..{total_lines}⟫\n{}",
                            res.artifact_id,
                            res.raw_bytes,
                            res.compressed_ahtl.len(),
                            res.compressed_ahtl
                        );
                        mmu.record_emitted(&stub);
                        json!({
                            "hookEventName": "PostToolUse",
                            "updatedMCPToolOutput": stub
                        })
                    } else {
                        json!({
                            "hookEventName": "PostToolUse",
                            "additionalContext": res.compressed_ahtl
                        })
                    };

                    json!({
                        "continue": true,
                        "hookSpecificOutput": hso,
                        "reason": null,
                        "stopReason": null,
                        "suppressOutput": false,
                        "systemMessage": null
                    })
                }
            }
            "SubagentStart" | "subagent-start" => {
                let raw_subagent_id = ["subagent_id", "agent_id"]
                    .into_iter()
                    .find_map(|k| payload.get(k).and_then(Value::as_str))
                    .unwrap_or("codex_subagent");
                let subagent_id = mmu.begin_subagent(raw_subagent_id);
                let tail = mmu.build_ephemeral_tail_slot(AgentKind::Codex, &subagent_id, 3);
                json!({
                    "continue": true,
                    "hookSpecificOutput": {
                        "hookEventName": "SubagentStart",
                        "additionalContext": tail
                    },
                    "stopReason": null,
                    "suppressOutput": false,
                    "systemMessage": null
                })
            }
            "SubagentStop" | "subagent-stop" => {
                let subagent_id = ["subagent_id", "agent_id"]
                    .into_iter()
                    .find_map(|k| payload.get(k).and_then(Value::as_str));
                let summary = ["summary", "outcome"]
                    .into_iter()
                    .find_map(|k| payload.get(k).and_then(Value::as_str))
                    .unwrap_or("subagent_completed");
                let folded_msg = mmu.finish_subagent(AgentKind::Codex, subagent_id, summary);
                json!({
                    "continue": true,
                    "stopReason": null,
                    "suppressOutput": false,
                    "systemMessage": folded_msg
                })
            }
            "PreCompact" | "pre-compact" | "PostCompact" | "post-compact" => {
                let _ = mmu.rollover_and_sync_dag_root();
                let win = mmu.assemble_prompt_window(AgentKind::Codex, "compact_checkpoint");
                json!({
                    "continue": true,
                    "stopReason": null,
                    "suppressOutput": false,
                    "systemMessage": win.deterministic_prefix()
                })
            }
            "SessionEnd" | "session-end" => {
                mmu.fold_all_open_subagents(AgentKind::Codex, "session_end");
                let _ = mmu.rollover_and_sync_dag_root();
                let win = mmu.assemble_prompt_window(AgentKind::Codex, "session_end");
                json!({
                    "continue": true,
                    "stopReason": null,
                    "suppressOutput": false,
                    "systemMessage": win.deterministic_prefix()
                })
            }
            _ => json!({
                "continue": true,
                "stopReason": null,
                "suppressOutput": false,
                "systemMessage": null
            }),
        };

        serde_json::to_string(&response)
    }
}

/// Per-file bookmark tracking committed byte offset, pre-read file identity/mtime metadata, and a
/// bounded $O(1)$ multi-window anchor digest (`<= 16 KiB`) so unchanged polls complete in $O(1)$
/// and incremental polls complete in $O(\Delta\text{bytes})$.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RolloutBookmark {
    offset: u64,
    file_len: u64,
    mtime_nanos: Option<u128>,
    file_id: (u64, u64),
    anchor_digest: icx_core::Blake3Digest,
}

/// Maximum per-record buffer allocation (`64 MiB + 64 KiB`) before draining the remainder of an
/// oversized rollout JSONL record through its terminating newline.
pub const MAX_ROLLOUT_RECORD_BYTES: usize = icx_core::ICX_MAX_INGEST_BYTES + 65_536;

/// Incremental JSONL rollout tailer for OpenAI Codex `threads.rollout_path` (`RolloutItemWire`).
#[derive(Clone, Debug, Default)]
pub struct CodexRolloutTailer {
    bookmarks: BTreeMap<PathBuf, RolloutBookmark>,
}

impl CodexRolloutTailer {
    /// Creates a new `CodexRolloutTailer`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the committed byte offset for `path`.
    pub fn offset_for(&self, path: &Path) -> u64 {
        self.bookmarks.get(path).map_or(0, |b| b.offset)
    }

    fn extract_file_id(meta: &std::fs::Metadata) -> (u64, u64) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            (meta.dev(), meta.ino())
        }
        #[cfg(not(unix))]
        {
            let _ = meta;
            (0, 0)
        }
    }

    fn extract_mtime_nanos(meta: &std::fs::Metadata) -> Option<u128> {
        meta.modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos())
    }

    /// Computes a bounded $O(1)$ anchor digest (`<= 16 KiB` total I/O regardless of file size)
    /// sampling up to four 4 KiB windows across `[0..len)` (`0`, `len/3`, `2*len/3`, `len-4096`).
    fn compute_anchor_digest(file: &mut File, len: u64) -> io::Result<icx_core::Blake3Digest> {
        use std::io::Read;
        if len <= 16_384 {
            file.seek(SeekFrom::Start(0))?;
            let mut buf = vec![0u8; len as usize];
            file.read_exact(&mut buf)?;
            buf.extend_from_slice(&len.to_le_bytes());
            return Ok(icx_core::Blake3Digest::hash(&buf));
        }
        let offsets = [0u64, len / 3, (2 * len) / 3, len - 4096];
        let mut combined = Vec::with_capacity(4 * 4096 + 8);
        let mut win = [0u8; 4096];
        for off in offsets {
            file.seek(SeekFrom::Start(off))?;
            file.read_exact(&mut win)?;
            combined.extend_from_slice(&win);
        }
        combined.extend_from_slice(&len.to_le_bytes());
        Ok(icx_core::Blake3Digest::hash(&combined))
    }

    /// Polls `path` from its last committed byte offset (resetting to `0` if the file was
    /// truncated, rotated, or rewritten in-place), ingesting newly appended complete JSONL lines
    /// into `mmu` and advancing the stored bookmark in $O(\Delta\text{bytes})$ using a single
    /// open file descriptor.
    pub fn poll_file(
        &mut self,
        path: &Path,
        mmu: &mut InfinityContextMmu,
    ) -> Result<usize, io::Error> {
        use std::io::Read;
        let mut file = File::open(path)?;
        let meta = file.metadata()?;
        let file_len = meta.len();
        let file_id = Self::extract_file_id(&meta);
        let mtime_nanos = Self::extract_mtime_nanos(&meta);

        let mut start_offset = 0u64;
        let mut validated_anchor = None;

        if let Some(bm) = self.bookmarks.get(path).copied()
            && bm.offset > 0
            && file_len >= bm.offset
            && file_id == bm.file_id
        {
            let cur_anchor = Self::compute_anchor_digest(&mut file, bm.offset)?;
            if cur_anchor == bm.anchor_digest {
                if file_len == bm.file_len && mtime_nanos == bm.mtime_nanos {
                    return Ok(0);
                }
                file.seek(SeekFrom::Start(bm.offset - 1))?;
                let mut prev = [0u8; 1];
                file.read_exact(&mut prev)?;
                if prev[0] == b'\n' {
                    start_offset = bm.offset;
                    validated_anchor = Some(cur_anchor);
                }
            }
        }

        let (next_offset, count, mut file) =
            Self::ingest_from_validated_offset(file, start_offset, mmu)?;
        let anchor_digest = if next_offset == start_offset
            && let Some(reused) = validated_anchor
        {
            reused
        } else {
            Self::compute_anchor_digest(&mut file, next_offset)?
        };
        // Cache the pre-read file_len and mtime_nanos so any concurrent append during
        // `ingest_from_validated_offset` causes the next poll's metadata check to miss and read.
        self.bookmarks.insert(
            path.to_path_buf(),
            RolloutBookmark {
                offset: next_offset,
                file_len,
                mtime_nanos,
                file_id,
                anchor_digest,
            },
        );
        Ok(count)
    }

    /// Reads complete `\n`-terminated `RolloutItemWire` JSONL lines from `path` starting at
    /// `byte_offset` (resetting to `0` if `file_len < byte_offset` or if `byte_offset` does not
    /// land immediately after a newline `b'\n'`), ingests non-empty entries into `mmu`, and
    /// returns `(committed_byte_offset, ingested_count)`.
    pub fn ingest_rollout_jsonl_since(
        path: &Path,
        byte_offset: u64,
        mmu: &mut InfinityContextMmu,
    ) -> Result<(u64, usize), io::Error> {
        use std::io::Read;
        let mut file = File::open(path)?;
        let file_len = file.metadata()?.len();
        let start_offset = if byte_offset == 0 || file_len < byte_offset {
            0
        } else {
            file.seek(SeekFrom::Start(byte_offset - 1))?;
            let mut prev = [0u8; 1];
            file.read_exact(&mut prev)?;
            if prev[0] == b'\n' { byte_offset } else { 0 }
        };
        let (committed, ingested, _) = Self::ingest_from_validated_offset(file, start_offset, mmu)?;
        Ok((committed, ingested))
    }

    /// Reads a single JSONL record from `reader`, appending at most `max_bytes` into `out` while
    /// draining any excess bytes through the terminating `b'\n'`. Returns
    /// `(total_bytes_on_disk, ended_with_newline, was_truncated)`.
    pub fn read_bounded_jsonl_record<R: BufRead>(
        reader: &mut R,
        max_bytes: usize,
        out: &mut Vec<u8>,
    ) -> io::Result<(u64, bool, bool)> {
        out.clear();
        let mut total_bytes = 0u64;
        let mut ended_with_newline = false;
        let mut was_truncated = false;
        loop {
            let buf = reader.fill_buf()?;
            if buf.is_empty() {
                break;
            }
            let (used, found_nl) = match buf.iter().position(|&b| b == b'\n') {
                Some(pos) => (pos + 1, true),
                None => (buf.len(), false),
            };
            total_bytes += used as u64;
            let remaining_cap = max_bytes.saturating_sub(out.len());
            if remaining_cap > 0 {
                let copy_len = used.min(remaining_cap);
                out.extend_from_slice(&buf[..copy_len]);
                if copy_len < used {
                    was_truncated = true;
                }
            } else {
                was_truncated = true;
            }
            reader.consume(used);
            if found_nl {
                ended_with_newline = true;
                break;
            }
        }
        Ok((total_bytes, ended_with_newline, was_truncated))
    }

    fn ingest_from_validated_offset(
        mut file: File,
        start_offset: u64,
        mmu: &mut InfinityContextMmu,
    ) -> Result<(u64, usize, File), io::Error> {
        let mut committed = start_offset;
        file.seek(SeekFrom::Start(committed))?;
        let mut reader = BufReader::new(file);
        let mut raw_line = Vec::new();
        let mut ingested = 0usize;

        loop {
            let (bytes_read, ended_with_nl, was_truncated) = Self::read_bounded_jsonl_record(
                &mut reader,
                MAX_ROLLOUT_RECORD_BYTES,
                &mut raw_line,
            )?;
            if bytes_read == 0 || !ended_with_nl {
                // Stop before decoding or consuming any incomplete trailing line (including
                // mid-character UTF-8 sequences) still being written.
                break;
            }
            committed += bytes_read;
            let line = String::from_utf8_lossy(&raw_line);
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if was_truncated {
                let _ = mmu.ingest_payload_with_exit_code(
                    AgentKind::Codex,
                    "oversized_rollout_item",
                    trimmed,
                    None,
                    false,
                );
                ingested += 1;
                continue;
            }
            let Ok(val) = serde_json::from_str::<Value>(trimmed) else {
                continue;
            };
            if let Some((subject, body, exit_code)) = Self::extract_rollout_item(&val) {
                if InfinityContextMmu::is_icx_pager_tool(&subject) {
                    continue;
                }
                let _ = mmu.ingest_payload_with_exit_code(
                    AgentKind::Codex,
                    &subject,
                    &body,
                    exit_code,
                    false,
                );
                ingested += 1;
            }
        }
        Ok((committed, ingested, reader.into_inner()))
    }

    fn non_empty_body_value(v: &Value) -> Option<String> {
        match v {
            Value::String(s) if !s.trim().is_empty() => Some(s.clone()),
            Value::Array(a) if !a.is_empty() => Some(v.to_string()),
            Value::Object(o) if !o.is_empty() => Some(v.to_string()),
            Value::Number(_) | Value::Bool(_) => Some(v.to_string()),
            _ => None,
        }
    }

    fn extract_rollout_item(val: &Value) -> Option<(String, String, Option<i32>)> {
        let item = ["item", "payload"]
            .into_iter()
            .find_map(|k| val.get(k))
            .unwrap_or(val);
        let kind = item
            .get("type")
            .and_then(Value::as_str)
            .or_else(|| val.get("type").and_then(Value::as_str))
            .unwrap_or("rollout_item");
        let subject = ["name", "tool_name", "call_id", "role"]
            .into_iter()
            .find_map(|k| {
                item.get(k)
                    .and_then(Value::as_str)
                    .or_else(|| val.get(k).and_then(Value::as_str))
            })
            .unwrap_or(kind)
            .to_owned();
        let body = ["output", "content", "summary", "text"]
            .into_iter()
            .find_map(|k| {
                item.get(k)
                    .and_then(Self::non_empty_body_value)
                    .or_else(|| val.get(k).and_then(Self::non_empty_body_value))
            })?;
        let exit_code = InfinityContextMmu::extract_exit_code(item)
            .or_else(|| InfinityContextMmu::extract_exit_code(val));
        Some((subject, body, exit_code))
    }
}
