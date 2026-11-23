//! T-65-001 and T-65-008: the agy adapter against payloads recorded live on agy 1.5.14
//! (`tests/fixtures/agy/1.5.14/`, sanitized by `scripts/sanitize-hook-fixtures.py`).
//!
//! The recorded session ran one shell command (`run_command`) and one call of icx's own MCP
//! tool, which agy loaded lazily and so named `call_mcp_tool` with `ServerName`/`ToolName`.

use icx_adapters::{AntigravityAdapter, InfinityContextMmu};
use serde_json::{Value, json};
use std::path::Path;

const DIR: &str = "tests/fixtures/agy/1.5.14";

fn fixture(name: &str) -> String {
    std::fs::read_to_string(Path::new(DIR).join(format!("{name}.json")))
        .unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

fn reply(event: &str, name: &str, mmu: &mut InfinityContextMmu) -> Value {
    let out = AntigravityAdapter::handle_hook(event, &fixture(name), mmu).unwrap();
    serde_json::from_str(&out).unwrap()
}

#[test]
fn context_events_inject_a_system_message_step() {
    let mut mmu = InfinityContextMmu::default();
    for (event, name) in [
        ("SessionStart", "SessionStart"),
        ("PreInvocation", "PreInvocation"),
        ("PostInvocation", "PostInvocation"),
    ] {
        let r = reply(event, name, &mut mmu);
        let step = &r["injectSteps"][0]["systemMessage"];
        assert!(
            step["systemMessage"]
                .as_str()
                .is_some_and(|s| s.contains("ICX:")),
            "{event}: {r}"
        );
        assert!(r.get("terminationBehavior").is_some(), "{event}");
    }
}

#[test]
fn a_shell_result_is_ingested_and_the_reply_is_empty() {
    let mut mmu = InfinityContextMmu::default();
    let before = mmu.cas.artifact_count();
    assert_eq!(reply("PostToolUse", "PostToolUse", &mut mmu), json!({}));
    // The recorded payload carries the call's arguments; its transcript is not in the fixture.
    assert!(mmu.cas.artifact_count() > before, "the call is recorded");
}

#[test]
fn icx_own_tool_is_exempt_in_its_lazy_call_mcp_tool_form() {
    let raw: Value = serde_json::from_str(&fixture("PostToolUse-2")).unwrap();
    assert_eq!(raw["toolCall"]["name"], "call_mcp_tool");
    assert_eq!(raw["toolCall"]["args"]["ServerName"], "icx");

    let mut mmu = InfinityContextMmu::default();
    let before = mmu.cas.artifact_count();
    assert_eq!(reply("PostToolUse", "PostToolUse-2", &mut mmu), json!({}));
    assert_eq!(
        mmu.cas.artifact_count(),
        before,
        "icx never ingests its own reply"
    );

    // The same exemption for `icx_write`, and for the eager `mcp_<server>_<tool>` form.
    for (name, args) in [
        (
            "call_mcp_tool",
            json!({"ServerName": "icx", "ToolName": "icx_write"}),
        ),
        ("mcp_icx_icx", json!({})),
        ("mcp_icx_icx_write", json!({})),
    ] {
        let mut p = raw.clone();
        p["toolCall"]["name"] = json!(name);
        p["toolCall"]["args"] = args;
        let out = AntigravityAdapter::handle_hook("PostToolUse", &p.to_string(), &mut mmu).unwrap();
        assert_eq!(out, "{}", "{name}");
    }
    assert_eq!(mmu.cas.artifact_count(), before);

    // Another server reached through `call_mcp_tool` is still ingested.
    let mut other = raw;
    other["toolCall"]["args"] =
        json!({"ServerName": "github", "ToolName": "search", "Arguments": {"q": "x"}});
    AntigravityAdapter::handle_hook("PostToolUse", &other.to_string(), &mut mmu).unwrap();
    assert!(
        mmu.cas.artifact_count() > before,
        "other servers are ingested"
    );
}

#[test]
fn stop_always_stops() {
    let mut mmu = InfinityContextMmu::default();
    assert_eq!(reply("Stop", "Stop", &mut mmu), json!({"decision": "stop"}));
}
