//! T-63-002: Cursor adapter contract tests.
//!
//! Fixtures follow the documented payloads (cursor.com/docs/hooks, read 2026-11-22). On
//! `cursor-agent` 2026.09.08 the session-event payloads matched them live; the tool-call capture
//! was blocked by the account's usage limit, so the MCP replacement stays opt-in.

use icx_adapters::{CursorAdapter, ICX_AHTL_LEGEND, InfinityContextMmu, fail_open_reply};
use icx_core::{AgentKind, ArtifactId};
use serde_json::{Value, json};

fn payload(event: &str, extra: &Value) -> String {
    let mut m = json!({
        "conversation_id": "conv-1",
        "generation_id": "gen-1",
        "model": "claude-opus-4-7-thinking-max",
        "hook_event_name": event,
        "cursor_version": "2026.09.08",
        "workspace_roots": ["/home/u/project"],
        "user_email": null,
        "transcript_path": null,
        "an_undocumented_future_field": [1, 2, 3]
    })
    .as_object()
    .unwrap()
    .clone();
    m.extend(extra.as_object().unwrap().clone());
    Value::Object(m).to_string()
}

// Call sites pass `json!(...)` temporaries; taking them by value keeps the fixtures readable.
#[allow(clippy::needless_pass_by_value)]
fn reply(event: &str, extra: Value, mmu: &mut InfinityContextMmu, stub: bool) -> Value {
    let out = CursorAdapter::handle_hook_with_stub_mode(event, &payload(event, &extra), mmu, stub)
        .unwrap();
    serde_json::from_str(&out).unwrap()
}

fn big_output() -> String {
    (0..400)
        .map(|i| format!("row {i}: value={}", i * 7))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn session_start_injects_nothing_because_it_lands_in_system_context() {
    // icx context is untrusted retrieved data and must never reach the model as system
    // instructions; Cursor adds sessionStart `additional_context` to the system context.
    let mut mmu = InfinityContextMmu::default();
    mmu.ingest_payload(
        AgentKind::Cursor,
        "prior",
        "earlier output to recall\n",
        false,
    );
    let r = reply(
        "sessionStart",
        json!({"session_id": "conv-1", "is_background_agent": false, "composer_mode": "agent"}),
        &mut mmu,
        false,
    );
    assert_eq!(r, json!({}));
    let out = CursorAdapter::handle_hook(
        "sessionStart",
        &payload("sessionStart", &json!({})),
        &mut mmu,
    )
    .unwrap();
    assert!(!out.contains(ICX_AHTL_LEGEND) && !out.contains("TAIL_SLOT"));
}

#[test]
fn shell_output_json_string_is_unwrapped_and_never_replaced() {
    let mut mmu = InfinityContextMmu::default();
    let out = big_output();
    let r = reply(
        "postToolUse",
        json!({
            "tool_name": "Shell",
            "tool_input": {"command": "cargo test"},
            "tool_output": json!({"exitCode": 0, "stdout": out}).to_string(),
            "tool_use_id": "abc123",
            "cwd": "/home/u/project",
            "duration": 5432
        }),
        &mut mmu,
        true,
    );
    assert_eq!(r, json!({}), "only MCP results can be replaced in Cursor");
    let id = ArtifactId::hash_bytes(out.as_bytes()).0;
    assert!(
        mmu.cas.get_meta(id).is_some(),
        "stdout, not the JSON envelope, is stored"
    );
}

#[test]
fn mcp_result_is_stubbed_only_in_stub_mode() {
    let mut mmu = InfinityContextMmu::default();
    let out = big_output();
    let extra = json!({
        "tool_name": "mcp_github_search_code",
        "tool_input": {"q": "x"},
        "tool_output": json!({"content": [{"type": "text", "text": out}]}).to_string()
    });
    assert_eq!(
        reply("postToolUse", extra.clone(), &mut mmu, false),
        json!({})
    );
    let r = reply("postToolUse", extra, &mut mmu, true);
    let text = r["updated_mcp_tool_output"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(text.starts_with("⟪ICX:STUB│#icx:"), "{text}");
    let handle = text.split('│').nth(1).unwrap().trim_start_matches("#icx:");
    let id = mmu.cas.resolve_handle(handle).unwrap();
    assert_eq!(mmu.cas.deref_bytes(id).unwrap(), out.as_bytes(), "lossless");
}

#[test]
fn failures_are_ingested_with_their_error_and_never_replaced() {
    let mut mmu = InfinityContextMmu::default();
    let r = reply(
        "postToolUseFailure",
        json!({
            "tool_name": "mcp_github_search_code",
            "tool_output": json!({"content": [{"type": "text", "text": big_output()}]}).to_string(),
            "error_message": "rate limited"
        }),
        &mut mmu,
        true,
    );
    assert_eq!(r, json!({}));
    let mut expected = big_output();
    expected.push_str("\nrate limited");
    let id = ArtifactId::hash_bytes(expected.as_bytes()).0;
    assert!(
        mmu.cas.get_meta(id).is_some(),
        "the error is kept with the output"
    );
}

#[test]
fn icx_tool_compaction_end_and_unknown_events_change_nothing() {
    let mut mmu = InfinityContextMmu::default();
    let before = mmu.cas.artifact_count();
    assert_eq!(
        reply(
            "postToolUse",
            json!({"tool_name": "mcp_icx_icx", "tool_output": big_output()}),
            &mut mmu,
            true
        ),
        json!({})
    );
    assert_eq!(
        mmu.cas.artifact_count(),
        before,
        "icx's own tool is not ingested"
    );
    for (event, extra) in [
        (
            "preCompact",
            json!({"trigger": "auto", "context_usage_percent": 85}),
        ),
        ("sessionEnd", json!({})),
        (
            "beforeSubmitPrompt",
            json!({"prompt": "hi", "attachments": []}),
        ),
        (
            "afterShellExecution",
            json!({"command": "ls", "output": "a"}),
        ),
    ] {
        assert_eq!(reply(event, extra, &mut mmu, true), json!({}), "{event}");
    }
    assert_eq!(fail_open_reply(AgentKind::Cursor, "postToolUse"), "{}");
}

// The MCP replacement is unverified live, so the adapter must not advertise it.
const _: () = assert!(!CursorAdapter::SUPPORTS_TOOL_OUTPUT_REPLACEMENT);

fn held(mmu: &InfinityContextMmu, text: &str) -> bool {
    mmu.cas
        .get_meta(ArtifactId::hash_bytes(text.as_bytes()).0)
        .is_some()
}

#[test]
fn trait_dispatch_empty_stdin_and_the_env_entry_point() {
    use icx_adapters::AgentHookAdapter;
    let mut mmu = InfinityContextMmu::default();
    let adapter = CursorAdapter;
    assert_eq!(adapter.agent_kind(), AgentKind::Cursor);
    assert_eq!(
        adapter
            .handle_hook_event("sessionEnd", "", &mut mmu)
            .unwrap(),
        "{}"
    );
    assert!(
        adapter
            .handle_hook_event("postToolUse", "{", &mut mmu)
            .is_err()
    );
    CursorAdapter::handle_hook(
        "postToolUse",
        &payload(
            "postToolUse",
            &json!({"tool_name": "Shell", "tool_output": json!({"stdout": "via env entry"}).to_string()}),
        ),
        &mut mmu,
    )
    .unwrap();
    assert!(held(&mmu, "via env entry"));
}

#[test]
fn tool_output_shapes_are_unwrapped_or_kept_verbatim() {
    let mut mmu = InfinityContextMmu::default();
    let cases = [
        (
            json!({"output": "an object, not a string"}),
            "an object, not a string".to_owned(),
        ),
        (json!("not json at all"), "not json at all".to_owned()),
        (
            json!(json!({"text": "text field"}).to_string()),
            "text field".to_owned(),
        ),
        (
            json!(json!({"weird": true}).to_string()),
            json!({"weird": true}).to_string(),
        ),
        (
            json!(json!({"content": [{"type": "image"}]}).to_string()),
            json!({"content": [{"type": "image"}]}).to_string(),
        ),
    ];
    for (tool_output, expected) in cases {
        let r = reply(
            "postToolUse",
            json!({"tool_name": "Shell", "tool_output": tool_output}),
            &mut mmu,
            true,
        );
        assert_eq!(r, json!({}));
        assert!(held(&mmu, &expected), "{expected}");
    }
    for extra in [
        json!({"tool_name": "Shell"}),
        json!({"tool_name": "Shell", "tool_output": 42}),
    ] {
        assert_eq!(reply("postToolUse", extra, &mut mmu, true), json!({}));
    }
}
