//! Google Antigravity CLI (`agy`) native lifecycle hook adapter (`AntigravityAdapter`).
//!
//! Exploits `SessionStart`, `PreInvocation`, and `PostInvocation` `injectSteps` with
//! `metadata.user_markdown = ""` so non-human `AHTL` frames are completely invisible in the human
//! chat UI while injected directly into the model's context, plus `Stop` continuation across
//! `MAX_TOKEN_BUDGET_EXCEEDED` and `MAX_INVOCATIONS`.

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
            let step_idx = ["step_index", "stepIdx", "idx"]
                .into_iter()
                .find_map(|k| val.get(k).and_then(Value::as_u64))
                .unwrap_or(fallback_idx);
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

            if target_step_idx == Some(step_idx) && body.is_some() {
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
        let mut file = File::open(path).ok()?;
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

        if target_step_idx.is_none() && file_len > TRANSCRIPT_TAIL_WINDOW_BYTES {
            let start = file_len - TRANSCRIPT_TAIL_WINDOW_BYTES;
            file.seek(SeekFrom::Start(start)).ok()?;
            let mut r = BufReader::new(file);
            let mut discard = Vec::new();
            let _ = CodexRolloutTailer::read_bounded_jsonl_record(
                &mut r,
                MAX_TRANSCRIPT_RECORD_BYTES,
                &mut discard,
            );
            let (body_opt, saw_step_record) = Self::scan_transcript_reader(&mut r, None, max_lines);
            if saw_step_record {
                return body_opt;
            }
            file = r.into_inner();
            file.seek(SeekFrom::Start(0)).ok()?;
        }

        let mut reader = BufReader::new(file);
        Self::scan_transcript_reader(&mut reader, target_step_idx, max_lines).0
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
                let term_behavior = if payload
                    .get("forceContinue")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                    || payload.get("terminationBehavior").and_then(Value::as_str)
                        == Some("force_continue")
                {
                    "force_continue"
                } else {
                    ""
                };
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
                let decision = payload
                    .get("decision")
                    .and_then(Value::as_str)
                    .filter(|d| matches!(*d, "allow" | "deny" | "ask"))
                    .unwrap_or("allow");
                if let Some(overwrite) = payload
                    .get("overwrite")
                    .filter(|v| v.as_object().is_some_and(|o| !o.is_empty()))
                {
                    json!({
                        "decision": decision,
                        "overwrite": overwrite
                    })
                } else {
                    json!({
                        "decision": decision
                    })
                }
            }
            "PostToolUse" | "post-tool-use" => {
                let tool_name = payload
                    .get("toolCall")
                    .and_then(|tc| tc.get("name"))
                    .or_else(|| payload.get("tool_name"))
                    .or_else(|| payload.get("toolName"))
                    .and_then(Value::as_str)
                    .unwrap_or("agy_tool");
                if InfinityContextMmu::is_icx_pager_tool(tool_name) {
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
                let reason = payload
                    .get("terminationReason")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let cont_active = payload
                    .get("continuationActive")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);

                if !cont_active
                    && (reason == "MAX_TOKEN_BUDGET_EXCEEDED" || reason == "MAX_INVOCATIONS")
                {
                    mmu.fold_all_open_subagents(AgentKind::AntigravityCli, "budget_rollover");
                    let ep_id = mmu.force_rollover_and_sync_dag_root();
                    let ep_handle = ep_id
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| "#icx:0000000000000000".to_owned());
                    let tail = Self::attach_conversation_id(
                        mmu.build_ephemeral_tail_slot(
                            AgentKind::AntigravityCli,
                            "epoch_rollover",
                            3,
                        ),
                        &payload,
                    );
                    json!({
                        "decision": "continue",
                        "reason": format!(
                            "⟪ICX:EPOCH_FOLDED│{ep_handle}│Resume from sealed AHTL state⟫\n⟪ICX:EPOCH_ROLLOVER⟫\n{tail}"
                        )
                    })
                } else {
                    json!({
                        "decision": "stop"
                    })
                }
            }
            "SessionEnd" | "session-end" => {
                mmu.fold_all_open_subagents(AgentKind::AntigravityCli, "session_ended");
                let _ = mmu.rollover_and_sync_dag_root();
                json!({})
            }
            _ => json!({}),
        };

        serde_json::to_string(&response)
    }
}
