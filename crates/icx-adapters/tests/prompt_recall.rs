// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later AND AGPL-3.0-or-later

//! What icx adds to every user prompt (v1.5.0, ADR-0010 owner decision): at most one recall
//! line, only for a hit whose subject shares a distinctive word with the prompt, and no graph
//! lines. Other events keep the full tail slot.

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
fn several_matching_hits_still_give_one_line() {
    let mut m = mmu();
    // Each word is in exactly one subject, so up to three hits match.
    let tail = m.build_prompt_tail_slot(AgentKind::ClaudeCode, "compare crate1 crate2 crate3");
    assert_eq!(recall_lines(&tail).len(), 1, "{tail}");
    assert!(!tail.contains("PPR_GRAPH"), "{tail}");
}

#[test]
fn a_word_in_many_subjects_does_not_count() {
    let mut m = mmu();
    // "cargo", "test" and "package" are in 30 of 31 subjects, over the cap of 3.
    let tail = m.build_prompt_tail_slot(AgentKind::ClaudeCode, "run cargo package again");
    assert!(recall_lines(&tail).is_empty(), "{tail}");
}

#[test]
fn a_capitalized_stop_word_does_not_count() {
    let mut m = mmu();
    m.ingest_payload(AgentKind::ClaudeCode, "Bash:echo what", "what\n", false);
    // "What" is a stop word whatever its case (review of #101).
    let tail = m.build_prompt_tail_slot(AgentKind::ClaudeCode, "What now, Then?");
    assert!(recall_lines(&tail).is_empty(), "{tail}");
}

#[test]
fn a_candidate_too_weak_to_show_does_not_hide_one_that_would_be() {
    let mut m = InfinityContextMmu::default();
    // Unrelated text gives the first subject almost no HDC similarity to the prompt.
    m.ingest_payload(AgentKind::ClaudeCode, "Bash:zzqx", "\u{1}\n", false);
    m.ingest_payload(
        AgentKind::ClaudeCode,
        "Read:src/zzqx_parser.rs",
        "fn zzqx_parser() { parse the zzqx stream }\n",
        false,
    );
    for i in 0..4 {
        m.ingest_payload(
            AgentKind::ClaudeCode,
            &format!("Bash:echo filler{i}"),
            "filler\n",
            false,
        );
    }
    let tail = m.build_prompt_tail_slot(AgentKind::ClaudeCode, "zzqx parser stream");
    let lines = recall_lines(&tail);
    assert_eq!(lines.len(), 1, "{tail}");
    assert!(lines[0].contains("zzqx"), "{tail}");
}

#[test]
fn the_prompt_slot_keeps_cross_agent_activity() {
    let mut m = mmu();
    m.ingest_payload(
        AgentKind::Codex,
        "Bash:cargo build --release",
        "Finished release\n",
        false,
    );
    let tail = m.build_prompt_tail_slot(AgentKind::ClaudeCode, "what now");
    assert!(tail.contains("codex"), "sibling activity is kept: {tail}");
}

#[test]
fn fused_ranking_checks_every_candidate_like_legacy() {
    let mut legacy = mmu();
    let mut fused = mmu();
    fused.set_lexical_index(true);
    let mut shown = 0;
    for i in 0..30 {
        let q = format!("did crate{i} tests pass");
        let a = legacy.build_prompt_tail_slot(AgentKind::ClaudeCode, &q);
        let b = fused.build_prompt_tail_slot(AgentKind::ClaudeCode, &q);
        assert_eq!(recall_lines(&a), recall_lines(&b), "{q}");
        shown += recall_lines(&a).len();
    }
    assert!(shown > 0);
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
    assert!(recall_lines(&none).is_empty(), "{none}");
    // A word only in the output text, not the subject, does not count.
    let text_only = m.build_prompt_tail_slot(AgentKind::ClaudeCode, "where is redact_secrets");
    assert!(recall_lines(&text_only).is_empty(), "{text_only}");
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
    // The full slot: recall with no word rule, and graph lines.
    assert!(
        !recall_lines(&start).is_empty() || start.contains("PPR_GRAPH"),
        "{start}"
    );
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
        if line.trim().is_empty() {
            continue;
        }
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
