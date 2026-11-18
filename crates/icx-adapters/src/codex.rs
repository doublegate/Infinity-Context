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
                let mut hso = json!({
                    "hookEventName": "PreToolUse"
                });
                if let Some(updated) = payload.get("updatedInput").filter(|v| !v.is_null()) {
                    hso["updatedInput"] = updated.clone();
                }
                json!({
                    "continue": true,
                    "hookSpecificOutput": hso,
                    "stopReason": null,
                    "suppressOutput": false,
                    "systemMessage": null
                })
            }
            "PostToolUse" | "post-tool-use" => {
                let tool_name = payload
                    .get("tool_name")
                    .and_then(Value::as_str)
                    .unwrap_or("codex_tool");
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
                    tool_name,
                    &raw_output,
                    exit_code,
                    false,
                    explicit_sub_id.as_deref(),
                );
                let mut hso = json!({
                    "hookEventName": "PostToolUse",
                    "additionalContext": res.compressed_ahtl
                });
                if tool_name.starts_with("mcp__") && res.was_virtualized {
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
                    hso["updatedMCPToolOutput"] = Value::String(stub);
                }

                json!({
                    "continue": true,
                    "hookSpecificOutput": hso,
                    "reason": null,
                    "stopReason": null,
                    "suppressOutput": false,
                    "systemMessage": null
                })
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

/// Per-file bookmark tracking committed byte offset and prefix digest so file replacement or
/// truncation/rewrite is deterministically detected even when the replacement file is longer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RolloutBookmark {
    offset: u64,
    prefix_digest: icx_core::Blake3Digest,
}

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

    fn compute_prefix_digest(file: &mut File, len: u64) -> io::Result<icx_core::Blake3Digest> {
        use std::io::Read;
        file.seek(SeekFrom::Start(0))?;
        let mut acc = [0u8; 40];
        acc[32..40].copy_from_slice(&len.to_le_bytes());
        let mut remaining = len as usize;
        let mut chunk = [0u8; 8192];
        while remaining > 0 {
            let to_read = remaining.min(chunk.len());
            file.read_exact(&mut chunk[..to_read])?;
            let h = icx_core::Blake3Digest::hash(&chunk[..to_read]);
            for (a, b) in acc[..32].iter_mut().zip(h.0.iter()) {
                *a ^= *b;
            }
            let mixed = icx_core::Blake3Digest::hash(&acc);
            acc[..32].copy_from_slice(&mixed.0);
            remaining -= to_read;
        }
        Ok(icx_core::Blake3Digest::hash(&acc))
    }

    /// Polls `path` from its last committed byte offset (resetting to `0` if the file was
    /// truncated, rotated, or rewritten in-place), ingesting newly appended complete JSONL lines
    /// into `mmu` and advancing the stored bookmark using a single open file descriptor.
    pub fn poll_file(
        &mut self,
        path: &Path,
        mmu: &mut InfinityContextMmu,
    ) -> Result<usize, io::Error> {
        let prev_bm = self.bookmarks.get(path).copied();
        let (start_offset, expected_prefix) = match prev_bm {
            Some(bm) if bm.offset > 0 => (bm.offset, Some(bm.prefix_digest)),
            _ => (0, None),
        };
        let file = File::open(path)?;
        let (next_offset, count, prefix_digest) =
            Self::ingest_from_open_file(file, start_offset, expected_prefix, true, mmu)?;
        self.bookmarks.insert(
            path.to_path_buf(),
            RolloutBookmark {
                offset: next_offset,
                prefix_digest: prefix_digest.unwrap_or_else(|| icx_core::Blake3Digest::hash(&[])),
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
        let file = File::open(path)?;
        let (committed, ingested, _) =
            Self::ingest_from_open_file(file, byte_offset, None, false, mmu)?;
        Ok((committed, ingested))
    }

    fn ingest_from_open_file(
        mut file: File,
        byte_offset: u64,
        expected_prefix: Option<icx_core::Blake3Digest>,
        need_prefix_digest: bool,
        mmu: &mut InfinityContextMmu,
    ) -> Result<(u64, usize, Option<icx_core::Blake3Digest>), io::Error> {
        use std::io::Read;
        let file_len = file.metadata()?.len();
        let mut committed = if byte_offset == 0 || file_len < byte_offset {
            0
        } else {
            let prefix_ok = match expected_prefix {
                Some(exp) => Self::compute_prefix_digest(&mut file, byte_offset)? == exp,
                None => true,
            };
            if prefix_ok {
                file.seek(SeekFrom::Start(byte_offset - 1))?;
                let mut prev = [0u8; 1];
                file.read_exact(&mut prev)?;
                if prev[0] == b'\n' { byte_offset } else { 0 }
            } else {
                0
            }
        };
        file.seek(SeekFrom::Start(committed))?;
        let mut reader = BufReader::new(file);
        let mut raw_line = Vec::new();
        let mut ingested = 0usize;

        loop {
            raw_line.clear();
            let bytes_read = (&mut reader)
                .take(icx_core::ICX_MAX_INGEST_BYTES as u64 + 4096)
                .read_until(b'\n', &mut raw_line)?;
            if bytes_read == 0 || !raw_line.ends_with(b"\n") {
                // Stop before decoding or consuming any incomplete trailing line (including
                // mid-character UTF-8 sequences) still being written.
                break;
            }
            committed += bytes_read as u64;
            let line = String::from_utf8_lossy(&raw_line);
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let Ok(val) = serde_json::from_str::<Value>(trimmed) else {
                continue;
            };
            if let Some((subject, body, exit_code)) = Self::extract_rollout_item(&val) {
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
        let prefix_digest = if need_prefix_digest {
            let mut file = reader.into_inner();
            Some(Self::compute_prefix_digest(&mut file, committed)?)
        } else {
            None
        };
        Ok((committed, ingested, prefix_digest))
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
