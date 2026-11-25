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
    // A distinctive word of the subject (`Read:src/redactor.rs`): shown.
    let tail = m.build_prompt_tail_slot(AgentKind::ClaudeCode, "look at the redactor again");
    let lines = recall_lines(&tail);
    assert_eq!(lines.len(), 1, "{tail}");
    assert!(lines[0].contains("redactor"), "{tail}");
    // No word in common (stop words and short words do not count): nothing shown.
    let none = m.build_prompt_tail_slot(AgentKind::ClaudeCode, "what is it now and then?");
    assert_eq!(recall_lines(&none), Vec::<&str>::new(), "{none}");
    // A word only in the output text, not the subject, does not count.
    let text_only = m.build_prompt_tail_slot(AgentKind::ClaudeCode, "where is redact_secrets");
    assert_eq!(recall_lines(&text_only), Vec::<&str>::new(), "{text_only}");
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

/// Measurement (run with `ICX_MEASURE_WORKSPACE=<workspace dir> ICX_MEASURE_PROMPTS=<prompt set
/// .jsonl> cargo test --release -p icx-adapters --test prompt_recall measure -- --ignored
/// --nocapture`): for each prompt, whether a recall line is shown with no word rule (the best HDC
/// hit alone) and with the shipped distinctive-word rule, and whether the shown line is one of
/// the outputs that turn used. The workspace is loaded, never saved. Prints counts only.
#[test]
#[ignore = "measurement; needs ICX_MEASURE_WORKSPACE and ICX_MEASURE_PROMPTS"]
fn measure_prompt_recall_lines() {
    let (Ok(dir), Ok(set)) = (
        std::env::var("ICX_MEASURE_WORKSPACE"),
        std::env::var("ICX_MEASURE_PROMPTS"),
    ) else {
        return;
    };
    let mut m = InfinityContextMmu::default();
    m.load_workspace_dir(std::path::Path::new(&dir)).unwrap();
    let (mut n, mut unfiltered, mut shipped, mut unfiltered_hit, mut shipped_hit) = (0, 0, 0, 0, 0);
    for line in std::fs::read_to_string(&set).unwrap().lines() {
        let case: serde_json::Value = serde_json::from_str(line).unwrap();
        let q = case["query"].as_str().unwrap_or_default();
        let relevant: Vec<String> = case["relevant"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        n += 1;
        let uses = |t: &str| relevant.iter().any(|h| t.contains(h.as_str()));
        let full = m.build_ephemeral_tail_slot(AgentKind::ClaudeCode, q, 1);
        let first = full.lines().find(|l| l.contains("HDC_RECALL"));
        if let Some(l) = first {
            unfiltered += 1;
            unfiltered_hit += usize::from(uses(l));
        }
        let tail = m.build_prompt_tail_slot(AgentKind::ClaudeCode, q);
        if let Some(l) = tail.lines().find(|l| l.contains("HDC_RECALL")) {
            shipped += 1;
            shipped_hit += usize::from(uses(l));
        }
    }
    println!(
        "prompts={n} | best hit, no word rule: shown {unfiltered}, used {unfiltered_hit} | \
         distinctive-word rule: shown {shipped}, used {shipped_hit}"
    );
}
