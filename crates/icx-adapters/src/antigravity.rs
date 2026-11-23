//! Google Antigravity CLI (`agy`) native lifecycle hook adapter (`AntigravityAdapter`).
//!
//! Uses `SessionStart`, `PreInvocation`, and `PostInvocation` `injectSteps` to put icx frames in
//! the model's context. The steps carry `metadata.user_markdown = ""`, but on agy 1.5.14 that hides
//! nothing: the terminal UI never reads it, the web UI treats an empty value as unset, and the
//! steps are visible to the user (Sprint 1, T-65-003). Whether `renderInfo.hidden` hides a
//! hook-injected step is untested, so it is not emitted. Also handles `Stop` continuation across
//! `max_steps_exceeded` / `error` (and the legacy `MAX_*` reasons) by sealing an epoch; it never
//! overrides a stop.

use crate::codex::CodexRolloutTailer;
use crate::mmu::InfinityContextMmu;
use crate::{AdapterError, AgentHookAdapter};
use icx_core::AgentKind;
use serde_json::{Value, json};
use std::fs::File;
use std::io::{BufReader, Seek, SeekFrom};
use std::path::Path;

const MAX_TRANSCRIPT_RECORD_BYTES: usize = 2 * icx_core::ICX_MAX_INGEST_BYTES;
const TRANSCRIPT_TAIL_WINDOW_BYTES: u64 = 512 * 1024;
const MAX_TRANSCRIPT_SCAN_LINES: usize = 10_000;

/// Native hook adapter for Google Antigravity CLI (`agy`).
#[derive(Clone, Debug, Default)]
pub struct AntigravityAdapter;

impl AgentHookAdapter for AntigravityAdapter {
    fn agent_kind(&self) -> AgentKind {
        AgentKind::AntigravityCli
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

impl AntigravityAdapter {
    fn non_empty_json_value(v: &Value) -> Option<String> {
        match v {
            Value::String(s) if !s.trim().is_empty() => Some(s.clone()),
            Value::Array(a) if !a.is_empty() => Some(v.to_string()),
            Value::Object(o) if !o.is_empty() => Some(v.to_string()),
            Value::Number(_) | Value::Bool(_) => Some(v.to_string()),
            _ => None,
        }
    }

    fn extract_error_string(v: &Value) -> Option<String> {
        if matches!(v, Value::Bool(false)) {
            None
        } else {
            Self::non_empty_json_value(v)
        }
    }

    fn attach_conversation_id(tail: String, payload: &Value) -> String {
        if let Some(conv_id) = payload
            .get("conversationId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let clean_conv = InfinityContextMmu::sanitize_hook_label(conv_id, "conv");
            return tail.replacen(
                "⟪ICX:TAIL_SLOT│",
                &format!("⟪ICX:TAIL_SLOT│conv={clean_conv}│"),
                1,
            );
        }
        tail
    }

    fn scan_transcript_reader(
        reader: &mut BufReader<File>,
        target_step_idx: Option<u64>,
        max_lines: usize,
        allow_ordinal_fallback: bool,
    ) -> (Option<String>, bool) {
        let mut raw_line = Vec::new();
        let mut latest_body: Option<String> = None;
        let mut valid_step_idx = 0u64;
        let mut saw_step_record = false;

        for _ in 0..max_lines {
            let (bytes_read, ended_with_nl, was_truncated) =
                CodexRolloutTailer::read_bounded_jsonl_record(
                    reader,
                    MAX_TRANSCRIPT_RECORD_BYTES,
                    &mut raw_line,
                )
                .unwrap_or((0, false, false));
            if bytes_read == 0 || !ended_with_nl {
                break;
            }
            if was_truncated {
                saw_step_record = true;
                if target_step_idx.is_none() {
                    latest_body = None;
                }
                valid_step_idx += 1;
                continue;
            }
            let line = String::from_utf8_lossy(&raw_line);
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let Ok(val) = serde_json::from_str::<Value>(trimmed) else {
                continue;
            };
            if !val.is_object() {
                continue;
            }
            saw_step_record = true;
            let fallback_idx = valid_step_idx;
            valid_step_idx += 1;
            let explicit_step_idx = ["step_index", "stepIdx", "idx"]
                .into_iter()
                .find_map(|k| val.get(k).and_then(Value::as_u64));
            let step_idx_opt =
                explicit_step_idx.or_else(|| allow_ordinal_fallback.then_some(fallback_idx));
            let body = [
                "content",
                "output",
                "toolResponse",
                "tool_response",
                "toolResult",
                "result",
                "text",
            ]
            .into_iter()
            .find_map(|k| val.get(k).and_then(Self::non_empty_json_value));

            if target_step_idx.is_some() && target_step_idx == step_idx_opt && body.is_some() {
                return (body, true);
            }
            latest_body = body;
        }
        if target_step_idx.is_some() {
            (None, saw_step_record)
        } else {
            (latest_body, saw_step_record)
        }
    }

    /// Reads a tool/step output from an Antigravity `transcript.jsonl` / `transcript_full.jsonl`
    /// file using bounded JSONL record framing. When `target_step_idx` is `Some(idx)`, returns
    /// that step's content if found; otherwise returns the content of the latest step record in
    /// the trailing window of `path` (falling back to a full scan from offset 0 if a single record
    /// exceeds the 512 KiB tail window).
    pub fn read_transcript_step(path: &Path, target_step_idx: Option<u64>) -> Option<String> {
        // ICX-AUD-018: reject non-regular files (FIFOs, devices, symlinks) *before* opening, and
        // open non-blocking without following symlinks so a swap between the check and the open
        // cannot block the hook; then re-check the opened handle.
        if !std::fs::symlink_metadata(path).ok()?.file_type().is_file() {
            return None;
        }
        let mut opts = std::fs::OpenOptions::new();
        opts.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let flags = rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::NOFOLLOW;
            opts.custom_flags(flags.bits().cast_signed());
        }
        let mut file: File = opts.open(path).ok()?;
        let meta = file.metadata().ok()?;
        if !meta.file_type().is_file() {
            return None;
        }
        let file_len = meta.len();
        let max_lines = target_step_idx
            .and_then(|t| usize::try_from(t).ok())
            .map(|t| {
                t.saturating_add(1024)
                    .clamp(MAX_TRANSCRIPT_SCAN_LINES, 1_000_000)
            })
            .unwrap_or(1_000_000);

        if file_len > TRANSCRIPT_TAIL_WINDOW_BYTES {
            let start = file_len - TRANSCRIPT_TAIL_WINDOW_BYTES;
            file.seek(SeekFrom::Start(start)).ok()?;
            let mut r = BufReader::new(file);
            let mut discard = Vec::new();
            let _ = CodexRolloutTailer::read_bounded_jsonl_record(
                &mut r,
                MAX_TRANSCRIPT_RECORD_BYTES,
                &mut discard,
            );
            let (body_opt, saw_step_record) =
                Self::scan_transcript_reader(&mut r, target_step_idx, max_lines, false);
            if body_opt.is_some() || (target_step_idx.is_none() && saw_step_record) {
                return body_opt;
            }
            file = r.into_inner();
            file.seek(SeekFrom::Start(0)).ok()?;
        }

        let mut reader = BufReader::new(file);
        Self::scan_transcript_reader(&mut reader, target_step_idx, max_lines, true).0
    }

    /// Handles an Antigravity CLI hook payload (camelCase protojson) and returns the response JSON.
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
                let tail = Self::attach_conversation_id(
                    mmu.build_ephemeral_tail_slot(AgentKind::AntigravityCli, "session_init", 3),
                    &payload,
                );
                json!({
                    "injectSteps": [
                        {
                            "systemMessage": {
                                "systemMessage": tail,
                                "metadata": {
                                    "user_markdown": ""
                                }
                            }
                        }
                    ],
                    "terminationBehavior": ""
                })
            }
            "PreInvocation" | "pre-invocation" => {
                let last_input = ["lastUserInput", "prompt", "query"]
                    .into_iter()
                    .find_map(|k| {
                        payload
                            .get(k)
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                    })
                    .unwrap_or("agy_turn");
                let tail = Self::attach_conversation_id(
                    mmu.build_ephemeral_tail_slot(AgentKind::AntigravityCli, last_input, 4),
                    &payload,
                );
                json!({
                    "injectSteps": [
                        {
                            "systemMessage": {
                                "systemMessage": tail,
                                "metadata": {
                                    "user_markdown": ""
                                }
                            }
                        }
                    ],
                    "terminationBehavior": ""
                })
            }
            "PostInvocation" | "post-invocation" => {
                let query = ["lastUserInput", "prompt", "query"]
                    .into_iter()
                    .find_map(|k| {
                        payload
                            .get(k)
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                    })
                    .unwrap_or("agy_post_invocation");
                let tail = Self::attach_conversation_id(
                    mmu.build_ephemeral_tail_slot(AgentKind::AntigravityCli, query, 3),
                    &payload,
                );
                // ICX-AUD-007/-009: icx never forces the agent to keep running, and never turns
                // input fields into control decisions (v1.0.x echoed `forceContinue`).
                let term_behavior = "";
                json!({
                    "injectSteps": [
                        {
                            "systemMessage": {
                                "systemMessage": tail,
                                "metadata": {
                                    "user_markdown": ""
                                }
                            }
                        }
                    ],
                    "terminationBehavior": term_behavior
                })
            }
            "PreToolUse" | "pre-tool-use" => {
                // `decision` is required by agy. "allow" defers to the next hook or the default
                // policy, so icx expresses no opinion. v1.0.x echoed `decision` / `overwrite`
                // from the hook input back as its own decision (ICX-AUD-009 class).
                json!({ "decision": "allow" })
            }
            "PostToolUse" | "post-tool-use" => {
                let tool_name = payload
                    .get("toolCall")
                    .and_then(|tc| tc.get("name"))
                    .or_else(|| payload.get("tool_name"))
                    .or_else(|| payload.get("toolName"))
                    .and_then(Value::as_str)
                    .unwrap_or("agy_tool");
                // agy names an eagerly loaded MCP tool `mcp_<server>_<tool>`, but a lazily loaded
                // one arrives as `call_mcp_tool` with `args.ServerName` / `args.ToolName`
                // (recorded live on agy 1.5.14, T-65-008). icx's own tools are exempt either way.
                let lazy_mcp_name = (tool_name == "call_mcp_tool")
                    .then(|| {
                        let args = payload.get("toolCall")?.get("args")?;
                        let server = args.get("ServerName")?.as_str()?;
                        let tool = args.get("ToolName")?.as_str()?;
                        Some(format!("mcp_{server}_{tool}"))
                    })
                    .flatten();
                if InfinityContextMmu::is_icx_pager_tool(tool_name)
                    || lazy_mcp_name
                        .as_deref()
                        .is_some_and(InfinityContextMmu::is_icx_pager_tool)
                {
                    return serde_json::to_string(&json!({}));
                }
                let subject = InfinityContextMmu::extract_hook_subject(&payload, tool_name);
                let args_str = payload
                    .get("toolCall")
                    .and_then(|tc| tc.get("args"))
                    .and_then(Self::non_empty_json_value)
                    .unwrap_or_default();
                let mut resp_str = [
                    "toolResponse",
                    "tool_response",
                    "toolResult",
                    "result",
                    "output",
                ]
                .into_iter()
                .find_map(|k| payload.get(k).and_then(Self::non_empty_json_value))
                .unwrap_or_default();
                if resp_str.is_empty()
                    && let Some(tp) = payload
                        .get("transcriptPath")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                {
                    let step_idx = ["stepIdx", "step_index", "idx"]
                        .into_iter()
                        .find_map(|k| payload.get(k).and_then(Value::as_u64));
                    resp_str =
                        Self::read_transcript_step(Path::new(tp), step_idx).unwrap_or_default();
                }
                let err_str = payload
                    .get("error")
                    .and_then(Self::extract_error_string)
                    .unwrap_or_default();
                let err_line = if err_str.is_empty() {
                    String::new()
                } else {
                    format!("error: {err_str}")
                };
                let combined = [args_str, resp_str, err_line]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n");
                let exit_code = InfinityContextMmu::extract_exit_code(&payload);
                let sub_id = ["subagentId", "subagent_id", "agentId", "agent_id"]
                    .into_iter()
                    .find_map(|k| {
                        payload
                            .get(k)
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                    });
                if !combined.is_empty() || exit_code.is_some() || !err_str.is_empty() {
                    let _ = mmu.ingest_payload_for_subagent(
                        AgentKind::AntigravityCli,
                        &subject,
                        &combined,
                        exit_code,
                        !err_str.is_empty(),
                        sub_id,
                    );
                }
                json!({})
            }
            "SubagentStart" | "subagent-start" => {
                let sub_id = ["subagentId", "subagent_id", "agentId", "agent_id"]
                    .into_iter()
                    .find_map(|k| {
                        payload
                            .get(k)
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                    })
                    .unwrap_or("agy_subagent");
                let clean_id = mmu.begin_subagent(sub_id);
                json!({
                    "injectSteps": [
                        {
                            "systemMessage": {
                                "systemMessage": format!("⟪ICX:SUBAGENT_START│{clean_id}⟫"),
                                "metadata": {
                                    "user_markdown": ""
                                }
                            }
                        }
                    ]
                })
            }
            "SubagentStop" | "subagent-stop" => {
                let sub_id = ["subagentId", "subagent_id", "agentId", "agent_id"]
                    .into_iter()
                    .find_map(|k| {
                        payload
                            .get(k)
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                    });
                let summary = ["summary", "result", "lastAssistantMessage"]
                    .into_iter()
                    .find_map(|k| {
                        payload
                            .get(k)
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                    })
                    .unwrap_or("subagent_completed");
                let folded_msg = mmu.finish_subagent(AgentKind::AntigravityCli, sub_id, summary);
                json!({
                    "injectSteps": [
                        {
                            "systemMessage": {
                                "systemMessage": folded_msg,
                                "metadata": {
                                    "user_markdown": ""
                                }
                            }
                        }
                    ]
                })
            }
            "Stop" | "stop" => {
                // ICX-AUD-007: `decision: "continue"` makes agy keep running. v1.0.x returned it
                // when the user's step/token budget ran out, silently overriding that limit.
                // v1.1.0 always lets the stop happen and only preserves state: open subagents
                // are folded and the active window is sealed into an epoch, so the next session
                // resumes from it.
                let reason = payload
                    .get("terminationReason")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                mmu.fold_all_open_subagents(AgentKind::AntigravityCli, "agy_stop");
                if matches!(
                    reason,
                    "max_steps_exceeded"
                        | "error"
                        | "MAX_TOKEN_BUDGET_EXCEEDED"
                        | "MAX_INVOCATIONS"
                ) {
                    let _ = mmu.force_rollover_and_sync_dag_root();
                } else {
                    let _ = mmu.rollover_and_sync_dag_root();
                }
                json!({ "decision": "stop" })
            }
            "SessionEnd" | "session-end" => {
                mmu.fold_all_open_subagents(AgentKind::AntigravityCli, "session_ended");
                let _ = mmu.force_rollover_and_sync_dag_root();
                json!({})
            }
            _ => json!({}),
        };

        serde_json::to_string(&response)
    }
}
