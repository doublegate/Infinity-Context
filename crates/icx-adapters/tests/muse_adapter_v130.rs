//! Meta Muse Code (1.4.25) hook adapter contract (v1.3.0).
//!
//! Observed on 1.4.25 with probe hooks: payloads are Claude-Code-shaped (`hook_event_name`,
//! `session_id`, `tool_name`, `tool_input`, `tool_response`, plus `turn_id`, `model`,
//! `model_provider`); SessionStart, UserPromptSubmit and PostToolUse `additionalContext` reach the
//! model; `hookSpecificOutput` requires `hookEventName`; and Muse rejects replacing tool output
//! ("PostToolUse output cannot replace tool output"). Bash results arrive as a JSON string with
//! `output` and `exit_code`.
use icx_adapters::{InfinityContextMmu, MuseAdapter};
use icx_core::AgentKind;
use serde_json::{Value, json};

fn reply(mmu: &mut InfinityContextMmu, event: &str, payload: &Value) -> Value {
    let out = MuseAdapter::handle_hook(event, &payload.to_string(), mmu).expect("valid reply");
    serde_json::from_str(&out).expect("reply is JSON")
}

fn has_null(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Array(a) => a.iter().any(has_null),
        Value::Object(m) => m.values().any(has_null),
        _ => false,
    }
}

fn base(event: &str) -> Value {
    json!({
        "hook_event_name": event,
        "session_id": "sess-1",
        "turn_id": "turn-1",
        "cwd": "/work/project",
        "model": "muse-model",
        "model_provider": "meta",
        "permission_mode": "default",
        "transcript_path": null
    })
}

#[test]
fn agent_kind_parses_and_prints_muse() {
    assert_eq!(AgentKind::parse_agent("muse"), AgentKind::Muse);
    assert_eq!(AgentKind::parse_agent("muse-code"), AgentKind::Muse);
    assert_eq!(AgentKind::Muse.as_str(), "muse");
}

#[test]
fn context_events_return_additional_context_with_the_event_name() {
    let mut mmu = InfinityContextMmu::default();
    for event in ["SessionStart", "UserPromptSubmit"] {
        let mut p = base(event);
        p["prompt"] = json!("explain the epoch controller");
        p["source"] = json!("startup");
        let r = reply(&mut mmu, event, &p);
        let hso = &r["hookSpecificOutput"];
        assert_eq!(hso["hookEventName"], event, "{r}");
        assert!(
            hso["additionalContext"]
                .as_str()
                .is_some_and(|s| s.contains("ICX:TAIL_SLOT")),
            "{r}"
        );
        assert!(!has_null(&r), "{r}");
    }
}

#[test]
fn post_tool_use_captures_but_never_replaces_output() {
    let mut mmu = InfinityContextMmu::default();
    let before = mmu.cas.artifact_count();
    let response = json!({
        "chunk_id": "exec-1-1",
        "command": "seq 1 400",
        "exit_code": 0,
        "terminal_status": "completed",
        "output": (1..=400).map(|n| n.to_string()).collect::<Vec<_>>().join("\n"),
        "truncated": false
    })
    .to_string();
    let mut p = base("PostToolUse");
    p["tool_name"] = json!("bash");
    p["tool_input"] =
        json!({"command": "seq 1 400", "description": "numbers", "workdir": "/work/project"});
    p["tool_response"] = json!(response);
    p["tool_use_id"] = json!("call-1");
    let r = reply(&mut mmu, "PostToolUse", &p);
    assert_eq!(
        mmu.cas.artifact_count(),
        before + 1,
        "output must be ingested"
    );
    let text = r.to_string();
    for forbidden in [
        "updatedToolOutput",
        "updatedMCPToolOutput",
        "additionalContext",
        "decision",
    ] {
        assert!(!text.contains(forbidden), "{forbidden} in {r}");
    }
    assert_eq!(
        r["hookSpecificOutput"]["hookEventName"], "PostToolUse",
        "{r}"
    );
    assert!(!has_null(&r), "{r}");
}

#[test]
fn bash_json_string_is_unwrapped_to_its_output_text() {
    let mut mmu = InfinityContextMmu::default();
    let response = json!({"output": "line-a\nline-b\n", "exit_code": 3}).to_string();
    let mut p = base("PostToolUse");
    p["tool_name"] = json!("bash");
    p["tool_input"] = json!({"command": "false"});
    p["tool_response"] = json!(response);
    let _ = reply(&mut mmu, "PostToolUse", &p);
    let id = mmu
        .hdc_index
        .latest_entry()
        .expect("an artifact was indexed")
        .artifact_id;
    let stored = String::from_utf8(mmu.cas.deref_bytes(id).unwrap()).unwrap();
    assert!(
        stored.contains("line-a") && !stored.contains("\"exit_code\""),
        "{stored:?}"
    );
}

#[test]
fn plain_text_tool_responses_and_failures_are_captured() {
    let mut mmu = InfinityContextMmu::default();
    let before = mmu.cas.artifact_count();
    let mut p = base("PostToolUse");
    p["tool_name"] = json!("read_file");
    p["tool_input"] = json!({"path": "src/main.rs"});
    p["tool_response"] = json!("fn main() {}\n");
    let _ = reply(&mut mmu, "PostToolUse", &p);
    let mut f = base("PostToolUseFailure");
    f["tool_name"] = json!("bash");
    f["tool_input"] = json!({"command": "cargo test"});
    f["error"] = json!("test failed: assertion `left == right`");
    let r = reply(&mut mmu, "PostToolUseFailure", &f);
    assert_eq!(mmu.cas.artifact_count(), before + 2);
    assert!(!r.to_string().contains("additionalContext"), "{r}");
}

#[test]
fn every_other_event_gets_an_empty_reply() {
    let mut mmu = InfinityContextMmu::default();
    for event in [
        "PreToolUse",
        "PreCompact",
        "PostCompact",
        "SubagentStart",
        "SubagentStop",
        "Stop",
        "SessionEnd",
        "PostToolBatch",
        "PreLLMCall",
        "PostLLMCall",
        "Notification",
        "UnknownFutureEvent",
    ] {
        let mut p = base(event);
        p["subagent_id"] = json!("agent-1");
        p["tool_name"] = json!("bash");
        p["tool_input"] = json!({"command": "ls"});
        let r = reply(&mut mmu, event, &p);
        assert_eq!(r, json!({}), "{event}: {r}");
    }
}

#[test]
fn malformed_stdin_is_an_error_not_a_panic() {
    let mut mmu = InfinityContextMmu::default();
    assert!(MuseAdapter::handle_hook("PostToolUse", "{not json", &mut mmu).is_err());
    let empty =
        MuseAdapter::handle_hook("SessionStart", "", &mut mmu).expect("empty stdin is allowed");
    assert!(empty.contains("SessionStart"));
}

#[test]
fn icx_pager_calls_null_responses_and_subagent_folds_are_handled() {
    let mut mmu = InfinityContextMmu::default();
    let before = mmu.cas.artifact_count();
    // The icx MCP pager's own results are never re-ingested.
    let mut pager = base("PostToolUse");
    pager["tool_name"] = json!("mcp__icx__icx");
    pager["tool_input"] = json!({"op": "status"});
    pager["tool_response"] = json!("<<ICX:STATUS>>");
    let r = reply(&mut mmu, "PostToolUse", &pager);
    assert_eq!(r["hookSpecificOutput"]["hookEventName"], "PostToolUse");
    assert_eq!(mmu.cas.artifact_count(), before);

    // A null tool_response is stored as empty output, not an error.
    let mut nul = base("PostToolUse");
    nul["tool_name"] = json!("bash");
    nul["tool_input"] = json!({"command": "true"});
    nul["tool_response"] = Value::Null;
    assert!(!has_null(&reply(&mut mmu, "PostToolUse", &nul)));

    // A failure that carries a tool_response keeps that text and its exit code.
    let mut fail = base("PostToolUseFailure");
    fail["tool_name"] = json!("bash");
    fail["tool_input"] = json!({"command": "make"});
    fail["tool_response"] =
        json!(json!({"output": "make: *** [all] Error 2", "exit_code": 2}).to_string());
    assert_eq!(reply(&mut mmu, "PostToolUseFailure", &fail), json!({}));

    // Subagent start/stop with a summary, then compaction and session end.
    let mut start = base("SubagentStart");
    start["subagent_id"] = json!("agent-7");
    assert_eq!(reply(&mut mmu, "SubagentStart", &start), json!({}));
    let mut stop = base("SubagentStop");
    stop["subagent_id"] = json!("agent-7");
    stop["last_assistant_message"] = json!("found the failing test in epoch.rs");
    assert_eq!(reply(&mut mmu, "SubagentStop", &stop), json!({}));
    for event in ["PostCompact", "SessionEnd"] {
        assert_eq!(reply(&mut mmu, event, &base(event)), json!({}));
    }
    assert!(mmu.cas.artifact_count() > before);
}

#[test]
fn the_trait_object_reports_muse_and_delegates() {
    use icx_adapters::AgentHookAdapter;
    let mut mmu = InfinityContextMmu::default();
    let adapter: &dyn AgentHookAdapter = &MuseAdapter;
    assert_eq!(adapter.agent_kind(), AgentKind::Muse);
    let out = adapter
        .handle_hook_event("Notification", "{}", &mut mmu)
        .expect("reply");
    assert_eq!(out, "{}");
    const { assert!(!MuseAdapter::SUPPORTS_TOOL_OUTPUT_REPLACEMENT) };
}

/// PR #27 review: the bus decoded `AgentKind::Muse` (8) as `GenericMcp`, so Muse's own events
/// came back to Muse as sibling telepathy frames.
#[test]
fn muse_does_not_receive_its_own_events_as_sibling_activity() {
    let mut mmu = InfinityContextMmu::default();
    for i in 0..3 {
        let mut p = base("PostToolUse");
        p["tool_name"] = json!("bash");
        p["tool_input"] = json!({"command": format!("echo turn{i}")});
        p["tool_response"] =
            json!(json!({"output": format!("turn{i}\n"), "exit_code": 0}).to_string());
        let _ = reply(&mut mmu, "PostToolUse", &p);
    }
    let mut prompt = base("UserPromptSubmit");
    prompt["prompt"] = json!("what did I just run");
    let r = reply(&mut mmu, "UserPromptSubmit", &prompt);
    let ctx = r["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(!ctx.contains("Θ:muse@") && !ctx.contains("Θ:mcp@"), "{ctx}");
}

/// PR #27 review (Codex): only the Bash tool's JSON envelope is unwrapped; another tool's text that
/// happens to be JSON with an `output` member is stored unchanged.
#[test]
fn only_bash_results_are_unwrapped() {
    let mut mmu = InfinityContextMmu::default();
    let file_text = r#"{"output":"result","metadata":"important"}"#;
    let mut p = base("PostToolUse");
    p["tool_name"] = json!("read_file");
    p["tool_input"] = json!({"path": "data.json"});
    p["tool_response"] = json!(file_text);
    let _ = reply(&mut mmu, "PostToolUse", &p);
    let id = mmu.hdc_index.latest_entry().expect("indexed").artifact_id;
    let stored = String::from_utf8(mmu.cas.deref_bytes(id).unwrap()).unwrap();
    assert!(
        stored.contains("metadata") && stored.contains("important"),
        "{stored:?}"
    );
}

/// PR #27 review (Antigravity): subagent identity falls back to `child_session_id`, and a
/// PostCompact `compact_summary` is kept.
#[test]
fn subagent_fallback_ids_and_compact_summaries() {
    let mut mmu = InfinityContextMmu::default();
    let mut start = base("SubagentStart");
    start["child_session_id"] = json!("child-9");
    assert_eq!(reply(&mut mmu, "SubagentStart", &start), json!({}));
    let mut stop = base("SubagentStop");
    stop["child_session_id"] = json!("child-9");
    stop["last_assistant_message"] = json!("child done");
    assert_eq!(reply(&mut mmu, "SubagentStop", &stop), json!({}));
    // No identity at all still opens and folds one default branch without error.
    assert_eq!(
        reply(&mut mmu, "SubagentStart", &base("SubagentStart")),
        json!({})
    );
    assert_eq!(
        reply(&mut mmu, "SubagentStop", &base("SubagentStop")),
        json!({})
    );

    let before = mmu.cas.artifact_count();
    let mut post = base("PostCompact");
    post["compact_summary"] = json!("Summary: fixed the eviction planner and its tests.");
    assert_eq!(reply(&mut mmu, "PostCompact", &post), json!({}));
    assert_eq!(mmu.cas.artifact_count(), before + 1);
    let id = mmu.hdc_index.latest_entry().expect("indexed").artifact_id;
    let stored = String::from_utf8(mmu.cas.deref_bytes(id).unwrap()).unwrap();
    assert!(stored.contains("eviction planner"), "{stored:?}");
}

/// Review of #27 (round 4): `toolName` is honoured for the pager check, and SubagentStop falls back
/// to `summary` / `outcome`.
#[test]
fn camel_case_tool_name_and_summary_fallbacks() {
    let mut mmu = InfinityContextMmu::default();
    let before = mmu.cas.artifact_count();
    let mut pager = base("PostToolUse");
    pager["toolName"] = json!("mcp__icx__icx");
    pager["tool_input"] = json!({"op": "status"});
    pager["tool_response"] = json!("status text");
    let _ = reply(&mut mmu, "PostToolUse", &pager);
    assert_eq!(
        mmu.cas.artifact_count(),
        before,
        "pager output must not be re-ingested"
    );
    let mut stop = base("SubagentStop");
    stop["subagent_id"] = json!("agent-3");
    stop["summary"] = json!("summarised via the summary field");
    assert_eq!(reply(&mut mmu, "SubagentStop", &stop), json!({}));
}

/// Review of #27: `toolResponse` is honoured and an inline JSON object Bash result is unwrapped.
#[test]
fn camel_case_tool_response_and_inline_bash_objects() {
    let mut mmu = InfinityContextMmu::default();
    let mut p = base("PostToolUse");
    p["tool_name"] = json!("bash");
    p["tool_input"] = json!({"command": "echo hi"});
    p["toolResponse"] = json!({"output": "hi-from-object\n", "exit_code": 0});
    let _ = reply(&mut mmu, "PostToolUse", &p);
    let id = mmu.hdc_index.latest_entry().expect("indexed").artifact_id;
    let stored = String::from_utf8(mmu.cas.deref_bytes(id).unwrap()).unwrap();
    assert!(
        stored.contains("hi-from-object") && !stored.contains("exit_code"),
        "{stored:?}"
    );
}

/// Review of #27 (23:40 UTC): a failure's error message is kept alongside partial output.
#[test]
fn failure_error_text_is_appended_to_partial_output() {
    let mut mmu = InfinityContextMmu::default();
    let mut f = base("PostToolUseFailure");
    f["tool_name"] = json!("read_file");
    f["tool_input"] = json!({"path": "a.txt"});
    f["tool_response"] = json!("partial-output-line");
    f["error"] = json!("permission denied reading a.txt");
    let _ = reply(&mut mmu, "PostToolUseFailure", &f);
    let id = mmu.hdc_index.latest_entry().expect("indexed").artifact_id;
    let stored = String::from_utf8(mmu.cas.deref_bytes(id).unwrap()).unwrap();
    assert!(
        stored.contains("partial-output-line") && stored.contains("permission denied"),
        "{stored:?}"
    );
}

/// Review of #27 (23:59 UTC): an empty error string falls back to the default failure text.
#[test]
fn empty_error_strings_fall_back() {
    let mut mmu = InfinityContextMmu::default();
    let mut f = base("PostToolUseFailure");
    f["tool_name"] = json!("read_file");
    f["tool_input"] = json!({"path": "a.txt"});
    f["error"] = json!("   ");
    let _ = reply(&mut mmu, "PostToolUseFailure", &f);
    let id = mmu.hdc_index.latest_entry().expect("indexed").artifact_id;
    let stored = String::from_utf8(mmu.cas.deref_bytes(id).unwrap()).unwrap();
    assert!(stored.contains("tool call failed"), "{stored:?}");
}

/// Review of #27 (00:49 UTC): a structured error object is kept, not replaced by the default.
#[test]
fn structured_error_objects_are_kept() {
    let mut mmu = InfinityContextMmu::default();
    let mut f = base("PostToolUseFailure");
    f["tool_name"] = json!("read_file");
    f["tool_input"] = json!({"path": "a.txt"});
    f["error"] = json!({"message": "disk quota exceeded"});
    let _ = reply(&mut mmu, "PostToolUseFailure", &f);
    let id = mmu.hdc_index.latest_entry().expect("indexed").artifact_id;
    let stored = String::from_utf8(mmu.cas.deref_bytes(id).unwrap()).unwrap();
    assert!(stored.contains("disk quota exceeded"), "{stored:?}");
}

/// Review of #27 (01:01 UTC): a Bash result without `output` keeps its exit code.
#[test]
fn bash_exit_code_without_output_is_kept() {
    let mut mmu = InfinityContextMmu::default();
    let mut f = base("PostToolUseFailure");
    f["tool_name"] = json!("bash");
    f["tool_input"] = json!({"command": "sleep 99"});
    f["tool_response"] = json!(r#"{"output":null,"exit_code":124}"#);
    let _ = reply(&mut mmu, "PostToolUseFailure", &f);
    let entry = mmu.hdc_index.latest_entry().expect("indexed");
    let stored = String::from_utf8(mmu.cas.deref_bytes(entry.artifact_id).unwrap()).unwrap();
    // Unwrapped (not the raw JSON text), with the failure text in place of the missing output.
    assert!(
        !stored.contains("exit_code") && stored.contains("tool call failed"),
        "{stored:?}"
    );
}

/// Review of #27 (01:24 UTC): `stdout`/`stderr` stand in for a missing `output`; array errors
/// are kept.
#[test]
fn bash_streams_and_array_errors() {
    let mut mmu = InfinityContextMmu::default();
    let mut p = base("PostToolUse");
    p["tool_name"] = json!("bash");
    p["tool_input"] = json!({"command": "make"});
    p["tool_response"] = json!(r#"{"stdout":"built-ok","stderr":"warn-1","exit_code":0}"#);
    let _ = reply(&mut mmu, "PostToolUse", &p);
    let id = mmu.hdc_index.latest_entry().expect("indexed").artifact_id;
    let stored = String::from_utf8(mmu.cas.deref_bytes(id).unwrap()).unwrap();
    assert!(
        stored.contains("built-ok") && stored.contains("warn-1") && !stored.contains("exit_code"),
        "{stored:?}"
    );
    let mut f = base("PostToolUseFailure");
    f["tool_name"] = json!("read_file");
    f["tool_input"] = json!({"path": "b.txt"});
    f["error"] = json!(["eacces", "b.txt"]);
    let _ = reply(&mut mmu, "PostToolUseFailure", &f);
    let id = mmu.hdc_index.latest_entry().expect("indexed").artifact_id;
    let stored = String::from_utf8(mmu.cas.deref_bytes(id).unwrap()).unwrap();
    assert!(stored.contains("eacces"), "{stored:?}");
}
