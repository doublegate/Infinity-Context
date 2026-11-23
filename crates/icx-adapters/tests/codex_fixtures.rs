//! T-65-001: the Codex adapter against payloads recorded live on Codex CLI 0.171.0
//! (`tests/fixtures/codex/0.171.0/`, sanitized by `scripts/sanitize-hook-fixtures.py`).
//!
//! The recorded session ran `seq 1 3000` (`Bash`, a string `tool_response`), then icx's own MCP
//! tools `mcp__icx__icx` and `mcp__icx__icx_write` (object responses). Codex's own memory task ran
//! in the same process and was left out of the fixtures.

use icx_adapters::{CODEX_INLINE_CONTEXT_MAX_BYTES, CodexAdapter, InfinityContextMmu};
use serde_json::{Value, json};
use std::path::Path;

const DIR: &str = "tests/fixtures/codex/0.171.0";

fn fixture(name: &str) -> String {
    std::fs::read_to_string(Path::new(DIR).join(format!("{name}.json")))
        .unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

fn reply(event: &str, name: &str, mmu: &mut InfinityContextMmu) -> Value {
    let out = CodexAdapter::handle_hook(event, &fixture(name), mmu).unwrap();
    serde_json::from_str(&out).unwrap()
}

/// Codex rejects hook output with unknown keys (`deny_unknown_fields`), so every reply sticks to
/// the documented ones.
fn only_documented_keys(v: &Value) {
    for key in v.as_object().unwrap().keys() {
        assert!(
            [
                "continue",
                "suppressOutput",
                "hookSpecificOutput",
                "decision",
                "reason",
                "stopReason",
                "systemMessage"
            ]
            .contains(&key.as_str()),
            "undocumented key {key} in {v}"
        );
    }
}

#[test]
fn context_events_add_bounded_additional_context() {
    let mut mmu = InfinityContextMmu::default();
    for event in ["SessionStart", "UserPromptSubmit"] {
        let r = reply(event, event, &mut mmu);
        only_documented_keys(&r);
        assert_eq!(r["continue"], true);
        assert_eq!(r["hookSpecificOutput"]["hookEventName"], event);
        let ctx = r["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap();
        assert!(ctx.contains("ICX:") && ctx.len() <= CODEX_INLINE_CONTEXT_MAX_BYTES);
    }
}

#[test]
fn a_shell_result_is_ingested_and_never_replaced() {
    let mut mmu = InfinityContextMmu::default();
    let r = reply("PostToolUse", "PostToolUse", &mut mmu);
    only_documented_keys(&r);
    assert_eq!(r["continue"], true);
    // Code Mode-only models ignore Codex's replacement routes (T-62-001), so icx sends none.
    assert!(r.get("decision").is_none());
    assert!(
        r["hookSpecificOutput"]
            .get("updatedMCPToolOutput")
            .is_none()
    );
    assert_eq!(mmu.cas.artifact_count(), 1, "the shell output is stored");
}

#[test]
fn icx_own_tools_are_never_ingested() {
    let mut mmu = InfinityContextMmu::default();
    for name in ["PostToolUse-2", "PostToolUse-3"] {
        let raw: Value = serde_json::from_str(&fixture(name)).unwrap();
        assert!(raw["tool_name"].as_str().unwrap().starts_with("mcp__icx__"));
        let r = reply("PostToolUse", name, &mut mmu);
        assert_eq!(
            r,
            json!({"continue": true, "suppressOutput": false}),
            "{name}"
        );
    }
    assert_eq!(mmu.cas.artifact_count(), 0);
}

#[test]
fn stop_continues() {
    let mut mmu = InfinityContextMmu::default();
    let r = reply("Stop", "Stop", &mut mmu);
    only_documented_keys(&r);
    assert_eq!(r["continue"], true);
}
