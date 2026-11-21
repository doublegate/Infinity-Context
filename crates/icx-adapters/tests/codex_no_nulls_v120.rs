//! v1.2.0 regression: Codex CLI marks a hook "Failed" when its reply carries an explicit JSON
//! `null` member (`"stopReason": null`, `"systemMessage": null`, `"reason": null`), so no Codex
//! reply may contain a `null` anywhere.
use icx_adapters::{CodexAdapter, InfinityContextMmu};
use serde_json::{Value, json};

fn null_paths(value: &Value, path: &str, out: &mut Vec<String>) {
    match value {
        Value::Null => out.push(path.to_owned()),
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                null_paths(item, &format!("{path}[{i}]"), out);
            }
        }
        Value::Object(map) => {
            for (key, item) in map {
                null_paths(item, &format!("{path}.{key}"), out);
            }
        }
        _ => {}
    }
}

#[test]
fn every_codex_hook_reply_is_free_of_json_null() {
    let mut mmu = InfinityContextMmu::default();
    let events: Vec<(&str, Value)> = vec![
        (
            "SessionStart",
            json!({"session_id": "s1", "source": "startup"}),
        ),
        (
            "UserPromptSubmit",
            json!({"session_id": "s1", "prompt": "explain the cache epoch controller"}),
        ),
        (
            "PreToolUse",
            json!({"session_id": "s1", "tool_name": "Bash", "tool_input": {"command": "ls"}}),
        ),
        (
            "PostToolUse",
            json!({
                "session_id": "s1",
                "tool_name": "Bash",
                "tool_input": {"command": "ls"},
                "tool_response": "a\nb\nc\n".repeat(400)
            }),
        ),
        (
            "PostToolUse",
            json!({"session_id": "s1", "tool_name": "Bash", "tool_input": {"command": "true"}}),
        ),
        (
            "Stop",
            json!({"session_id": "s1", "stop_hook_active": false}),
        ),
        ("PreCompact", json!({"session_id": "s1", "trigger": "auto"})),
        (
            "PostCompact",
            json!({"session_id": "s1", "trigger": "auto"}),
        ),
        (
            "SubagentStart",
            json!({"session_id": "s1", "agent_id": "sub-1", "agent_type": "explorer"}),
        ),
        (
            "SubagentStop",
            json!({"session_id": "s1", "agent_id": "sub-1", "last_assistant_message": "done"}),
        ),
        ("SessionEnd", json!({"session_id": "s1", "reason": "exit"})),
        ("UnknownFutureEvent", json!({})),
    ];
    for (event, payload) in events {
        let reply = CodexAdapter::handle_hook(event, &payload.to_string(), &mut mmu)
            .unwrap_or_else(|e| panic!("{event} failed: {e}"));
        let value: Value = serde_json::from_str(&reply).unwrap();
        let mut nulls = Vec::new();
        null_paths(&value, "$", &mut nulls);
        assert!(
            nulls.is_empty(),
            "{event} reply contains JSON null at {nulls:?}: {reply}"
        );
        assert_eq!(value["continue"], json!(true), "{event}: {reply}");
    }
}
