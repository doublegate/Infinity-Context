//! Cursor (`cursor-agent` and the editor) lifecycle hook adapter (`CursorAdapter`, v1.4.0,
//! T-63-002).
//!
//! Built from the documented contract (cursor.com/docs/hooks, read 2026-11-22). The live
//! capture is pending (see the Sprint 3 research notes), so the MCP replacement path is
//! opt-in until it is verified.
//!
//! - Every hook receives `conversation_id`, `generation_id`, `hook_event_name`,
//!   `workspace_roots`, `transcript_path` and model fields. Hooks are configured in
//!   `~/.cursor/hooks.json` or a project's `.cursor/hooks.json` (`{"version": 1, "hooks": ...}`).
//! - `sessionStart` accepts `additional_context`, added to the conversation's initial system
//!   context. It runs fire-and-forget, so the context may arrive after the first request.
//! - `postToolUse` receives `tool_name`, `tool_input` and `tool_output`, a JSON-stringified
//!   result payload, not raw terminal text. It can return `updated_mcp_tool_output` (MCP tools
//!   only), which replaces the output the model sees, and `additional_context`. icx ingests every
//!   result. It replaces MCP results with a stub only in the opt-in mode `ICX_CURSOR_STUB=1`,
//!   until that is verified live.
//! - `postToolUseFailure` is ingested as a failure. `preCompact` is observational and only seals
//!   the epoch. `sessionEnd` folds open subagent branches and seals.
//! - `beforeSubmitPrompt` can only allow or block a prompt, so icx does not register it.

use crate::mmu::{ICX_AHTL_LEGEND, InfinityContextMmu};
use crate::{AdapterError, AgentHookAdapter};
use icx_core::AgentKind;
use serde_json::{Value, json};

/// Native hook adapter for Cursor.
#[derive(Clone, Debug, Default)]
pub struct CursorAdapter;

impl AgentHookAdapter for CursorAdapter {
    fn agent_kind(&self) -> AgentKind {
        AgentKind::Cursor
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

/// `(text, exit code)` of a Cursor `tool_output`: a JSON string whose `stdout`/`stderr`,
/// `output`, `content` or `text` is the result. MCP `content` blocks are joined by their
/// `text`. Anything else is kept verbatim.
fn cursor_tool_text(output: Option<&Value>) -> (String, Option<i32>) {
    let Some(output) = output.filter(|v| !v.is_null()) else {
        return (String::new(), None);
    };
    let parsed = match output {
        Value::String(s) => serde_json::from_str::<Value>(s).ok(),
        Value::Object(_) => Some(output.clone()),
        _ => None,
    };
    let Some(Value::Object(obj)) = parsed else {
        return (crate::claude::tool_output_text(Some(output)), None);
    };
    let code = ["exitCode", "exit_code"]
        .into_iter()
        .find_map(|k| obj.get(k).and_then(Value::as_i64))
        .and_then(|n| i32::try_from(n).ok());
    let streams: Vec<&str> = ["stdout", "stderr"]
        .into_iter()
        .filter_map(|k| obj.get(k).and_then(Value::as_str))
        .filter(|t| !t.is_empty())
        .collect();
    if !streams.is_empty() {
        return (streams.join("\n"), code);
    }
    for key in ["output", "text", "result"] {
        if let Some(Value::String(s)) = obj.get(key)
            && !s.is_empty()
        {
            return (s.clone(), code);
        }
    }
    if let Some(Value::Array(blocks)) = obj.get("content") {
        let texts: Vec<&str> = blocks
            .iter()
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .collect();
        if !texts.is_empty() {
            return (texts.join("\n"), code);
        }
    }
    (Value::Object(obj).to_string(), code)
}

/// Whether a Cursor `tool_name` names an MCP tool. Cursor's MCP name form is still to be
/// confirmed by the live capture, so the forms other agents use are all accepted.
fn is_mcp_tool(tool_name: &str, payload: &Value) -> bool {
    let t = tool_name.trim();
    payload.get("mcp_server_name").is_some()
        || t.starts_with("mcp__")
        || t.starts_with("mcp_")
        || t.to_ascii_lowercase().starts_with("mcp:")
}

impl CursorAdapter {
    /// Cursor documents `updated_mcp_tool_output` as a replacement for MCP tools; until it is
    /// verified live, replacement is off unless [`Self::stub_mode_enabled`].
    pub const SUPPORTS_TOOL_OUTPUT_REPLACEMENT: bool = false;

    /// Whether the opt-in MCP stub mode is on (`ICX_CURSOR_STUB=1`).
    pub fn stub_mode_enabled() -> bool {
        std::env::var("ICX_CURSOR_STUB").is_ok_and(|v| v.trim() == "1")
    }

    /// Handles a Cursor hook event JSON payload and returns the reply JSON.
    pub fn handle_hook(
        event_name: &str,
        stdin_json: &str,
        mmu: &mut InfinityContextMmu,
    ) -> Result<String, serde_json::Error> {
        Self::handle_hook_with_stub_mode(event_name, stdin_json, mmu, Self::stub_mode_enabled())
    }

    /// [`Self::handle_hook`] with the stub mode given explicitly (for tests).
    pub fn handle_hook_with_stub_mode(
        event_name: &str,
        stdin_json: &str,
        mmu: &mut InfinityContextMmu,
        stub_mode: bool,
    ) -> Result<String, serde_json::Error> {
        let payload: Value = if stdin_json.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(stdin_json)?
        };
        let response = match event_name {
            "sessionStart" | "SessionStart" | "session-start" => {
                let tail = mmu.build_ephemeral_tail_slot(AgentKind::Cursor, "session_start", 3);
                json!({ "additional_context": format!("{ICX_AHTL_LEGEND}\n{tail}") })
            }
            "postToolUse"
            | "PostToolUse"
            | "post-tool-use"
            | "postToolUseFailure"
            | "PostToolUseFailure"
            | "post-tool-use-failure" => {
                let failure = event_name.to_ascii_lowercase().contains("failure");
                let tool_name = payload
                    .get("tool_name")
                    .and_then(Value::as_str)
                    .unwrap_or("cursor_tool");
                if InfinityContextMmu::is_icx_pager_tool(tool_name) {
                    json!({})
                } else {
                    let subject = InfinityContextMmu::extract_hook_subject(&payload, "cursor_tool");
                    let (mut text, code) = cursor_tool_text(payload.get("tool_output"));
                    if failure {
                        if let Some(e) = ["error_message", "error", "message"]
                            .into_iter()
                            .find_map(|k| payload.get(k).and_then(Value::as_str))
                            .filter(|e| !e.trim().is_empty())
                        {
                            if !text.is_empty() {
                                text.push('\n');
                            }
                            text.push_str(e);
                        }
                    }
                    let exit_code = code.or(failure.then_some(1));
                    let res = mmu.ingest_payload_for_subagent(
                        AgentKind::Cursor,
                        &subject,
                        &text,
                        exit_code,
                        failure,
                        None,
                    );
                    if stub_mode
                        && !failure
                        && res.was_virtualized
                        && is_mcp_tool(tool_name, &payload)
                    {
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
                        // An MCP `CallToolResult`, the shape the replaced output takes.
                        json!({
                            "updated_mcp_tool_output": {
                                "content": [{ "type": "text", "text": stub }]
                            }
                        })
                    } else {
                        json!({})
                    }
                }
            }
            "preCompact" | "PreCompact" | "pre-compact" => {
                let _ = mmu.rollover_and_sync_dag_root();
                json!({})
            }
            "sessionEnd" | "SessionEnd" | "session-end" => {
                mmu.fold_all_open_subagents(AgentKind::Cursor, "session_end");
                let _ = mmu.rollover_and_sync_dag_root();
                json!({})
            }
            _ => json!({}),
        };
        serde_json::to_string(&response)
    }
}
