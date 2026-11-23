//! T-65-001: the Muse adapter against payloads recorded live on Muse Code 1.4.25
//! (`tests/fixtures/muse/1.4.25/`, sanitized by `scripts/sanitize-hook-fixtures.py`).
//!
//! The recorded session called icx's two tools from a test server named `icxtest` (Muse
//! refuses to override a configured server name per run), so they arrive as
//! `mcp__icxtest__icx` and `mcp__icxtest__icx_write`, then Muse's own `submit_reminder_decision`.

use icx_adapters::{InfinityContextMmu, MuseAdapter};
use serde_json::{Value, json};
use std::path::Path;

const DIR: &str = "tests/fixtures/muse/1.4.25";

fn fixture(name: &str) -> String {
    std::fs::read_to_string(Path::new(DIR).join(format!("{name}.json")))
        .unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

fn reply(event: &str, name: &str, mmu: &mut InfinityContextMmu) -> Value {
    let out = MuseAdapter::handle_hook(event, &fixture(name), mmu).unwrap();
    serde_json::from_str(&out).unwrap()
}

#[test]
fn context_events_add_additional_context_naming_the_event() {
    let mut mmu = InfinityContextMmu::default();
    for event in ["SessionStart", "UserPromptSubmit"] {
        let r = reply(event, event, &mut mmu);
        assert_eq!(r["hookSpecificOutput"]["hookEventName"], event);
        assert!(
            r["hookSpecificOutput"]["additionalContext"]
                .as_str()
                .is_some_and(|c| c.contains("ICX:"))
        );
    }
}

#[test]
fn icx_tools_are_exempt_and_other_tools_are_ingested_never_replaced() {
    let mut mmu = InfinityContextMmu::default();
    for name in ["PostToolUse", "PostToolUse-2"] {
        let raw: Value = serde_json::from_str(&fixture(name)).unwrap();
        assert!(
            raw["tool_name"]
                .as_str()
                .unwrap()
                .starts_with("mcp__icxtest__")
        );
        let r = reply("PostToolUse", name, &mut mmu);
        assert_eq!(
            r,
            json!({"hookSpecificOutput": {"hookEventName": "PostToolUse"}})
        );
    }
    assert_eq!(
        mmu.cas.artifact_count(),
        0,
        "icx's own tools are not ingested"
    );

    // Muse rejects replacing tool output, so icx only ever names the event.
    let r = reply("PostToolUse", "PostToolUse-3", &mut mmu);
    assert_eq!(
        r,
        json!({"hookSpecificOutput": {"hookEventName": "PostToolUse"}})
    );
    assert_eq!(
        mmu.cas.artifact_count(),
        1,
        "Muse's own tool result is stored"
    );
}

#[test]
fn stop_and_session_end_reply_empty() {
    let mut mmu = InfinityContextMmu::default();
    for event in ["Stop", "SessionEnd"] {
        assert_eq!(reply(event, event, &mut mmu), json!({}), "{event}");
    }
}
