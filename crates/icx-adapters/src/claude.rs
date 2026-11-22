//! Claude Code native lifecycle hook adapter (`ClaudeCodeAdapter`).
//!
//! Contract (pinned in `tests/claude_contract.rs` against <https://code.claude.com/docs/en/hooks>,
//! Claude Code 2.1.328):
//! - `PostToolUse`: `hookSpecificOutput.updatedToolOutput` replaces bulky output *in the tool's own
//!   output shape* (built-in tools return objects; a mismatched shape is silently ignored), plus
//!   `updatedMCPToolOutput` (string) for MCP tools.
//! - `SessionStart` / `UserPromptSubmit` / `SubagentStart`: `additionalContext`.
//! - `PreCompact`: plain stdout becomes the compaction instructions (`systemMessage` is discarded).
//! - `PostCompact` / `SubagentStop` / `Stop`: side effects only; no model-facing output.

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
                // `permissionDecision` / `updatedInput` are output-only fields; the input never
                // carries them, so icx expresses no decision and leaves the tool input unchanged
                // (ICX-AUD-009: the former stdin echo was dead code).
                json!({
                    "hookSpecificOutput": { "hookEventName": "PreToolUse" }
                })
            }
            "PostToolUse" | "post-tool-use" => {
                let tool_name = payload
                    .get("tool_name")
                    .or_else(|| payload.get("toolName"))
                    .and_then(Value::as_str)
                    .unwrap_or("Tool");
                if InfinityContextMmu::is_icx_pager_tool(tool_name) {
                    json!({})
                } else {
                    let subject = InfinityContextMmu::extract_hook_subject(&payload, "Tool");
                    let tool_response = payload.get("tool_response").filter(|v| !v.is_null());
                    let raw_output = tool_output_text(tool_response);

                    let exit_code = InfinityContextMmu::extract_exit_code(&payload);
                    let explicit_sub_id = ["subagent_id", "agent_id"]
                        .into_iter()
                        .find_map(|k| payload.get(k).and_then(Value::as_str))
                        .map(str::to_owned);
                    let res = mmu.ingest_payload_for_subagent(
                        AgentKind::ClaudeCode,
                        &subject,
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
                        mmu.record_emitted(&stub);
                        let mut hso = json!({
                            "hookEventName": "PostToolUse",
                            "updatedToolOutput": shape_preserving_replacement(tool_response, &stub)
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
                // Upstream input field is `last_assistant_message` (older drafts: summary/outcome).
                let summary_raw = ["last_assistant_message", "summary", "outcome"]
                    .into_iter()
                    .find_map(|k| payload.get(k).and_then(Value::as_str))
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or("subagent_completed");
                let summary: String = summary_raw.chars().take(240).collect();
                let _ = mmu.finish_subagent(AgentKind::ClaudeCode, subagent_id, &summary);
                // Any additionalContext / decision would keep the subagent running.
                json!({})
            }
            "PreCompact" | "pre-compact" => {
                let _ = mmu.rollover_and_sync_dag_root();
                let tail = mmu.build_ephemeral_tail_slot(AgentKind::ClaudeCode, "pre_compact", 5);
                // Plain stdout is used verbatim as the compaction instructions (< 10,000 chars).
                return Ok(compaction_instructions(&tail));
            }
            "PostCompact" | "post-compact" => {
                if let Some(summary) = payload
                    .get("compact_summary")
                    .and_then(Value::as_str)
                    .filter(|s| !s.trim().is_empty())
                {
                    // Keep the compaction summary itself recoverable.
                    let _ = mmu.ingest_payload(
                        AgentKind::ClaudeCode,
                        "compact_summary",
                        summary,
                        false,
                    );
                }
                let _ = mmu.rollover_and_sync_dag_root();
                json!({})
            }
            "SessionEnd" | "session-end" => {
                mmu.fold_all_open_subagents(AgentKind::ClaudeCode, "session_end");
                let _ = mmu.rollover_and_sync_dag_root();
                json!({})
            }
            _ => json!({}),
        };

        serde_json::to_string(&response)
    }
}

/// Claude Code caps PreCompact instructions at 10,000 characters per hook.
const MAX_COMPACTION_INSTRUCTION_CHARS: usize = 9_000;

fn compaction_instructions(tail_slot: &str) -> String {
    let mut out = String::from(
        "When writing the compaction summary, copy every `#icx:<hex>` handle, `⟪ICX:…⟫` header \
         and `⟦…⟧` packet verbatim. They are lossless pointers into the icx workspace: after \
         compaction the full content stays retrievable with the `icx` MCP tool (op=deref, slice \
         or grep). Keep failing tests, error codes, file paths and open tasks.\n\
         Current icx context map:\n",
    );
    let budget = MAX_COMPACTION_INSTRUCTION_CHARS.saturating_sub(out.chars().count());
    out.extend(tail_slot.chars().take(budget));
    out
}

/// The text of an MCP tool result made only of `text` content blocks (`[{"type":"text",...}]`, or
/// a `CallToolResult` object with such a `content` array), joined by blank lines; `None` for any
/// other shape, or when an image/resource block would be lost (v1.3.2, #37: stored as real lines,
/// so `slice` and `grep` can address a docs result instead of one JSON line).
fn mcp_text_content(response: &Value) -> Option<String> {
    let blocks = match response {
        Value::Array(blocks) => blocks,
        // Only a plain result: any other field (`structuredContent`, ...) keeps the JSON form, so
        // structured data is never dropped (review of #38).
        Value::Object(map)
            if map
                .keys()
                .all(|k| matches!(k.as_str(), "content" | "isError" | "_meta")) =>
        {
            map.get("content")?.as_array()?
        }
        _ => return None,
    };
    if blocks.is_empty() {
        return None;
    }
    let texts = blocks
        .iter()
        .map(|b| {
            (b.get("type").and_then(Value::as_str) == Some("text"))
                .then(|| b.get("text").and_then(Value::as_str))
                .flatten()
        })
        .collect::<Option<Vec<&str>>>()?;
    Some(texts.join("\n\n"))
}

/// Text to store for a tool response: Bash-like `{stdout, stderr}` objects become their readable
/// text, MCP text content becomes its text, and other shapes are stored as JSON.
pub(crate) fn tool_output_text(response: Option<&Value>) -> String {
    if let Some(text) = response.and_then(mcp_text_content) {
        return text;
    }
    match response {
        None => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Object(map)) if map.get("stdout").is_some_and(Value::is_string) => {
            let stdout = map.get("stdout").and_then(Value::as_str).unwrap_or("");
            match map
                .get("stderr")
                .and_then(Value::as_str)
                .filter(|e| !e.is_empty())
            {
                Some(stderr) => format!("{stdout}\n[stderr]\n{stderr}"),
                None => stdout.to_owned(),
            }
        }
        Some(other) => other.to_string(),
    }
}

/// Builds `updatedToolOutput` in the original response's shape (ICX-AUD-004): string responses
/// become the stub; Bash-like objects get `stdout` = stub and an emptied `stderr`; any other
/// object or array has its largest string leaf replaced, keeping every other field.
pub(crate) fn shape_preserving_replacement(original: Option<&Value>, stub: &str) -> Value {
    // A text-only MCP result was stored as all of its blocks joined, so every block is replaced
    // by one stub block, matching `updatedMCPToolOutput` and the stored artifact (review of #38).
    if let Some(v) = original.filter(|v| mcp_text_content(v).is_some()) {
        let block = serde_json::json!([{"type": "text", "text": stub}]);
        return match v {
            Value::Object(map) => {
                let mut out = map.clone();
                out.insert("content".to_owned(), block);
                Value::Object(out)
            }
            _ => block,
        };
    }
    match original {
        Some(Value::Object(map)) if map.get("stdout").is_some_and(Value::is_string) => {
            let mut out = map.clone();
            out.insert("stdout".to_owned(), Value::String(stub.to_owned()));
            if out.contains_key("stderr") {
                out.insert("stderr".to_owned(), Value::String(String::new()));
            }
            Value::Object(out)
        }
        Some(v @ (Value::Object(_) | Value::Array(_))) => {
            let mut out = v.clone();
            if !replace_largest_string_leaf(&mut out, stub) {
                return Value::String(stub.to_owned());
            }
            out
        }
        _ => Value::String(stub.to_owned()),
    }
}

fn largest_string_leaf(v: &Value) -> usize {
    match v {
        Value::String(s) => s.len(),
        Value::Array(a) => a.iter().map(largest_string_leaf).max().unwrap_or(0),
        Value::Object(m) => m.values().map(largest_string_leaf).max().unwrap_or(0),
        _ => 0,
    }
}

fn replace_string_leaf_of_len(v: &mut Value, target: usize, stub: &str) -> bool {
    match v {
        Value::String(s) if s.len() == target => {
            stub.clone_into(s);
            true
        }
        Value::Array(a) => a
            .iter_mut()
            .any(|c| replace_string_leaf_of_len(c, target, stub)),
        Value::Object(m) => m
            .values_mut()
            .any(|c| replace_string_leaf_of_len(c, target, stub)),
        _ => false,
    }
}

fn replace_largest_string_leaf(v: &mut Value, stub: &str) -> bool {
    let target = largest_string_leaf(v);
    target != 0 && replace_string_leaf_of_len(v, target, stub)
}
