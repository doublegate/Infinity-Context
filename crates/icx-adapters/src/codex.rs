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
    /// Native Codex hooks capture tool output but cannot replace it. Supported session/prompt
    /// `additionalContext` supplies recall separately; use the authenticated proxy for paging.
    pub const SUPPORTS_TOOL_OUTPUT_REPLACEMENT: bool = false;

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
                let resume = mmu
                    .take_resume_packet(AgentKind::Codex, crate::mmu::RESUME_PACKET_MAX_BYTES)
                    .map_or_else(String::new, |p| format!("\n{p}"));
                json!({
                    "continue": true,
                    "hookSpecificOutput": {
                        "hookEventName": "SessionStart",
                        "additionalContext": codex_inline_context(format!("{ICX_AHTL_LEGEND}\n{tail}{resume}"))
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
                // Codex has no hook that changes the summary; the first hook after compaction
                // that carries context delivers the resume packet (T-73-002).
                let resume = mmu
                    .take_resume_packet(AgentKind::Codex, crate::mmu::RESUME_PACKET_MAX_BYTES)
                    .map_or_else(String::new, |p| format!("\n{p}"));
                json!({
                    "continue": true,
                    "hookSpecificOutput": {
                        "hookEventName": "UserPromptSubmit",
                        "additionalContext": codex_inline_context(format!("{tail}{resume}"))
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
                    let raw_output = crate::claude::tool_output_text(
                        payload.get("tool_response").filter(|v| !v.is_null()),
                    );
                    let exit_code = InfinityContextMmu::extract_exit_code(&payload);
                    let explicit_sub_id = ["subagent_id", "agent_id"]
                        .into_iter()
                        .find_map(|k| payload.get(k).and_then(Value::as_str))
                        .map(str::to_owned);
                    let _ = mmu.ingest_payload_for_subagent(
                        AgentKind::Codex,
                        &subject,
                        &raw_output,
                        exit_code,
                        false,
                        explicit_sub_id.as_deref(),
                    );
                    // Codex 0.171.0 and its published contract reject updatedMCPToolOutput.
                    // Capture for recall/deref, without claiming native output replacement or
                    // adding a second copy alongside the original through additionalContext.
                    let hso = json!({ "hookEventName": "PostToolUse" });

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
                        "additionalContext": codex_inline_context(tail)
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
                let summary_raw = ["last_assistant_message", "summary", "outcome"]
                    .into_iter()
                    .find_map(|k| payload.get(k).and_then(Value::as_str))
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or("subagent_completed");
                let summary: String = summary_raw.chars().take(240).collect();
                let folded_msg = mmu.finish_subagent(AgentKind::Codex, subagent_id, &summary);
                json!({
                    "continue": true,
                    "stopReason": null,
                    "suppressOutput": false,
                    "systemMessage": folded_msg
                })
            }
            "PreCompact" | "pre-compact" | "PostCompact" | "post-compact" => {
                let _ = mmu.force_rollover_and_sync_dag_root();
                mmu.mark_resume_pending(AgentKind::Codex);
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
                let _ = mmu.force_rollover_and_sync_dag_root();
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

        let mut response = response;
        strip_null_members(&mut response);
        serde_json::to_string(&response)
    }
}

/// Largest `additionalContext` Codex delivers inline under its default per-handler
/// `additionalContextLimit` of 2,500 tokens. Codex estimates tokens as UTF-8 bytes / 4 (rounded up)
/// and spills anything larger to a file, leaving the model only a preview (T-62-006; Codex
/// `codex-rs/hooks/src/output_spill.rs` and `utils/string/src/truncate.rs` at `rust-v0.171.0`).
pub const CODEX_INLINE_CONTEXT_MAX_BYTES: usize = 2_500 * 4;

/// Keeps a Codex `additionalContext` value within [`CODEX_INLINE_CONTEXT_MAX_BYTES`]. The tail
/// slot is already far smaller; this pins the bound at the reply. Over the bound, the context is
/// replaced whole by a diagnostic, never cut mid-frame, so no `#icx:` handle is ever split.
fn codex_inline_context(context: String) -> String {
    if context.len() <= CODEX_INLINE_CONTEXT_MAX_BYTES {
        context
    } else {
        "⟪ICX:TAIL_OMITTED│reason=codex_additional_context_limit⟫".to_owned()
    }
}

#[cfg(test)]
mod inline_context_tests {
    use super::*;

    #[test]
    fn context_at_the_limit_passes_and_one_byte_over_is_replaced_whole() {
        let at = "x".repeat(CODEX_INLINE_CONTEXT_MAX_BYTES);
        assert_eq!(codex_inline_context(at.clone()), at);
        let over = format!("{at}y");
        let replaced = codex_inline_context(over);
        assert!(replaced.starts_with("⟪ICX:TAIL_OMITTED"));
        assert!(replaced.len() <= CODEX_INLINE_CONTEXT_MAX_BYTES);
        // Multi-byte text is measured in bytes, as Codex measures it.
        let wide = "⟪".repeat(CODEX_INLINE_CONTEXT_MAX_BYTES / 3 + 1);
        assert!(codex_inline_context(wide).starts_with("⟪ICX:TAIL_OMITTED"));
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(64))]

        /// For any input the reply fits, and fitting input passes through unchanged.
        #[test]
        fn any_context_fits_and_fitting_context_is_unchanged(
            unit in "[a-z⟪⟫#:0-9 \n]{1,8}",
            repeat in 0usize..4_000,
        ) {
            let input = unit.repeat(repeat);
            let out = codex_inline_context(input.clone());
            proptest::prop_assert!(out.len() <= CODEX_INLINE_CONTEXT_MAX_BYTES);
            if input.len() <= CODEX_INLINE_CONTEXT_MAX_BYTES {
                proptest::prop_assert_eq!(out, input);
            }
        }
    }
}

/// Recursively removes `null` object members. Codex CLI marks a hook "Failed" when its reply
/// carries an explicit `null` (e.g. `"stopReason": null`), whereas an absent optional field is
/// accepted, so every Codex reply is normalised through this before serialisation.
fn strip_null_members(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|_, member| !member.is_null());
            map.values_mut().for_each(strip_null_members);
        }
        Value::Array(items) => items.iter_mut().for_each(strip_null_members),
        _ => {}
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

/// Which rollout records [`CodexRolloutTailer::ingest_jsonl_records`] leaves out. The default
/// ingests everything (the in-process tailer's behaviour).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RecordFilter {
    /// Skip a record whose redacted output the store already holds byte for byte.
    pub skip_held: bool,
    /// Skip tool-output records (`*_call_output`). icx's Codex hook captures tool output when
    /// the tool runs, and its copy never matches the rollout's (by bytes or, under Code Mode, by
    /// call id), so ingesting both would index each output twice.
    pub skip_tool_outputs: bool,
}

/// What one pass of [`CodexRolloutTailer::ingest_jsonl_records`] did.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RecordCounts {
    /// Bytes consumed through the last complete line.
    pub consumed: u64,
    pub ingested: usize,
    /// Records skipped because the store already held them.
    pub held: usize,
    /// Tool-output records skipped by `skip_tool_outputs`.
    pub tool_outputs: usize,
}

/// Whether a rollout record is a tool result (`function_call_output`, `custom_tool_call_output`
/// and any other `*_call_output`).
fn is_tool_output_record(val: &Value) -> bool {
    ["payload", "item"]
        .into_iter()
        .find_map(|k| val.get(k))
        .unwrap_or(val)
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|t| t.ends_with("_call_output"))
}

/// The same test on the first bytes of a record too large to parse. The record's own `type`
/// values are read in order at any spacing, skipping wrapper kinds, so the payload's or item's
/// type decides; a message that merely quotes `"function_call_output"` in its text does not
/// match (review of #87).
fn is_tool_output_prefix(line: &str) -> bool {
    // Wrapper records name their kind first; the payload's or item's type follows.
    const WRAPPERS: [&str; 5] = [
        "response_item",
        "event_msg",
        "turn_context",
        "session_meta",
        "compacted",
    ];
    let end = line.len().min(1024);
    let mut cut = end;
    while !line.is_char_boundary(cut) {
        cut -= 1;
    }
    let head = &line[..cut];
    // As in the parsed test, the `payload` or `item` object's type decides when there is one, so
    // an outer event type (`item.completed`, `response.output_item.done`) never hides it (review
    // of #88).
    let inner = ["\"payload\"", "\"item\""]
        .iter()
        .filter_map(|key| unescaped_key_value(head, key))
        .min();
    let types = json_type_values(inner.map_or(head, |at| &head[at..]));
    let decisive = if inner.is_some() {
        types.first().copied()
    } else {
        types.into_iter().find(|t| !WRAPPERS.contains(t))
    };
    decisive.is_some_and(|t| t.ends_with("_call_output"))
}

/// Where the value of the first unescaped `key` (quoted, followed by `:` at any spacing) starts.
fn unescaped_key_value(text: &str, key: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(rel) = text[from..].find(key) {
        let at = from + rel;
        from = at + key.len();
        let escaped = at > 0 && text.as_bytes()[at - 1] == b'\\';
        let after = &text[from..];
        if !escaped && after.trim_start().starts_with(':') {
            return Some(from);
        }
    }
    None
}

/// The `"type"` string values in `text`, in order, at any spacing. A `"type"` quoted inside a
/// string value is escaped (`\"type\"`), so it is never one of these.
fn json_type_values(text: &str) -> Vec<&str> {
    let mut types = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("\"type\"") {
        let escaped = at > 0 && rest.as_bytes()[at - 1] == b'\\';
        let after = rest[at + 6..].trim_start();
        rest = &rest[at + 6..];
        if escaped {
            continue;
        }
        let Some(value) = after.strip_prefix(':').map(str::trim_start) else {
            continue;
        };
        if let Some(v) = value
            .strip_prefix('"')
            .and_then(|v| v.split_once('"').map(|(t, _)| t))
        {
            types.push(v);
        }
    }
    types
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
        file.seek(SeekFrom::Start(start_offset))?;
        let mut reader = BufReader::new(file);
        let counts = Self::ingest_jsonl_records(&mut reader, mmu, RecordFilter::default())?;
        Ok((
            start_offset + counts.consumed,
            counts.ingested,
            reader.into_inner(),
        ))
    }

    /// Ingests complete `\n`-terminated rollout records from `reader` into `mmu`, stopping before
    /// an incomplete trailing line. Shared by the plain-file tailer and the compressed-rollout
    /// reader; `filter` says which records to leave out.
    pub(crate) fn ingest_jsonl_records<R: BufRead>(
        reader: &mut R,
        mmu: &mut InfinityContextMmu,
        filter: RecordFilter,
    ) -> Result<RecordCounts, io::Error> {
        let mut committed = 0u64;
        let mut raw_line = Vec::new();
        let mut ingested = 0usize;
        let mut skipped = 0usize;
        let mut tool_outputs = 0usize;

        loop {
            let (bytes_read, ended_with_nl, was_truncated) =
                Self::read_bounded_jsonl_record(reader, MAX_ROLLOUT_RECORD_BYTES, &mut raw_line)?;
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
                // An oversized record cannot be parsed; its type is read from its first bytes.
                if filter.skip_tool_outputs && is_tool_output_prefix(trimmed) {
                    tool_outputs += 1;
                    continue;
                }
                if filter.skip_held && mmu.holds_ingested(trimmed) {
                    skipped += 1;
                    continue;
                }
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
            if filter.skip_tool_outputs && is_tool_output_record(&val) {
                tool_outputs += 1;
                continue;
            }
            if let Some((subject, body, exit_code)) = Self::extract_rollout_item(&val) {
                if InfinityContextMmu::is_icx_pager_tool(&subject) {
                    continue;
                }
                if filter.skip_held && mmu.holds_ingested(&body) {
                    skipped += 1;
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
        Ok(RecordCounts {
            consumed: committed,
            ingested,
            held: skipped,
            tool_outputs,
        })
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

#[cfg(test)]
mod record_filter_tests {
    use super::{is_tool_output_prefix, is_tool_output_record};
    use serde_json::json;

    #[test]
    fn tool_output_records_are_recognised_by_payload_type() {
        for ty in [
            "function_call_output",
            "custom_tool_call_output",
            "mcp_tool_call_output",
        ] {
            let rec = json!({"type": "response_item", "payload": {"type": ty, "output": "x"}});
            assert!(is_tool_output_record(&rec), "{ty}");
            // Codex writes the payload's `type` first; `json!` would sort the keys.
            let line = format!(
                r#"{{"timestamp":"2026-11-22T00:00:00Z","type":"response_item","payload":{{"type":"{ty}","call_id":"c1","output":"x"}}}}"#
            );
            assert!(is_tool_output_prefix(&line), "{ty}");
        }
        let item = json!({"item": {"type": "function_call_output"}});
        assert!(is_tool_output_record(&item));
        assert!(is_tool_output_record(
            &json!({"type": "function_call_output"})
        ));
        let message = json!({"type": "response_item", "payload": {"type": "message",
            "content": "see \"type\":\"function_call_output\" and \"function_call_output\""}});
        assert!(!is_tool_output_record(&message));
        assert!(
            !is_tool_output_prefix(&message.to_string()),
            "quoted text is not the type"
        );
        assert!(!is_tool_output_record(
            &json!({"payload": {"type": "function_call"}})
        ));
    }

    #[test]
    fn prefix_test_tolerates_spacing_and_direct_records() {
        assert!(is_tool_output_prefix(
            r#"{ "type" : "response_item", "payload" : { "type" : "function_call_output" }"#
        ));
        assert!(is_tool_output_prefix(
            r#"{"type":"custom_tool_call_output","output":"x"#
        ));
        assert!(is_tool_output_prefix(
            r#"{"item": {"type": "function_call_output""#
        ));
        assert!(!is_tool_output_prefix(
            r#"{"type":"response_item","payload":{"type":"message","content":"a \"type\":\"function_call_output\" b"#
        ));
        assert!(!is_tool_output_prefix(
            r#"{"type":"event_msg","payload":{"type":"token_count"}}"#
        ));
        // An outer event type does not hide the item's type (review of #88).
        for line in [
            r#"{"type":"item.completed","item":{"type":"function_call_output","output":"x"#,
            r#"{"type": "response.output_item.done", "item": {"type": "custom_tool_call_output""#,
        ] {
            assert!(is_tool_output_prefix(line), "{line}");
        }
        assert!(!is_tool_output_prefix(
            r#"{"type":"item.completed","item":{"type":"agent_message","text":"x"#
        ));
        // A quoted `item` key inside text is not the record's item.
        assert!(!is_tool_output_prefix(
            r#"{"type":"response_item","payload":{"type":"message","content":"\"item\": {\"type\": \"function_call_output\""#
        ));
    }

    #[test]
    fn an_oversized_tool_output_record_is_skipped_by_its_prefix() {
        // Larger than a record may be, so it cannot be parsed; its opening bytes decide, here
        // with an outer event type before the item (review of #88).
        let mut line = String::from(
            r#"{"type":"item.completed","item":{"type":"function_call_output","output":""#,
        );
        line.push_str(&"x".repeat(super::MAX_ROLLOUT_RECORD_BYTES + 16));
        line.push_str("\"}}\n");
        let mut reader = std::io::Cursor::new(line.into_bytes());
        let mut mmu = crate::InfinityContextMmu::default();
        let filter = super::RecordFilter {
            skip_held: true,
            skip_tool_outputs: true,
        };
        let counts =
            super::CodexRolloutTailer::ingest_jsonl_records(&mut reader, &mut mmu, filter).unwrap();
        assert_eq!((counts.tool_outputs, counts.ingested), (1, 0));
        assert_eq!(mmu.cas.artifact_count(), 0);
    }

    #[test]
    fn prefix_test_reads_only_the_opening_bytes_on_a_char_boundary() {
        let late = format!(
            "{}\"payload\":{{\"type\":\"function_call_output\"",
            "é".repeat(600)
        );
        assert!(
            !is_tool_output_prefix(&late),
            "beyond the first 1,024 bytes"
        );
        assert!(!is_tool_output_prefix(""));
    }
}
