// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later AND AGPL-3.0-or-later

//! What icx adds to every user prompt (v1.5.0, ADR-0010 owner decision): at most one recall
//! line, only for a hit that shares a word with the prompt, and no graph lines. Other events keep
//! the full tail slot.

use icx_adapters::{ClaudeCodeAdapter, InfinityContextMmu};
use icx_core::AgentKind;

fn mmu() -> InfinityContextMmu {
    let mut m = InfinityContextMmu::default();
    for i in 0..30 {
        m.ingest_payload(
            AgentKind::ClaudeCode,
            &format!("Bash:cargo test --package crate{i}"),
            &format!("running 4 tests\ntest parser_{i} ... ok\ntest result: ok. 4 passed\n"),
            false,
        );
    }
    m.ingest_payload(
        AgentKind::ClaudeCode,
        "Read:src/redactor.rs",
        "pub fn redact_secrets(input: &str) -> String { todo!() }\n",
        false,
    );
    m
}

fn recall_lines(tail: &str) -> Vec<&str> {
    tail.lines().filter(|l| l.contains("HDC_RECALL")).collect()
}

#[test]
fn a_prompt_gets_at_most_one_recall_line_and_no_graph_lines() {
    let mut m = mmu();
    let tail = m.build_prompt_tail_slot(AgentKind::ClaudeCode, "why did the parser tests pass");
    assert!(recall_lines(&tail).len() <= 1, "{tail}");
    assert!(!tail.contains("PPR_GRAPH"), "{tail}");
    // The full slot, for comparison, has more.
    let full =
        m.build_ephemeral_tail_slot(AgentKind::ClaudeCode, "why did the parser tests pass", 4);
    assert!(
        recall_lines(&full).len() + full.matches("PPR_GRAPH").count() > 1,
        "{full}"
    );
}

#[test]
fn the_recall_line_needs_a_shared_word() {
    let mut m = mmu();
    // A real word in common: shown.
    let tail = m.build_prompt_tail_slot(AgentKind::ClaudeCode, "look at redact_secrets again");
    let lines = recall_lines(&tail);
    assert_eq!(lines.len(), 1, "{tail}");
    assert!(lines[0].contains("redactor"), "{tail}");
    // No word in common (stop words and short words do not count): nothing shown.
    let none = m.build_prompt_tail_slot(AgentKind::ClaudeCode, "what is it now and then?");
    assert_eq!(recall_lines(&none), Vec::<&str>::new(), "{none}");
}

#[test]
fn the_prompt_hook_uses_the_prompt_slot_and_session_start_keeps_the_full_one() {
    let mut m = mmu();
    let reply = ClaudeCodeAdapter::handle_hook(
        "UserPromptSubmit",
        r#"{"prompt":"why did the parser tests pass"}"#,
        &mut m,
    )
    .unwrap();
    let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
    let ctx = v["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(
        recall_lines(ctx).len() <= 1 && !ctx.contains("PPR_GRAPH"),
        "{ctx}"
    );
    let start = ClaudeCodeAdapter::handle_hook("SessionStart", "{}", &mut m).unwrap();
    assert!(start.contains("ICX:TAIL_SLOT"), "{start}");
}
