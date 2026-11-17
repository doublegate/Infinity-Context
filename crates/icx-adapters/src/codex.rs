//! OpenAI Codex CLI (`0.169.0+`) native lifecycle hook adapter (`CodexAdapter`).
//!
//! Strictly conforms to Codex's embedded `additionalProperties: false` JSON schemas
//! (`session-start.command.output`, `user-prompt-submit.command.output`,
//! `post-tool-use.command.output`, `pre-compact.command.output`).

use crate::mmu::InfinityContextMmu;
use icx_core::AgentKind;
use serde_json::{Value, json};

/// Native hook adapter for OpenAI Codex CLI (`0.169.0+`).
#[derive(Clone, Debug, Default)]
pub struct CodexAdapter;

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
                        "additionalContext": tail
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
                let res = mmu.ingest_payload_with_exit_code(
                    AgentKind::Codex,
                    tool_name,
                    &raw_output,
                    exit_code,
                    false,
                );

                json!({
                    "continue": true,
                    "hookSpecificOutput": {
                        "hookEventName": "PostToolUse",
                        "additionalContext": res.compressed_ahtl
                    },
                    "reason": null,
                    "stopReason": null,
                    "suppressOutput": false,
                    "systemMessage": null
                })
            }
            "PreCompact" | "pre-compact" | "PostCompact" | "post-compact" => {
                let _ = mmu.epochs.maybe_rollover(&mut mmu.dag);
                let win = mmu.assemble_prompt_window(AgentKind::Codex, "compact_checkpoint");
                json!({
                    "continue": true,
                    "stopReason": null,
                    "suppressOutput": false,
                    "systemMessage": win.deterministic_prefix()
                })
            }
            _ => json!({
                "continue": true
            }),
        };

        serde_json::to_string(&response)
    }
}
