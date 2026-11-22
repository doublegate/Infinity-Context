//! v1.3.1 (#30): fail-open replies have the same shape as each adapter's own replies, so the
//! CLI and the adapters cannot drift apart.
use icx_adapters::{AntigravityAdapter, InfinityContextMmu, fail_open_reply};
use icx_core::AgentKind;
use serde_json::Value;
use std::collections::BTreeSet;

fn keys(v: &Value) -> BTreeSet<String> {
    v.as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default()
}

#[test]
fn fail_open_reply_matches_agy_shapes() {
    for event in [
        "SessionStart",
        "PreInvocation",
        "PostInvocation",
        "PreToolUse",
        "PostToolUse",
        "SubagentStart",
        "SubagentStop",
        "Stop",
        "SessionEnd",
        "SomethingNew",
    ] {
        let mut mmu = InfinityContextMmu::default();
        let real: Value = serde_json::from_str(
            &AntigravityAdapter::handle_hook(event, "{}", &mut mmu).expect("agy reply"),
        )
        .unwrap();
        let fail: Value =
            serde_json::from_str(&fail_open_reply(AgentKind::AntigravityCli, event)).unwrap();
        assert_eq!(
            keys(&real),
            keys(&fail),
            "{event}: real {real} vs fail-open {fail}"
        );
        if let Some(d) = real.get("decision") {
            assert_eq!(Some(d), fail.get("decision"), "{event}");
        }
    }
    // Case and separators are ignored.
    assert_eq!(
        fail_open_reply(AgentKind::AntigravityCli, "STOP"),
        r#"{"decision":"stop"}"#
    );
    assert_eq!(
        fail_open_reply(AgentKind::AntigravityCli, "pre_tool_use"),
        r#"{"decision":"allow"}"#
    );
}

#[test]
fn fail_open_reply_per_agent() {
    assert_eq!(fail_open_reply(AgentKind::Hermes, "on_turn_complete"), "{}");
    assert_eq!(fail_open_reply(AgentKind::Muse, "PostToolUse"), "{}");
    assert_eq!(
        fail_open_reply(AgentKind::ClaudeCode, "PostToolUse"),
        r#"{"continue":true}"#
    );
    assert_eq!(
        fail_open_reply(AgentKind::Codex, "Stop"),
        r#"{"continue":true}"#
    );
}
