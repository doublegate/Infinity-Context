//! T-63-003: OpenCode V2 plugin adapter contract tests (the payloads `integrations/opencode/icx.ts`
//! sends; the event shapes it reads from OpenCode v2.2.7 are recorded in the Sprint 3 notes).

use icx_adapters::{ICX_AHTL_LEGEND, InfinityContextMmu, OpenCodeAdapter, fail_open_reply};
use icx_core::{AgentKind, ArtifactId};
use serde_json::{Value, json};

// Call sites pass `json!(...)` temporaries; taking them by value keeps the fixtures readable.
#[allow(clippy::needless_pass_by_value)]
fn reply(event: &str, payload: Value, mmu: &mut InfinityContextMmu, stub: bool) -> Value {
    let out = OpenCodeAdapter::handle_hook_with_stub_mode(event, &payload.to_string(), mmu, stub)
        .unwrap();
    serde_json::from_str(&out).unwrap()
}

fn big_output() -> String {
    (0..400)
        .map(|i| format!("line {i}: test_case_{i} ... ok"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn context_hook_writes_no_system_text() {
    let mut mmu = InfinityContextMmu::default();
    assert_eq!(
        reply("context", json!({"sessionID": "s1"}), &mut mmu, false),
        json!({})
    );
}

#[test]
fn prompt_appends_the_legend_and_tail_slot_to_the_user_message() {
    let mut mmu = InfinityContextMmu::default();
    let r = reply(
        "prompt",
        json!({"sessionID": "s1", "prompt": "why does the build fail"}),
        &mut mmu,
        false,
    );
    let append = r["append"].as_str().unwrap();
    assert!(append.starts_with(ICX_AHTL_LEGEND));
    assert!(append.contains("⟪ICX:TAIL_SLOT│agent=open-code"));
    assert!(r.get("system").is_none());
}

#[test]
fn execute_after_ingests_and_replaces_only_in_stub_mode() {
    let mut mmu = InfinityContextMmu::default();
    let out = big_output();
    let event = json!({
        "sessionID": "s1",
        "tool": "bash",
        "call_id": "call_1",
        "input": {"command": "cargo test"},
        "status": "completed",
        "output": out,
        "an_undocumented_future_field": true
    });
    assert_eq!(
        reply("execute.after", event.clone(), &mut mmu, false),
        json!({})
    );
    let id = ArtifactId::hash_bytes(out.as_bytes()).0;
    assert!(mmu.cas.get_meta(id).is_some(), "stored for recall");

    let r = reply("execute.after", event, &mut mmu, true);
    let stub = r["replace"].as_str().unwrap();
    assert!(stub.starts_with("⟪ICX:STUB│#icx:"), "{stub}");
    let handle = stub.split('│').nth(1).unwrap().trim_start_matches("#icx:");
    let rid = mmu.cas.resolve_handle(handle).unwrap();
    assert_eq!(
        mmu.cas.deref_bytes(rid).unwrap(),
        out.as_bytes(),
        "lossless"
    );
}

#[test]
fn errors_content_parts_and_the_icx_tool_are_handled() {
    let mut mmu = InfinityContextMmu::default();
    let failed = reply(
        "execute.after",
        json!({"tool": "bash", "status": "error", "output": big_output(), "error": "exit 101"}),
        &mut mmu,
        true,
    );
    assert_eq!(failed, json!({}), "a failed result is never replaced");
    let mut expected = big_output();
    expected.push_str("\nexit 101");
    assert!(
        mmu.cas
            .get_meta(ArtifactId::hash_bytes(expected.as_bytes()).0)
            .is_some()
    );

    let parts = reply(
        "execute.after",
        json!({"tool": "read", "status": "completed", "output": [{"type": "text", "text": "a"}, {"type": "text", "text": "b"}]}),
        &mut mmu,
        false,
    );
    assert_eq!(parts, json!({}));
    assert!(
        mmu.cas
            .get_meta(ArtifactId::hash_bytes(b"a\nb").0)
            .is_some()
    );

    let before = mmu.cas.artifact_count();
    for tool in [
        "icx_icx",
        "icx_icx_write",
        "infinity-context_icx",
        "infinity_context_icx_write",
        "mcp_icx_icx",
        "icx",
    ] {
        assert_eq!(
            reply(
                "execute.after",
                json!({"tool": tool, "status": "completed", "output": big_output()}),
                &mut mmu,
                true
            ),
            json!({}),
            "{tool}"
        );
    }
    assert_eq!(
        mmu.cas.artifact_count(),
        before,
        "icx's own tool is never ingested"
    );
}

#[test]
fn compaction_session_end_and_unknown_events_change_nothing() {
    let mut mmu = InfinityContextMmu::default();
    for event in ["compaction", "session.end", "some.future.event"] {
        assert_eq!(
            reply(event, json!({"sessionID": "s1"}), &mut mmu, true),
            json!({}),
            "{event}"
        );
    }
    assert_eq!(fail_open_reply(AgentKind::OpenCode, "execute.after"), "{}");
}

// Replacement is verified on v2.2.7 but opt-in, so it is not advertised as a default.
const _: () = assert!(!OpenCodeAdapter::SUPPORTS_TOOL_OUTPUT_REPLACEMENT);

fn held(mmu: &InfinityContextMmu, text: &str) -> bool {
    mmu.cas
        .get_meta(ArtifactId::hash_bytes(text.as_bytes()).0)
        .is_some()
}

#[test]
fn trait_dispatch_empty_stdin_and_the_env_entry_point() {
    use icx_adapters::AgentHookAdapter;
    let mut mmu = InfinityContextMmu::default();
    let adapter = OpenCodeAdapter;
    assert_eq!(adapter.agent_kind(), AgentKind::OpenCode);
    assert_eq!(
        adapter
            .handle_hook_event("compaction", "  ", &mut mmu)
            .unwrap(),
        "{}"
    );
    assert!(
        adapter
            .handle_hook_event("execute.after", "{not json", &mut mmu)
            .is_err()
    );
    // The environment-driven entry point ingests whatever the stub setting is.
    let out = big_output();
    OpenCodeAdapter::handle_hook(
        "execute.after",
        &json!({"tool": "bash", "status": "completed", "output": out}).to_string(),
        &mut mmu,
    )
    .unwrap();
    assert!(held(&mmu, &out));
}

#[test]
fn unusual_result_shapes_are_stored_as_text_or_json() {
    let mut mmu = InfinityContextMmu::default();
    let cases = [
        (json!({"tool": "t", "status": "completed"}), None),
        (
            json!({"tool": "t", "status": "error", "error": "boom only"}),
            Some("boom only".to_owned()),
        ),
        (
            json!({"tool": "t", "status": "completed", "output": [{"type": "image"}]}),
            Some(json!([{"type": "image"}]).to_string()),
        ),
    ];
    for (event, expected) in cases {
        assert_eq!(reply("execute.after", event, &mut mmu, true), json!({}));
        if let Some(text) = expected {
            assert!(held(&mmu, &text), "{text}");
        }
    }
    // An object result goes through the shared tool-output formatter and is stored.
    let before = mmu.cas.artifact_count();
    reply(
        "execute.after",
        json!({"tool": "t", "status": "completed", "output": {"stdout": "from an object"}}),
        &mut mmu,
        false,
    );
    assert!(mmu.cas.artifact_count() > before);
}

#[test]
fn mixed_text_and_image_parts_are_kept_whole() {
    let mut mmu = InfinityContextMmu::default();
    let parts =
        json!([{"type": "text", "text": "caption"}, {"type": "image", "data": "iVBORw0KGgo"}]);
    reply(
        "execute.after",
        json!({"tool": "screenshot", "status": "completed", "output": parts}),
        &mut mmu,
        false,
    );
    assert!(
        held(&mmu, &parts.to_string()),
        "the image part is not dropped"
    );
    assert!(!held(&mmu, "caption"));
}

#[test]
fn the_legend_is_left_out_when_the_plugin_already_sent_it() {
    let mut mmu = InfinityContextMmu::default();
    let first = reply(
        "prompt",
        json!({"sessionID": "s1", "prompt": "q"}),
        &mut mmu,
        false,
    );
    assert!(
        first["append"]
            .as_str()
            .unwrap()
            .starts_with(ICX_AHTL_LEGEND)
    );
    let later = reply(
        "prompt",
        json!({"sessionID": "s1", "prompt": "q", "legend": false}),
        &mut mmu,
        false,
    );
    let text = later["append"].as_str().unwrap();
    assert!(!text.contains("ICX:LEGEND"), "{text}");
    assert!(text.starts_with("⟪ICX:TAIL_SLOT"));
}
