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

/// Largest `additionalContext` Claude Code delivers inline on the settings-file hook path, in
/// JavaScript string length (UTF-16 code units); larger text is spilled to a file (Sprint 1
/// findings, Claude Code 2.1.330; T-64-006).
pub const CLAUDE_CONTEXT_MAX_CHARS: usize = 8_000;
/// Largest number of `additionalContext` lines delivered inline (same source).
pub const CLAUDE_CONTEXT_MAX_LINES: usize = 200;

/// Keeps an `additionalContext` value within [`CLAUDE_CONTEXT_MAX_CHARS`] and
/// [`CLAUDE_CONTEXT_MAX_LINES`].
///
/// Tail-slot lines are ranked best first, so whole lines are dropped from the end until both
/// limits hold, and a `⟪ICX:TRUNCATED│dropped=<n>⟫` line records how many went. Whole lines go,
/// so every `#icx:` handle that remains is intact. The result depends only on the input.
#[must_use]
pub fn claude_bounded_context(text: &str) -> String {
    let units = |s: &str| s.encode_utf16().count();
    if units(text) <= CLAUDE_CONTEXT_MAX_CHARS
        && text.split('\n').count() <= CLAUDE_CONTEXT_MAX_LINES
    {
        return text.to_owned();
    }
    let lines: Vec<&str> = text.split('\n').collect();
    // The marker's own size, at its widest (`dropped` = every line).
    let marker = |n: usize| format!("⟪ICX:TRUNCATED│dropped={n}⟫");
    let reserve = units(&marker(lines.len())) + 1;
    let mut kept: Vec<&str> = Vec::new();
    let mut used = 0usize;
    for line in &lines {
        let cost = units(line) + usize::from(!kept.is_empty());
        if kept.len() + 1 >= CLAUDE_CONTEXT_MAX_LINES
            || used + cost + reserve > CLAUDE_CONTEXT_MAX_CHARS
        {
            break;
        }
        kept.push(line);
        used += cost;
    }
    let dropped = lines.len() - kept.len();
    let mut out = kept.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(&marker(dropped));
    out
}

/// `hookSpecificOutput` carrying bounded `additionalContext` for `event`. Claude Code drops the
/// context when `hookEventName` does not name the event that ran, so it is always set.
fn context_reply(event: &str, context: &str) -> Value {
    json!({
        "hookSpecificOutput": {
            "hookEventName": event,
            "additionalContext": claude_bounded_context(context)
        }
    })
}

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
                context_reply("SessionStart", &format!("{ICX_AHTL_LEGEND}\n{tail}"))
            }
            "UserPromptSubmit" | "user-prompt-submit" => {
                let prompt = payload
                    .get("prompt")
                    .and_then(Value::as_str)
                    .unwrap_or("user_query");
                let tail = mmu.build_ephemeral_tail_slot(AgentKind::ClaudeCode, prompt, 4);
                context_reply("UserPromptSubmit", &tail)
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
                context_reply("SubagentStart", &tail)
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
            // A new model starts a new prompt cache, so the epoch is sealed (T-64-005). Only the
            // Post event is used: `PreModelSwitch` blocks the switch on a failing or slow hook.
            // The seal is forced: the old cache is of no use to the new model, whatever the
            // epoch's size.
            "PostModelSwitch" | "post-model-switch" => {
                let _ = mmu.force_rollover_and_sync_dag_root();
                json!({})
            }
            "PreCompact" | "pre-compact" => {
                let _ = mmu.force_rollover_and_sync_dag_root();
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
                let _ = mmu.force_rollover_and_sync_dag_root();
                json!({})
            }
            "SessionEnd" | "session-end" => {
                mmu.fold_all_open_subagents(AgentKind::ClaudeCode, "session_end");
                let _ = mmu.force_rollover_and_sync_dag_root();
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
        Some(Value::Object(map)) if is_repl_result(map) => repl_result_text(map),
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
        Some(Value::Object(map)) => builtin_result_text(map).unwrap_or_else(|| map_json(map)),
        Some(other) => other.to_string(),
    }
}

/// A Code Mode `REPL` result, recorded live on 2.1.330: `{code, result, stdout, stderr}`, plus
/// `toolsUsed` only when the script called a tool. It also has a `stdout` string, so it is
/// recognised before the Bash shape.
fn is_repl_result(map: &serde_json::Map<String, Value>) -> bool {
    map.contains_key("code") && map.contains_key("result")
}

/// The stored text of a `REPL` result: its result, then any output streams, labelled. The
/// echoed script is left out, since it is the tool's input.
fn repl_result_text(map: &serde_json::Map<String, Value>) -> String {
    let mut text = match map.get("result") {
        Some(Value::String(s)) => s.clone(),
        None | Some(Value::Null) => String::new(),
        Some(other) => other.to_string(),
    };
    for stream in ["stdout", "stderr"] {
        if let Some(s) = map
            .get(stream)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&format!("[{stream}]\n{s}"));
        }
    }
    text
}

fn map_json(map: &serde_json::Map<String, Value>) -> String {
    Value::Object(map.clone()).to_string()
}

/// The text of a Claude Code built-in tool result, in the shapes recorded live on 2.1.330
/// (T-64-001), so the stored artifact has the tool's real lines rather than one line of JSON:
///
/// - `Read` of a text file: `{"type": "text", "file": {"content", ...}}` gives `file.content`;
/// - `Grep`: `{"mode", "content", "filenames", ...}` gives `content`, or the file names, one per
///   line, in the file-list modes;
/// - `Glob`: `{"filenames", "numFiles", ...}` gives the file names, one per line.
///
/// Any other shape (an image or notebook `Read`, an unknown tool) returns `None` and is kept as
/// JSON.
fn builtin_result_text(map: &serde_json::Map<String, Value>) -> Option<String> {
    let filenames = || {
        map.get("filenames")?
            .as_array()?
            .iter()
            .map(Value::as_str)
            .collect::<Option<Vec<&str>>>()
            .filter(|names| !names.is_empty())
            .map(|names| names.join("\n"))
    };
    if map.get("type").and_then(Value::as_str) == Some("text")
        && let Some(content) = map
            .get("file")
            .and_then(|f| f.get("content"))
            .and_then(Value::as_str)
    {
        return Some(content.to_owned());
    }
    if map.contains_key("mode") {
        if let Some(content) = map
            .get("content")
            .and_then(Value::as_str)
            .filter(|c| !c.is_empty())
        {
            return Some(content.to_owned());
        }
        return filenames();
    }
    if map.contains_key("numFiles") && !map.contains_key("content") {
        return filenames();
    }
    // WebFetch: `{url, code, codeText, result, bytes, durationMs}`.
    if map.contains_key("url")
        && map.contains_key("code")
        && let Some(result) = map.get("result").and_then(Value::as_str)
    {
        return Some(result.to_owned());
    }
    // Agent (subagent report): `{agentId, content: [text blocks], ...}`; text only when every
    // block is text.
    if is_agent_result(map) {
        return map
            .get("content")
            .and_then(Value::as_array)
            .and_then(|blocks| crate::text_only_parts(blocks));
    }
    None
}

/// A Claude Code `Agent` (subagent) result, recorded live on 2.1.330: `{agentId, agentType,
/// status, prompt, content: [...], usage, ...}`.
fn is_agent_result(map: &serde_json::Map<String, Value>) -> bool {
    map.contains_key("agentId") && map.get("content").is_some_and(Value::is_array)
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
        // A subagent report: all its content blocks become one stub block, as for MCP results,
        // so no block of the report still reaches the model (T-64-001).
        Some(Value::Object(map)) if is_agent_result(map) => {
            let mut out = map.clone();
            out.insert(
                "content".to_owned(),
                serde_json::json!([{"type": "text", "text": stub}]),
            );
            Value::Object(out)
        }
        // A Code Mode script: the stub replaces its result and its streams, never the echoed
        // `code` (which can be the largest string).
        Some(Value::Object(map)) if is_repl_result(map) => {
            let mut out = map.clone();
            out.insert("result".to_owned(), Value::String(stub.to_owned()));
            for stream in ["stdout", "stderr"] {
                if out.contains_key(stream) {
                    out.insert(stream.to_owned(), Value::String(String::new()));
                }
            }
            Value::Object(out)
        }
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
