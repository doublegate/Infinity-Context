//! Claude Code (`2.1.326+`) native lifecycle hook adapter (`ClaudeCodeAdapter`).
//!
//! Exploits `PostToolUse` `hookSpecificOutput.updatedToolOutput` to replace bulky tool outputs
//! in-flight before LLM ingestion, `UserPromptSubmit`/`SessionStart` `additionalContext`, and
//! `PreCompact`/`PostCompact` lossless Merkle-DAG state preservation.

use crate::mmu::InfinityContextMmu;
use icx_core::AgentKind;
use serde_json::{json, Value};

/// Native hook adapter for Claude Code (`2.1.326+`).
#[derive(Clone, Debug, Default)]
pub struct ClaudeCodeAdapter;

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
                        "additionalContext": tail
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
            "PostToolUse" | "post-tool-use" => {
                let tool_name = payload
                    .get("tool_name")
                    .and_then(Value::as_str)
                    .unwrap_or("Tool");
                let raw_output = match payload.get("tool_response") {
                    Some(Value::String(s)) => s.clone(),
                    Some(other) => other.to_string(),
                    None => String::new(),
                };

                let res = mmu.ingest_payload(AgentKind::ClaudeCode, tool_name, &raw_output, false);
                if res.was_virtualized {
                    let stub = format!(
                        "⟪ICX:STUB│{}│raw={}B→{}B⟫\n{}",
                        res.artifact_id,
                        res.raw_bytes,
                        res.compressed_ahtl.len(),
                        res.compressed_ahtl
                    );
                    json!({
                        "hookSpecificOutput": {
                            "hookEventName": "PostToolUse",
                            "updatedToolOutput": stub
                        }
                    })
                } else {
                    json!({})
                }
            }
            "PreCompact" | "pre-compact" => {
                let _ = mmu.epochs.maybe_rollover(&mut mmu.dag);
                let tail = mmu.build_ephemeral_tail_slot(AgentKind::ClaudeCode, "pre_compact", 5);
                json!({
                    "systemMessage": tail
                })
            }
            "PostCompact" | "post-compact" => {
                let win = mmu.assemble_prompt_window(AgentKind::ClaudeCode, "post_compact");
                json!({
                    "systemMessage": win.deterministic_prefix()
                })
            }
            _ => json!({}),
        };

        serde_json::to_string(&response)
    }
}
