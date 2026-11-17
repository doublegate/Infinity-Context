//! Google Antigravity CLI (`agy`) native lifecycle hook adapter (`AntigravityAdapter`).
//!
//! Exploits `SessionStart` and `PreInvocation` `injectSteps` with `metadata.user_markdown = ""`
//! so non-human `AHTL` frames are completely invisible in the human chat UI while injected directly
//! into the model's context, plus `Stop` continuation across `MAX_TOKEN_BUDGET_EXCEEDED`.

use crate::mmu::InfinityContextMmu;
use icx_core::AgentKind;
use serde_json::{Value, json};

/// Native hook adapter for Google Antigravity CLI (`agy`).
#[derive(Clone, Debug, Default)]
pub struct AntigravityAdapter;

impl AntigravityAdapter {
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
                let tail =
                    mmu.build_ephemeral_tail_slot(AgentKind::AntigravityCli, "session_init", 3);
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
                    ]
                })
            }
            "PreInvocation" | "pre-invocation" => {
                let last_input = payload
                    .get("lastUserInput")
                    .and_then(Value::as_str)
                    .unwrap_or("agy_turn");
                let tail = mmu.build_ephemeral_tail_slot(AgentKind::AntigravityCli, last_input, 4);
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
            "PreToolUse" | "pre-tool-use" => {
                json!({
                    "decision": "allow"
                })
            }
            "PostToolUse" | "post-tool-use" => {
                let tool_name = payload
                    .get("toolCall")
                    .and_then(|tc| tc.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or("agy_tool");
                let args_str = payload
                    .get("toolCall")
                    .and_then(|tc| tc.get("args"))
                    .filter(|v| !v.is_null())
                    .map(Value::to_string)
                    .unwrap_or_default();
                let resp_str = ["toolResponse", "tool_response", "toolResult", "result"]
                    .into_iter()
                    .find_map(|k| payload.get(k).filter(|v| !v.is_null()))
                    .map(|v| match v {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    })
                    .unwrap_or_default();
                let err_str = payload.get("error").and_then(Value::as_str).unwrap_or("");
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
                let _ = mmu.ingest_payload_with_exit_code(
                    AgentKind::AntigravityCli,
                    tool_name,
                    &combined,
                    exit_code,
                    !err_str.is_empty(),
                );
                json!({})
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
                    let _ = mmu.epochs.maybe_rollover(&mut mmu.dag);
                    let tail = mmu.build_ephemeral_tail_slot(
                        AgentKind::AntigravityCli,
                        "epoch_rollover",
                        3,
                    );
                    json!({
                        "decision": "continue",
                        "reason": format!("⟪ICX:EPOCH_ROLLOVER⟫\n{tail}")
                    })
                } else {
                    json!({
                        "decision": "stop"
                    })
                }
            }
            _ => json!({}),
        };

        serde_json::to_string(&response)
    }
}
