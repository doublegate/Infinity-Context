//! v1.1.0 Claude Code hook contract (ICX-AUD-004), pinned to the documented payload/response
//! shapes at <https://code.claude.com/docs/en/hooks> (checked 2026-11-18, Claude Code 2.1.328).

use icx_adapters::ClaudeCodeAdapter;
use icx_adapters::mmu::InfinityContextMmu;
use serde_json::{Value, json};

fn run(event: &str, payload: &Value, mmu: &mut InfinityContextMmu) -> String {
    ClaudeCodeAdapter::handle_hook(event, &payload.to_string(), mmu).unwrap()
}

fn big_log() -> String {
    "test engine::case ... FAILED\nerror[E0308]: mismatched types at src/lib.rs:42\n".repeat(60)
}

#[test]
fn bash_replacement_matches_the_builtin_output_shape() {
    // Built-in tools return structured objects; a mismatched shape is silently ignored upstream.
    let mut mmu = InfinityContextMmu::default();
    let payload = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "cargo test"},
        "tool_response": {"stdout": big_log(), "stderr": "warning: x\n", "interrupted": false, "isImage": false}
    });
    let out: Value = serde_json::from_str(&run("PostToolUse", &payload, &mut mmu)).unwrap();
    let updated = &out["hookSpecificOutput"]["updatedToolOutput"];
    assert!(updated.is_object(), "Bash needs an object, got {updated}");
    assert!(
        updated["stdout"]
            .as_str()
            .unwrap()
            .starts_with("⟪ICX:STUB│#icx:")
    );
    assert_eq!(updated["stderr"], "");
    assert_eq!(updated["interrupted"], false);
    assert_eq!(updated["isImage"], false);
    // The CAS keeps the readable stdout/stderr text, not a JSON dump.
    let handle = updated["stdout"]
        .as_str()
        .unwrap()
        .split('│')
        .nth(1)
        .unwrap();
    let id = mmu.resolve_handle(handle).unwrap();
    let stored = mmu.deref_text(id).unwrap();
    assert!(
        stored.starts_with("test engine::case ... FAILED\n"),
        "{}",
        &stored[..80]
    );
    assert!(stored.contains("warning: x"));
}

#[test]
fn nested_builtin_shapes_keep_their_structure() {
    let mut mmu = InfinityContextMmu::default();
    let payload = json!({
        "tool_name": "Read",
        "tool_input": {"file_path": "src/lib.rs"},
        "tool_response": {"type": "text", "file": {"filePath": "src/lib.rs", "content": big_log(), "numLines": 120}}
    });
    let out: Value = serde_json::from_str(&run("PostToolUse", &payload, &mut mmu)).unwrap();
    let updated = &out["hookSpecificOutput"]["updatedToolOutput"];
    assert_eq!(updated["type"], "text");
    assert_eq!(updated["file"]["filePath"], "src/lib.rs");
    assert_eq!(updated["file"]["numLines"], 120);
    assert!(
        updated["file"]["content"]
            .as_str()
            .unwrap()
            .starts_with("⟪ICX:STUB│")
    );
}

#[test]
fn mcp_tools_receive_a_string_replacement() {
    let mut mmu = InfinityContextMmu::default();
    let payload = json!({"tool_name": "mcp__github__search", "tool_response": big_log()});
    let out: Value = serde_json::from_str(&run("PostToolUse", &payload, &mut mmu)).unwrap();
    let hso = &out["hookSpecificOutput"];
    assert!(
        hso["updatedMCPToolOutput"]
            .as_str()
            .unwrap()
            .starts_with("⟪ICX:STUB│")
    );
    assert!(hso["updatedToolOutput"].is_string());
}

#[test]
fn precompact_emits_plain_text_instructions_under_the_cap() {
    let mut mmu = InfinityContextMmu::default();
    run(
        "PostToolUse",
        &json!({"tool_name": "Bash", "tool_response": {"stdout": big_log()}}),
        &mut mmu,
    );
    let out = run(
        "PreCompact",
        &json!({"trigger": "auto", "custom_instructions": null}),
        &mut mmu,
    );
    assert!(
        serde_json::from_str::<Value>(&out).is_err(),
        "stdout is used verbatim, not parsed"
    );
    assert!(out.contains("#icx:"), "{out}");
    assert!(out.len() < 10_000);
}

#[test]
fn postcompact_stores_the_summary_and_emits_nothing_model_facing() {
    let mut mmu = InfinityContextMmu::default();
    let before = mmu.cas.artifact_count();
    let out = run(
        "PostCompact",
        &json!({"trigger": "auto", "compact_summary": "Summary: fixed E0308 in src/lib.rs"}),
        &mut mmu,
    );
    assert_eq!(out, "{}");
    assert_eq!(mmu.cas.artifact_count(), before + 1);
}

#[test]
fn subagent_stop_folds_with_last_assistant_message_and_never_keeps_it_running() {
    let mut mmu = InfinityContextMmu::default();
    run(
        "SubagentStart",
        &json!({"agent_id": "agent-1", "agent_type": "Explore"}),
        &mut mmu,
    );
    let out = run(
        "SubagentStop",
        &json!({
            "agent_id": "agent-1",
            "agent_type": "Explore",
            "stop_hook_active": false,
            "last_assistant_message": "Found 3 call sites of fold_branch"
        }),
        &mut mmu,
    );
    assert_eq!(
        out, "{}",
        "additionalContext/decision would keep the subagent running"
    );
    let rendered = format!("{:?}", mmu.dag);
    assert!(
        rendered.contains("Found 3 call sites"),
        "fold summary recorded"
    );
}

#[test]
fn array_and_non_textual_responses_keep_a_valid_shape() {
    let mut mmu = InfinityContextMmu::default();
    // Array-shaped output (e.g. MCP content blocks returned by a built-in wrapper).
    let arr = json!({"tool_name": "Grep", "tool_response": [{"type": "text", "text": big_log()}, {"n": 3}]});
    let out: Value = serde_json::from_str(&run("PostToolUse", &arr, &mut mmu)).unwrap();
    let updated = &out["hookSpecificOutput"]["updatedToolOutput"];
    assert!(updated.is_array());
    assert!(
        updated[0]["text"]
            .as_str()
            .unwrap()
            .starts_with("⟪ICX:STUB│")
    );
    assert_eq!(updated[1]["n"], 3);
    // An object with no string leaf at all falls back to a plain stub string.
    let numbers: Vec<u64> = (0..2_000).collect();
    let nums = json!({"tool_name": "Stats", "tool_response": {"values": numbers}});
    let out: Value = serde_json::from_str(&run("PostToolUse", &nums, &mut mmu)).unwrap();
    if let Some(updated) = out["hookSpecificOutput"].get("updatedToolOutput") {
        assert!(updated.is_string() || updated.is_object(), "{updated}");
    }
}

#[test]
fn precompact_instructions_are_capped_on_large_workspaces() {
    let mut mmu = InfinityContextMmu::default();
    for i in 0..60 {
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": format!("cargo test suite_{i}")},
            "tool_response": {"stdout": format!("{}\n{}", "x".repeat(200), big_log())}
        });
        run("PostToolUse", &payload, &mut mmu);
    }
    let out = run("PreCompact", &json!({"trigger": "auto"}), &mut mmu);
    assert!(out.chars().count() <= 9_000, "{}", out.chars().count());
}
