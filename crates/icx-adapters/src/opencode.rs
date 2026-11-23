//! OpenCode V2 plugin adapter (`OpenCodeAdapter`, v1.4.0, T-63-003).
//!
//! OpenCode v2.2.7 loads V2 plugins only (`{ id, setup(ctx) }`). The plugin in
//! `integrations/opencode/icx.ts` calls `icx hook --agent opencode --event <EVENT>` with a small,
//! stable JSON payload it builds from each hook event, and applies the reply. It fails open: if
//! icx is missing, errors or times out, OpenCode runs unchanged.
//!
//! Events (the plugin's names, not OpenCode's):
//!
//! - `prompt`, from `ctx.session.hook("prompt")`: icx returns the legend and the tail slot for
//!   the prompt (`{"append": ...}`). The plugin appends it to the user's message, so icx context
//!   reaches the model as user-turn data.
//! - The `context` hook (system text on every request) is deliberately not used. icx context is
//!   untrusted retrieved data and never goes into the system prompt, and per-request system text
//!   would also defeat the provider's prompt cache. A `context` event is answered with `{}`.
//! - `execute.after`, from `ctx.tool.hook("execute.after")`: the tool result
//!   (`{tool, call_id, input, status, output, error}`) is ingested. In the opt-in stub mode
//!   (`ICX_OPENCODE_STUB=1`), a completed result over the stub threshold gets
//!   `{"replace": stub}`, which the plugin writes into `event.result`. Off by default until the
//!   replacement is verified live.
//! - `compaction`, from `ctx.session.hook("compaction")`: seals the epoch. icx never supplies the
//!   summary, so OpenCode still writes its own.
//! - `session.end`: folds open subagent branches and seals.

use crate::mmu::{ICX_AHTL_LEGEND, InfinityContextMmu};
use crate::{AdapterError, AgentHookAdapter};
use icx_core::AgentKind;
use serde_json::{Value, json};

/// Native plugin adapter for OpenCode (V2 plugin API).
#[derive(Clone, Debug, Default)]
pub struct OpenCodeAdapter;

impl AgentHookAdapter for OpenCodeAdapter {
    fn agent_kind(&self) -> AgentKind {
        AgentKind::OpenCode
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

/// Text of an OpenCode tool result as the plugin forwards it: a string, or the `text` of each
/// content part, else the JSON.
fn opencode_result_text(output: Option<&Value>) -> String {
    match output {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(parts)) => {
            let texts: Vec<&str> = parts
                .iter()
                .filter_map(|p| p.get("text").and_then(Value::as_str).or_else(|| p.as_str()))
                .collect();
            if texts.is_empty() {
                Value::Array(parts.clone()).to_string()
            } else {
                texts.join("\n")
            }
        }
        Some(other) => crate::claude::tool_output_text(Some(other)),
    }
}

impl OpenCodeAdapter {
    /// `event.result` replacement is documented for V2 `execute.after` but not yet verified on
    /// the installed OpenCode, so it is off unless [`Self::stub_mode_enabled`].
    pub const SUPPORTS_TOOL_OUTPUT_REPLACEMENT: bool = false;

    /// Whether the opt-in stub mode is on (`ICX_OPENCODE_STUB=1`).
    pub fn stub_mode_enabled() -> bool {
        std::env::var("ICX_OPENCODE_STUB").is_ok_and(|v| v.trim() == "1")
    }

    /// Handles an OpenCode plugin event JSON payload and returns the reply JSON.
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
            "prompt" => {
                let prompt = payload
                    .get("prompt")
                    .and_then(Value::as_str)
                    .filter(|p| !p.trim().is_empty())
                    .unwrap_or("opencode_query");
                let tail = mmu.build_ephemeral_tail_slot(AgentKind::OpenCode, prompt, 4);
                json!({ "append": format!("{ICX_AHTL_LEGEND}\n{tail}") })
            }
            "execute.after" => {
                let tool = payload
                    .get("tool")
                    .and_then(Value::as_str)
                    .unwrap_or("opencode_tool");
                if InfinityContextMmu::is_icx_pager_tool(tool)
                    || tool.eq_ignore_ascii_case("icx_icx")
                {
                    json!({})
                } else {
                    let failed = payload.get("status").and_then(Value::as_str) == Some("error");
                    let mut text = opencode_result_text(payload.get("output"));
                    if let Some(e) = payload
                        .get("error")
                        .and_then(Value::as_str)
                        .filter(|e| !e.trim().is_empty())
                    {
                        if !text.is_empty() {
                            text.push('\n');
                        }
                        text.push_str(e);
                    }
                    let subject = InfinityContextMmu::extract_hook_subject(
                        &json!({ "tool_name": tool, "tool_input": payload.get("input") }),
                        "opencode_tool",
                    );
                    let exit_code =
                        InfinityContextMmu::extract_exit_code(&payload).or(failed.then_some(1));
                    let res = mmu.ingest_payload_for_subagent(
                        AgentKind::OpenCode,
                        &subject,
                        &text,
                        exit_code,
                        failed,
                        None,
                    );
                    if stub_mode && !failed && res.was_virtualized {
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
                        json!({ "replace": stub })
                    } else {
                        json!({})
                    }
                }
            }
            "compaction" => {
                let _ = mmu.rollover_and_sync_dag_root();
                json!({})
            }
            "session.end" => {
                mmu.fold_all_open_subagents(AgentKind::OpenCode, "session_end");
                let _ = mmu.rollover_and_sync_dag_root();
                json!({})
            }
            _ => json!({}),
        };
        serde_json::to_string(&response)
    }
}
