//! Meta Muse Code (`1.4.25+`) native lifecycle hook adapter (`MuseAdapter`, v1.3.0).
//!
//! Contract observed on Muse 1.4.25 with probe hooks (see `docs/agent-adapters-and-interception.md`):
//!
//! - Muse runs Claude-Code-style command hooks from `~/.config/muse/settings.json` in the project
//!   directory and with the caller's environment, passing Claude-Code-shaped JSON on stdin
//!   (`hook_event_name`, `session_id`, `tool_name`, `tool_input`, `tool_response`) plus `turn_id`,
//!   `model` and `model_provider`.
//! - `hookSpecificOutput.additionalContext` from SessionStart, UserPromptSubmit and PostToolUse
//!   reaches the model; `hookSpecificOutput` must name its `hookEventName`.
//! - Muse rejects replacement ("PostToolUse output cannot replace tool output"), so this adapter
//!   captures tool output for recall/deref and never claims to shrink it. A reply's
//!   `systemMessage` is shown to the user, not the model, so it is never used.
//! - Muse validates replies strictly, so every other event gets `{}` and no reply carries a
//!   JSON `null`.
//!
//! The Bash tool's `tool_response` is a JSON string (`output`, `exit_code`, `truncated`, ...);
//! its `output` text is what is stored. Other tools send plain text.

use crate::mmu::{ICX_AHTL_LEGEND, InfinityContextMmu};
use crate::{AdapterError, AgentHookAdapter};
use icx_core::AgentKind;
use serde_json::{Value, json};

/// Native hook adapter for Meta Muse Code.
#[derive(Clone, Debug, Default)]
pub struct MuseAdapter;

impl AgentHookAdapter for MuseAdapter {
    fn agent_kind(&self) -> AgentKind {
        AgentKind::Muse
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

/// `(text, exit code)` of a tool result. Only the Bash tool's JSON-encoded `tool_response` string
/// is unwrapped to its `output`; any other tool's text is kept verbatim, even when it happens to be
/// JSON with an `output` member (PR #27 review).
fn unwrap_muse_tool_response(tool_name: &str, response: Option<&Value>) -> (String, Option<i32>) {
    let Some(response) = response.filter(|v| !v.is_null()) else {
        return (String::new(), None);
    };
    if tool_name.eq_ignore_ascii_case("bash") {
        // A JSON-encoded string (observed on 1.4.25) or an inline JSON object.
        let parsed = match response {
            Value::String(text) => serde_json::from_str::<Value>(text).ok(),
            Value::Object(_) => Some(response.clone()),
            _ => None,
        };
        if let Some(Value::Object(obj)) = parsed {
            // The exit code is kept even when a failed command has no `output`.
            let code = obj
                .get("exit_code")
                .or_else(|| obj.get("exitCode"))
                .and_then(Value::as_i64)
                .and_then(|n| i32::try_from(n).ok());
            match obj.get("output") {
                Some(Value::String(output)) => return (output.clone(), code),
                None | Some(Value::Null) if code.is_some() => return (String::new(), code),
                _ => {}
            }
        }
    }
    (crate::claude::tool_output_text(Some(response)), None)
}

/// The subagent identity of a Muse payload: `subagent_id`, else `child_session_id`, else
/// `agent_id`, so SubagentStart, tool events and SubagentStop agree on one branch (PR #27 review).
fn subagent_id(payload: &Value) -> Option<&str> {
    ["subagent_id", "child_session_id", "agent_id"]
        .into_iter()
        .find_map(|k| payload.get(k).and_then(Value::as_str))
        .filter(|s| !s.trim().is_empty())
}

impl MuseAdapter {
    /// Muse hooks capture tool output but cannot replace it (Muse rejects replacement).
    pub const SUPPORTS_TOOL_OUTPUT_REPLACEMENT: bool = false;

    /// Handles a Muse lifecycle hook event JSON payload and returns the reply JSON.
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
                let tail = mmu.build_ephemeral_tail_slot(AgentKind::Muse, "session_start", 3);
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
                    .unwrap_or("muse_query");
                let tail = mmu.build_ephemeral_tail_slot(AgentKind::Muse, prompt, 4);
                json!({
                    "hookSpecificOutput": {
                        "hookEventName": "UserPromptSubmit",
                        "additionalContext": tail
                    }
                })
            }
            "PostToolUse" | "post-tool-use" | "PostToolUseFailure" | "post-tool-use-failure" => {
                let failure = matches!(event_name, "PostToolUseFailure" | "post-tool-use-failure");
                let tool_name = payload
                    .get("tool_name")
                    .or_else(|| payload.get("toolName"))
                    .and_then(Value::as_str)
                    .unwrap_or("muse_tool");
                if !InfinityContextMmu::is_icx_pager_tool(tool_name) {
                    let subject = InfinityContextMmu::extract_hook_subject(&payload, "muse_tool");
                    let (mut text, code) = unwrap_muse_tool_response(
                        tool_name,
                        payload
                            .get("tool_response")
                            .or_else(|| payload.get("toolResponse")),
                    );
                    if failure {
                        // Keep the error next to any partial output (as AntigravityAdapter does).
                        // A string, or a structured error (`{"error":{"message":...}}`) as JSON.
                        let error_owned =
                            ["error", "message", "reason"]
                                .into_iter()
                                .find_map(|k| match payload.get(k)? {
                                    Value::String(e) if !e.trim().is_empty() => Some(e.clone()),
                                    v @ Value::Object(o) if !o.is_empty() => Some(v.to_string()),
                                    _ => None,
                                });
                        let error = error_owned.as_deref();
                        match (text.is_empty(), error) {
                            (true, e) => e.unwrap_or("tool call failed").clone_into(&mut text),
                            (false, Some(e)) => {
                                text.push('\n');
                                text.push_str(e);
                            }
                            (false, None) => {}
                        }
                    }
                    let exit_code = code
                        .or_else(|| InfinityContextMmu::extract_exit_code(&payload))
                        .or(failure.then_some(1));
                    let sub = subagent_id(&payload);
                    let _ = mmu.ingest_payload_for_subagent(
                        AgentKind::Muse,
                        &subject,
                        &text,
                        exit_code,
                        failure,
                        sub,
                    );
                }
                if failure {
                    json!({})
                } else {
                    // Capture only: no replacement field and no second copy of the output.
                    json!({ "hookSpecificOutput": { "hookEventName": "PostToolUse" } })
                }
            }
            "SubagentStart" | "subagent-start" => {
                let _ = mmu.begin_subagent(subagent_id(&payload).unwrap_or("muse_subagent"));
                json!({})
            }
            "SubagentStop" | "subagent-stop" => {
                let id = subagent_id(&payload);
                let summary: String = ["last_assistant_message", "summary", "outcome"]
                    .into_iter()
                    .find_map(|k| payload.get(k).and_then(Value::as_str))
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or("subagent_completed")
                    .chars()
                    .take(240)
                    .collect();
                let _ = mmu.finish_subagent(AgentKind::Muse, id, &summary);
                json!({})
            }
            "PreCompact" | "pre-compact" => {
                let _ = mmu.rollover_and_sync_dag_root();
                json!({})
            }
            "PostCompact" | "post-compact" => {
                // Keep a compaction summary recoverable when Muse provides one (Claude-Code shape).
                if let Some(summary) = payload
                    .get("compact_summary")
                    .and_then(Value::as_str)
                    .filter(|s| !s.trim().is_empty())
                {
                    let _ = mmu.ingest_payload(AgentKind::Muse, "compact_summary", summary, false);
                }
                let _ = mmu.rollover_and_sync_dag_root();
                json!({})
            }
            "SessionEnd" | "session-end" => {
                mmu.fold_all_open_subagents(AgentKind::Muse, "session_end");
                let _ = mmu.rollover_and_sync_dag_root();
                json!({})
            }
            _ => json!({}),
        };
        serde_json::to_string(&response)
    }
}
