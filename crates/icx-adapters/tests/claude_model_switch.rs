//! T-64-005: a Claude Code model switch seals the cache epoch, using `PostModelSwitch` only.

use icx_adapters::{ClaudeCodeAdapter, InfinityContextMmu};
use icx_core::AgentKind;
use serde_json::json;

fn switch_payload(event: &str) -> String {
    json!({
        "session_id": "s1",
        "transcript_path": "/tmp/t.jsonl",
        "cwd": "/home/u/project",
        "permission_mode": "default",
        "hook_event_name": event,
        "from_model": "claude-opus-5-5",
        "to_model": "claude-haiku-5-5",
        "an_undocumented_future_field": 1
    })
    .to_string()
}

#[test]
fn post_model_switch_seals_the_epoch_and_says_nothing() {
    let mut mmu = InfinityContextMmu::default();
    mmu.ingest_payload(AgentKind::ClaudeCode, "work", "before the switch\n", false);
    let before = mmu.epochs.sealed_epoch_count();
    let out = ClaudeCodeAdapter::handle_hook(
        "PostModelSwitch",
        &switch_payload("PostModelSwitch"),
        &mut mmu,
    )
    .unwrap();
    assert_eq!(out, "{}");
    assert_eq!(
        mmu.epochs.sealed_epoch_count(),
        before + 1,
        "the epoch is sealed"
    );
}

#[test]
fn pre_model_switch_expresses_no_decision() {
    // icx does not register PreModelSwitch; if a user does, the reply must never block.
    let mut mmu = InfinityContextMmu::default();
    let out = ClaudeCodeAdapter::handle_hook(
        "PreModelSwitch",
        &switch_payload("PreModelSwitch"),
        &mut mmu,
    )
    .unwrap();
    assert_eq!(out, "{}");
}
