//! T-64-006: Claude Code `additionalContext` stays within 8,000 characters (UTF-16 units) and
//! 200 lines, keeps whole lines (so `#icx:` handles stay intact), and always names its event.

use icx_adapters::claude::{
    CLAUDE_CONTEXT_MAX_CHARS, CLAUDE_CONTEXT_MAX_LINES, claude_bounded_context,
};
use icx_adapters::{ClaudeCodeAdapter, InfinityContextMmu};
use icx_core::AgentKind;
use proptest::prelude::*;
use serde_json::{Value, json};

fn units(s: &str) -> usize {
    s.encode_utf16().count()
}

fn within(s: &str) -> bool {
    units(s) <= CLAUDE_CONTEXT_MAX_CHARS && s.split('\n').count() <= CLAUDE_CONTEXT_MAX_LINES
}

#[test]
fn small_context_is_unchanged() {
    let s = "⟪ICX:TAIL_SLOT│agent=claude-code⟫\n⟦Ψ:HDC_RECALL⊢subj=x↦#icx:0123456789abcdef⟧";
    assert_eq!(claude_bounded_context(s), s);
}

#[test]
fn too_many_lines_drops_the_lowest_ranked_and_says_so() {
    let lines: Vec<String> = (0..500)
        .map(|i| format!("⟦Ψ:HDC_RECALL⊢rank={i}↦#icx:{i:016x}⟧"))
        .collect();
    let out = claude_bounded_context(&lines.join("\n"));
    assert!(within(&out));
    let kept: Vec<&str> = out.split('\n').collect();
    let last = kept.last().unwrap();
    assert!(last.starts_with("⟪ICX:TRUNCATED│dropped="), "{last}");
    // The best-ranked lines survive, in order, each intact.
    for (i, line) in kept[..kept.len() - 1].iter().enumerate() {
        assert_eq!(*line, lines[i]);
    }
    let dropped: usize = last
        .trim_start_matches("⟪ICX:TRUNCATED│dropped=")
        .trim_end_matches('⟫')
        .parse()
        .unwrap();
    assert_eq!(dropped + kept.len() - 1, lines.len());
}

#[test]
fn wide_characters_are_counted_as_javascript_does() {
    // Each U+1D400 is two UTF-16 units: 3,000 of them is 6,000 units on one line.
    let wide = "\u{1D400}".repeat(3_000);
    let text = format!("{wide}\n{wide}");
    assert!(units(&text) > CLAUDE_CONTEXT_MAX_CHARS);
    let out = claude_bounded_context(&text);
    assert!(within(&out));
    assert!(
        out.starts_with(&wide),
        "the first line fits and is kept whole"
    );
}

#[test]
fn every_context_reply_names_its_own_event() {
    let mut mmu = InfinityContextMmu::default();
    for i in 0..50 {
        mmu.ingest_payload(
            AgentKind::ClaudeCode,
            &format!("subject_{i}"),
            &format!("output {i}\n"),
            false,
        );
    }
    for (event, payload) in [
        ("SessionStart", json!({})),
        ("UserPromptSubmit", json!({"prompt": "output 3"})),
        ("SubagentStart", json!({"agent_id": "a1"})),
    ] {
        let out = ClaudeCodeAdapter::handle_hook(event, &payload.to_string(), &mut mmu).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["hookSpecificOutput"]["hookEventName"], event);
        let ctx = v["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap();
        assert!(within(ctx), "{event}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn no_reply_exceeds_the_limit_and_handles_stay_whole(
        lines in prop::collection::vec(
            ("[a-z⟦⟧Ψ\u{1D400} ]{0,120}", 0u64..u64::MAX),
            0..400
        )
    ) {
        let text = lines
            .iter()
            .map(|(body, id)| format!("{body}↦#icx:{id:016x}"))
            .collect::<Vec<_>>()
            .join("\n");
        let out = claude_bounded_context(&text);
        prop_assert!(within(&out));
        prop_assert_eq!(&out, &claude_bounded_context(&text), "deterministic");
        if within(&text) {
            prop_assert_eq!(&out, &text);
        }
        // Every kept line is an input line, so no handle is cut.
        let originals: std::collections::HashSet<&str> = text.split('\n').collect();
        for line in out.split('\n').filter(|l| !l.starts_with("⟪ICX:TRUNCATED")) {
            prop_assert!(originals.contains(line));
        }
    }
}
