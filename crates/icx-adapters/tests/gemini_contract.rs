//! T-63-001: Gemini CLI adapter contract tests.
//!
//! Gemini CLI is not installed on the reference machine (owner decision, 2026-11-22), so these
//! fixtures are built from the documented payloads (`google-gemini/gemini-cli`
//! `docs/hooks/reference.md` and `packages/core/src/hooks/types.ts`, read 2026-11-22), not
//! captured live.

use icx_adapters::{GeminiCliAdapter, ICX_AHTL_LEGEND, InfinityContextMmu, fail_open_reply};
use icx_core::{AgentKind, ArtifactId};
use serde_json::{Value, json};

fn common(event: &str) -> serde_json::Map<String, Value> {
    let v = json!({
        "session_id": "gemini-session-1",
        "transcript_path": "/home/u/.gemini/tmp/chats/session-1.json",
        "cwd": "/home/u/project",
        "hook_event_name": event,
        "timestamp": "2026-11-22T00:00:00Z",
        "an_undocumented_future_field": {"nested": true}
    });
    v.as_object().unwrap().clone()
}

fn payload(event: &str, extra: &Value) -> String {
    let mut m = common(event);
    m.extend(extra.as_object().unwrap().clone());
    Value::Object(m).to_string()
}

// Call sites pass `json!(...)` temporaries; taking them by value keeps the fixtures readable.
#[allow(clippy::needless_pass_by_value)]
fn reply(event: &str, extra: Value, mmu: &mut InfinityContextMmu, stub: bool) -> Value {
    let out =
        GeminiCliAdapter::handle_hook_with_stub_mode(event, &payload(event, &extra), mmu, stub)
            .unwrap();
    serde_json::from_str(&out).unwrap()
}

fn big_output() -> String {
    (0..400)
        .map(|i| format!("src/module_{i}.rs: compiled ok in {i} ms"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn session_start_and_before_agent_inject_context_with_the_event_name() {
    let mut mmu = InfinityContextMmu::default();
    let start = reply(
        "SessionStart",
        json!({"source": "startup"}),
        &mut mmu,
        false,
    );
    assert_eq!(start["hookSpecificOutput"]["hookEventName"], "SessionStart");
    let ctx = start["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(ctx.starts_with(ICX_AHTL_LEGEND));
    assert!(ctx.contains("⟪ICX:TAIL_SLOT│agent=gemini-cli"));

    let before = reply(
        "BeforeAgent",
        json!({"prompt": "fix the build"}),
        &mut mmu,
        false,
    );
    assert_eq!(before["hookSpecificOutput"]["hookEventName"], "BeforeAgent");
    assert!(
        before["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .starts_with("⟪ICX:TAIL_SLOT")
    );
    // No flow-control field that could block or stop the turn.
    for v in [&start, &before] {
        assert!(v.get("decision").is_none() && v.get("continue").is_none());
    }
}

#[test]
fn after_tool_ingests_and_by_default_never_replaces_the_result() {
    let mut mmu = InfinityContextMmu::default();
    let out = big_output();
    let r = reply(
        "AfterTool",
        json!({
            "tool_name": "run_shell_command",
            "tool_input": {"command": "cargo build"},
            "tool_response": {"llmContent": out, "returnDisplay": "build ok"}
        }),
        &mut mmu,
        false,
    );
    assert_eq!(r, json!({}), "default mode changes nothing the model sees");
    let id = ArtifactId::hash_bytes(out.as_bytes()).0;
    assert!(
        mmu.cas.get_meta(id).is_some(),
        "the output is stored for recall"
    );
}

#[test]
fn stub_mode_denies_with_a_stub_whose_handle_dereferences_to_the_output() {
    let mut mmu = InfinityContextMmu::default();
    let out = big_output();
    let r = reply(
        "AfterTool",
        json!({
            "tool_name": "read_file",
            "tool_input": {"file_path": "build.log"},
            "tool_response": {"llmContent": [{"text": out}], "returnDisplay": "read 400 lines"}
        }),
        &mut mmu,
        true,
    );
    assert_eq!(r["decision"], "deny");
    let reason = r["reason"].as_str().unwrap();
    assert!(reason.starts_with("⟪ICX:STUB│#icx:"), "{reason}");
    let handle = reason
        .split('│')
        .nth(1)
        .unwrap()
        .trim_start_matches("#icx:");
    let id = mmu.cas.resolve_handle(handle).unwrap();
    assert_eq!(mmu.cas.deref_bytes(id).unwrap(), out.as_bytes(), "lossless");
    assert!(
        reason.len() < out.len(),
        "the stub is smaller than the output"
    );
}

#[test]
fn stub_mode_leaves_small_and_failed_results_alone() {
    let mut mmu = InfinityContextMmu::default();
    let small = reply(
        "AfterTool",
        json!({"tool_name": "ls", "tool_response": {"llmContent": "a.rs\nb.rs"}}),
        &mut mmu,
        true,
    );
    assert_eq!(small, json!({}), "below the stub threshold");
    let failed = reply(
        "AfterTool",
        json!({
            "tool_name": "run_shell_command",
            "tool_response": {"llmContent": big_output(), "error": {"message": "exit status 101"}}
        }),
        &mut mmu,
        true,
    );
    assert_eq!(failed, json!({}), "an error result is never replaced");
}

#[test]
fn the_icx_pager_tool_is_never_ingested_or_replaced() {
    let mut mmu = InfinityContextMmu::default();
    let before = mmu.cas.artifact_count();
    let r = reply(
        "AfterTool",
        json!({"tool_name": "mcp_icx_icx", "tool_response": {"llmContent": big_output()}}),
        &mut mmu,
        true,
    );
    assert_eq!(r, json!({}));
    assert_eq!(mmu.cas.artifact_count(), before);
}

#[test]
fn single_underscore_mcp_names_match_only_the_icx_tool() {
    for name in [
        "mcp_icx_icx",
        "mcp_infinity_context_icx",
        "mcp_infinity-context_icx",
        "MCP_ICX_ICX",
    ] {
        assert!(InfinityContextMmu::is_icx_pager_tool(name), "{name}");
    }
    // Other servers and tools, including ones whose names merely start with `icx`, are ingested.
    for name in [
        "mcp_icx_tools_search",
        "mcp_icxlike_icx2",
        "mcp_github_search",
        "run_shell_command",
    ] {
        assert!(!InfinityContextMmu::is_icx_pager_tool(name), "{name}");
    }
}

#[test]
fn compression_end_and_unknown_events_reply_with_no_change() {
    let mut mmu = InfinityContextMmu::default();
    for (event, extra) in [
        ("PreCompress", json!({"trigger": "auto"})),
        ("SessionEnd", json!({"reason": "exit"})),
        ("BeforeToolSelection", json!({})),
        ("SomeFutureEvent", json!({})),
    ] {
        assert_eq!(reply(event, extra, &mut mmu, true), json!({}), "{event}");
    }
    assert_eq!(fail_open_reply(AgentKind::GeminiCli, "AfterTool"), "{}");
}

// Replacement is unverified live, so the adapter must not advertise it.
const _: () = assert!(!GeminiCliAdapter::SUPPORTS_TOOL_OUTPUT_REPLACEMENT);
