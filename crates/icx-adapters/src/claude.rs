//! Claude Code (`2.1.326+`) native lifecycle hook adapter (`ClaudeCodeAdapter`).
//!
//! Exploits `PostToolUse` `hookSpecificOutput.updatedToolOutput` to replace bulky tool outputs
//! in-flight before LLM ingestion, `UserPromptSubmit`/`SessionStart` `additionalContext`, and
//! `PreCompact`/`PostCompact` lossless Merkle-DAG state preservation.

use crate::mmu::{ICX_AHTL_LEGEND, InfinityContextMmu};
use crate::{AdapterError, AgentHookAdapter};
use icx_core::AgentKind;
use serde_json::{Value, json};

/// Native hook adapter for Claude Code (`2.1.326+`).
#[derive(Clone, Debug, Default)]
pub struct ClaudeCodeAdapter;

impl AgentHookAdapter for ClaudeCodeAdapter {
    fn agent_kind(&self) -> AgentKind {
        AgentKind::ClaudeCode
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

impl ClaudeCodeAdapter {
    /// Handles a Claude Code lifecycle hook event JSON payload and returns the wire JSON response.
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
                let tail = mmu.build_ephemeral_tail_slot(AgentKind::ClaudeCode, "session_init", 3);
                json!({
                    "hookSpecificOutput": {
                        "hookEventName": "SessionStart",
                        "additionalContext": format!("{ICX_AHTL_LEGEND}\n{tail}")
                    }
                })
            }
            "UserPromptSubmit" | "user-prompt-submit" => {
                let prompt = payload
                    .get("prompt")
                    .and_then(Value::as_str)
                    .unwrap_or("user_query");
                let tail = mmu.build_ephemeral_tail_slot(AgentKind::ClaudeCode, prompt, 4);
                json!({
                    "hookSpecificOutput": {
                        "hookEventName": "UserPromptSubmit",
                        "additionalContext": tail
                    }
                })
            }
            "PreToolUse" | "pre-tool-use" => {
                let mut hso = json!({
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "allow"
                });
                if let Some(updated) = payload.get("updatedInput").filter(|v| !v.is_null()) {
                    hso["updatedInput"] = updated.clone();
                }
                json!({
                    "hookSpecificOutput": hso
                })
            }
            "PostToolUse" | "post-tool-use" => {
                let tool_name = payload
                    .get("tool_name")
                    .and_then(Value::as_str)
                    .unwrap_or("Tool");
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
                    AgentKind::ClaudeCode,
                    tool_name,
                    &raw_output,
                    exit_code,
                    false,
                    explicit_sub_id.as_deref(),
                );
                if res.was_virtualized {
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
                    let mut hso = json!({
                        "hookEventName": "PostToolUse",
                        "updatedToolOutput": stub
                    });
                    if tool_name.starts_with("mcp__") {
                        hso["updatedMCPToolOutput"] = Value::String(stub);
                    }
                    json!({
                        "hookSpecificOutput": hso
                    })
                } else {
                    json!({})
                }
            }
            "SubagentStart" | "subagent-start" => {
                let raw_subagent_id = ["subagent_id", "agent_id"]
                    .into_iter()
                    .find_map(|k| payload.get(k).and_then(Value::as_str))
                    .unwrap_or("claude_subagent");
                let subagent_id = mmu.begin_subagent(raw_subagent_id);
                let tail = mmu.build_ephemeral_tail_slot(AgentKind::ClaudeCode, &subagent_id, 3);
                json!({
                    "hookSpecificOutput": {
                        "hookEventName": "SubagentStart",
                        "additionalContext": tail
                    }
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
                let folded_msg = mmu.finish_subagent(AgentKind::ClaudeCode, subagent_id, summary);
                json!({
                    "systemMessage": folded_msg
                })
            }
            "PreCompact" | "pre-compact" => {
                let _ = mmu.rollover_and_sync_dag_root();
                let tail = mmu.build_ephemeral_tail_slot(AgentKind::ClaudeCode, "pre_compact", 5);
                json!({
                    "systemMessage": tail
                })
            }
            "PostCompact" | "post-compact" => {
                let _ = mmu.rollover_and_sync_dag_root();
                let win = mmu.assemble_prompt_window(AgentKind::ClaudeCode, "post_compact");
                json!({
                    "systemMessage": win.deterministic_prefix()
                })
            }
            "Stop" | "stop" | "SessionEnd" | "session-end" => {
                mmu.fold_all_open_subagents(AgentKind::ClaudeCode, "session_end");
                let _ = mmu.rollover_and_sync_dag_root();
                json!({})
            }
            _ => json!({}),
        };

        serde_json::to_string(&response)
    }
}
