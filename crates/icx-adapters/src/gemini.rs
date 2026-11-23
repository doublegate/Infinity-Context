//! Google Gemini CLI lifecycle hook adapter (`GeminiCliAdapter`, v1.4.0, T-63-001).
//!
//! Built from the documented contract (`google-gemini/gemini-cli` `docs/hooks/reference.md` and
//! `packages/core/src/hooks/types.ts`). **Gemini CLI is not installed on the reference machine
//! (owner decision, 2026-11-22), so nothing here is verified live.**
//!
//! - Every hook receives `session_id`, `transcript_path`, `cwd`, `hook_event_name` and
//!   `timestamp` on stdin, and answers with JSON on stdout (exit code 0).
//! - `SessionStart` and `BeforeAgent` accept `hookSpecificOutput.additionalContext`: the first
//!   turn (interactive) or prepended to the prompt (non-interactive), and appended to the prompt
//!   for that turn. Both carry the icx tail slot.
//! - `AfterTool` receives `tool_name`, `tool_input` and `tool_response`
//!   (`{llmContent, returnDisplay, error?}`). It is ingested for recall. The documented way to
//!   replace a result is `decision: "deny"` with a `reason`: the reason replaces the result the
//!   model sees, and the turn continues. icx uses it only in the opt-in stub mode
//!   (`ICX_GEMINI_STUB=1`), because the replacement is unverified. By default no reply changes a
//!   result.
//! - `PreCompress` is advisory (asynchronous, cannot block or modify compression), so it only
//!   seals the epoch. `SessionEnd` folds open subagent branches and seals.

use crate::mmu::{ICX_AHTL_LEGEND, InfinityContextMmu};
use crate::{AdapterError, AgentHookAdapter};
use icx_core::AgentKind;
use serde_json::{Value, json};

/// Native hook adapter for Google Gemini CLI.
#[derive(Clone, Debug, Default)]
pub struct GeminiCliAdapter;

impl AgentHookAdapter for GeminiCliAdapter {
    fn agent_kind(&self) -> AgentKind {
        AgentKind::GeminiCli
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

/// Text of a Gemini `tool_response`: `llmContent` as a string, or the `text` of each part when it
/// is a part list, else `returnDisplay`; an `error` is appended. Anything else is kept as JSON.
fn gemini_tool_text(response: Option<&Value>) -> (String, bool) {
    let Some(response) = response.filter(|v| !v.is_null()) else {
        return (String::new(), false);
    };
    let Value::Object(obj) = response else {
        return (crate::claude::tool_output_text(Some(response)), false);
    };
    let parts_text = |v: &Value| -> Option<String> {
        match v {
            Value::String(s) => Some(s.clone()),
            // A part list with any non-text part (inline data, a file) falls through to the
            // whole JSON, so nothing is dropped.
            Value::Array(parts) => crate::text_only_parts(parts),
            Value::Object(o) => o.get("text").and_then(Value::as_str).map(str::to_owned),
            _ => None,
        }
    };
    let mut text = obj
        .get("llmContent")
        .and_then(parts_text)
        .or_else(|| obj.get("returnDisplay").and_then(parts_text))
        .unwrap_or_else(|| response.to_string());
    let error = match obj.get("error") {
        Some(Value::String(e)) if !e.trim().is_empty() => Some(e.clone()),
        Some(Value::Object(e)) => e
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| Some(Value::Object(e.clone()).to_string())),
        _ => None,
    };
    let failed = error.is_some();
    if let Some(e) = error {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&e);
    }
    (text, failed)
}

impl GeminiCliAdapter {
    /// Gemini CLI documents `AfterTool` `decision: "deny"` as a result replacement, but it is not
    /// verified live, so replacement is off unless [`Self::stub_mode_enabled`].
    pub const SUPPORTS_TOOL_OUTPUT_REPLACEMENT: bool = false;

    /// Whether the opt-in `AfterTool` stub mode is on (`ICX_GEMINI_STUB=1`).
    pub fn stub_mode_enabled() -> bool {
        std::env::var("ICX_GEMINI_STUB").is_ok_and(|v| v.trim() == "1")
    }

    /// Handles a Gemini CLI hook event JSON payload and returns the reply JSON.
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
            "SessionStart" | "session-start" => {
                let tail = mmu.build_ephemeral_tail_slot(AgentKind::GeminiCli, "session_start", 3);
                json!({
                    "hookSpecificOutput": {
                        "hookEventName": "SessionStart",
                        "additionalContext": format!("{ICX_AHTL_LEGEND}\n{tail}")
                    }
                })
            }
            "BeforeAgent" | "before-agent" => {
                let prompt = payload
                    .get("prompt")
                    .and_then(Value::as_str)
                    .unwrap_or("gemini_query");
                let tail = mmu.build_ephemeral_tail_slot(AgentKind::GeminiCli, prompt, 4);
                json!({
                    "hookSpecificOutput": {
                        "hookEventName": "BeforeAgent",
                        "additionalContext": tail
                    }
                })
            }
            "AfterTool" | "after-tool" => {
                let tool_name = payload
                    .get("tool_name")
                    .and_then(Value::as_str)
                    .unwrap_or("gemini_tool");
                if InfinityContextMmu::is_icx_pager_tool(tool_name) {
                    json!({})
                } else {
                    let subject = InfinityContextMmu::extract_hook_subject(&payload, "gemini_tool");
                    let (text, failed) = gemini_tool_text(payload.get("tool_response"));
                    let exit_code =
                        InfinityContextMmu::extract_exit_code(&payload).or(failed.then_some(1));
                    let res = mmu.ingest_payload_for_subagent(
                        AgentKind::GeminiCli,
                        &subject,
                        &text,
                        exit_code,
                        failed,
                        None,
                    );
                    if stub_mode && res.was_virtualized && !failed {
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
                        // Documented: the reason replaces the result the model sees; the turn
                        // continues.
                        json!({ "decision": "deny", "reason": stub })
                    } else {
                        json!({})
                    }
                }
            }
            "PreCompress" | "pre-compress" => {
                let _ = mmu.rollover_and_sync_dag_root();
                json!({})
            }
            "SessionEnd" | "session-end" => {
                mmu.fold_all_open_subagents(AgentKind::GeminiCli, "session_end");
                let _ = mmu.rollover_and_sync_dag_root();
                json!({})
            }
            _ => json!({}),
        };
        serde_json::to_string(&response)
    }
}
