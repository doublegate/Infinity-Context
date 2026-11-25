// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later AND AGPL-3.0-or-later

//! The resume packet after compaction (v1.5.0, T-73-001 and T-73-002): bounded, deterministic,
//! made of handles that dereference, delivered once per compaction to Claude Code and Codex, kept
//! across a save, and switchable off.

use icx_adapters::{ClaudeCodeAdapter, CodexAdapter, InfinityContextMmu};
use icx_core::AgentKind;
use proptest::prelude::*;

fn filled(n: usize) -> InfinityContextMmu {
    let mut mmu = InfinityContextMmu::default();
    mmu.set_resume_packets(true);
    for i in 0..n {
        mmu.ingest_payload(
            AgentKind::ClaudeCode,
            &format!("Bash:cargo test step-{i} ⟪tricky⟫\nsecond line"),
            &format!("test result: ok. {i} passed\n"),
            false,
        );
    }
    mmu
}

fn handles(packet: &str) -> Vec<String> {
    packet
        .split("↦#icx:")
        .skip(1)
        .map(|h| format!("#icx:{}", &h[..16]))
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]
    #[test]
    fn the_packet_fits_its_budget_and_every_handle_dereferences(
        n in 0usize..40,
        budget in 0usize..4_096,
    ) {
        let mmu = filled(n);
        let packet = mmu.resume_packet(AgentKind::ClaudeCode, budget);
        prop_assert!(packet.len() <= budget, "{} > {budget}", packet.len());
        for h in handles(&packet) {
            let id = mmu.cas.resolve_handle(&h).unwrap();
            prop_assert!(mmu.deref_text(id).is_ok(), "{h} does not dereference");
        }
        // One line per entry, so a subject cannot break the frame.
        prop_assert!(packet.lines().all(|l| !l.contains("second line") || l.starts_with("⟦Ψ:RESUME")));
    }
}

#[test]
fn a_budget_too_small_for_the_header_keeps_the_packet_pending() {
    let mut mmu = filled(3);
    mmu.mark_resume_pending(AgentKind::ClaudeCode);
    assert_eq!(mmu.take_resume_packet(AgentKind::ClaudeCode, 0), None);
    assert_eq!(mmu.take_resume_packet(AgentKind::ClaudeCode, 20), None);
    let packet = mmu
        .take_resume_packet(AgentKind::ClaudeCode, 3_072)
        .expect("still pending");
    assert!(packet.contains("⟦Ψ:RESUME"), "{packet}");
    assert_eq!(mmu.take_resume_packet(AgentKind::ClaudeCode, 3_072), None);
}

#[test]
fn the_outputs_heading_never_stands_alone_and_failures_fill_by_subject() {
    let mut mmu = filled(4);
    for t in ["a", "b", "c"] {
        mmu.ingest_payload_with_exit_code(
            AgentKind::ClaudeCode,
            &format!("Bash:fail-{t}"),
            "error\n",
            Some(1),
            false,
        );
    }
    let full = mmu.resume_packet(AgentKind::ClaudeCode, 3_072);
    let header = full.lines().next().unwrap().len();
    for budget in header..full.len() {
        let p = mmu.resume_packet(AgentKind::ClaudeCode, budget);
        assert!(p.len() <= budget);
        let has_heading = p.contains("Recent outputs");
        assert_eq!(has_heading, p.contains("⟦Ψ:RESUME"), "budget {budget}: {p}");
        if let Some(line) = p.lines().find(|l| l.starts_with("Failing now:")) {
            // Whole subjects only.
            assert!(
                line.ends_with("fail-a") || line.ends_with("fail-b") || line.ends_with("fail-c"),
                "{line}"
            );
        }
    }
    // One subject fits where all three do not.
    let one = mmu.resume_packet(AgentKind::ClaudeCode, header + 30);
    assert!(one.contains("Failing now: Bash:fail-a"), "{one}");
}

#[test]
fn subject_delimiters_are_escaped_and_a_cut_never_splits_an_escape() {
    let mut mmu = InfinityContextMmu::default();
    let spoof = format!("{}\u{27e7}\u{27eb}]]>>\u{2028}end", "x".repeat(77));
    mmu.ingest_payload(AgentKind::ClaudeCode, &spoof, "body\n", false);
    let p = mmu.resume_packet(AgentKind::ClaudeCode, 3_072);
    let line = p.lines().find(|l| l.starts_with("⟦Ψ:RESUME")).unwrap();
    // Exactly one frame open and close, the packet's own: a frame bracket in the subject is
    // escaped to ASCII, also right at the cut.
    assert_eq!(line.matches('\u{27e6}').count(), 1, "{line}");
    assert_eq!(line.matches('\u{27e7}').count(), 1, "{line}");
    assert!(line.ends_with('\u{27e7}'), "{line}");
    assert_eq!(p.matches('\u{27eb}').count(), 1, "{p}");
    assert!(!p.contains('\u{2028}'), "{p}");
    assert_eq!(handles(&p).len(), 1, "{p}");
}

#[test]
fn the_packet_is_deterministic_lists_newest_first_and_names_failures() {
    let mut mmu = filled(5);
    mmu.ingest_payload_with_exit_code(
        AgentKind::ClaudeCode,
        "Bash:cargo clippy",
        "error: unused variable\n",
        Some(1),
        false,
    );
    let a = mmu.resume_packet(AgentKind::ClaudeCode, 3_072);
    assert_eq!(a, mmu.resume_packet(AgentKind::ClaudeCode, 3_072));
    assert!(a.starts_with("⟪ICX:RESUME│agent=claude-code"), "{a}");
    let hs = handles(&a);
    assert_eq!(hs.len(), 6, "{a}");
    assert!(
        a.lines()
            .find(|l| l.starts_with("⟦Ψ:RESUME"))
            .unwrap()
            .contains("cargo clippy"),
        "newest first: {a}"
    );
    // Failures come before the outputs, so a long list cannot crowd them out.
    assert_eq!(
        a.lines().nth(1),
        Some("Failing now: Bash:cargo clippy"),
        "{a}"
    );
    // Delimiters in a subject are escaped, never emitted raw.
    assert!(!a.contains("⟪tricky⟫"), "{a}");
    // A tiny budget keeps only the (cut) header.
    assert!(mmu.resume_packet(AgentKind::ClaudeCode, 10).len() <= 10);
}

fn context(reply: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(reply).unwrap();
    v["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap_or("")
        .to_owned()
}

#[test]
fn claude_code_gets_one_packet_after_compaction() {
    let mut mmu = filled(3);
    let start = ClaudeCodeAdapter::handle_hook("SessionStart", "{}", &mut mmu).unwrap();
    assert!(!context(&start).contains("ICX:RESUME"), "no compaction yet");
    ClaudeCodeAdapter::handle_hook("PreCompact", "{}", &mut mmu).unwrap();
    let after = ClaudeCodeAdapter::handle_hook("SessionStart", r#"{"source":"compact"}"#, &mut mmu)
        .unwrap();
    assert!(context(&after).contains("ICX:RESUME"), "{after}");
    let next =
        ClaudeCodeAdapter::handle_hook("UserPromptSubmit", r#"{"prompt":"continue"}"#, &mut mmu)
            .unwrap();
    assert!(!context(&next).contains("ICX:RESUME"), "delivered once");
    // Without a SessionStart after compaction, the next prompt carries it.
    ClaudeCodeAdapter::handle_hook("PostCompact", "{}", &mut mmu).unwrap();
    let prompt =
        ClaudeCodeAdapter::handle_hook("UserPromptSubmit", r#"{"prompt":"continue"}"#, &mut mmu)
            .unwrap();
    assert!(context(&prompt).contains("ICX:RESUME"), "{prompt}");
}

#[test]
fn codex_gets_one_packet_at_its_next_hook_with_context() {
    let mut mmu = filled(3);
    CodexAdapter::handle_hook("PreCompact", "{}", &mut mmu).unwrap();
    let first =
        CodexAdapter::handle_hook("UserPromptSubmit", r#"{"prompt":"go on"}"#, &mut mmu).unwrap();
    assert!(context(&first).contains("ICX:RESUME"), "{first}");
    let second =
        CodexAdapter::handle_hook("UserPromptSubmit", r#"{"prompt":"go on"}"#, &mut mmu).unwrap();
    assert!(!context(&second).contains("ICX:RESUME"));
    // Agents are independent: a Codex compaction leaves Claude Code alone.
    CodexAdapter::handle_hook("PostCompact", "{}", &mut mmu).unwrap();
    let claude =
        ClaudeCodeAdapter::handle_hook("UserPromptSubmit", r#"{"prompt":"x"}"#, &mut mmu).unwrap();
    assert!(!context(&claude).contains("ICX:RESUME"));
}

#[test]
fn a_pending_packet_survives_a_save_and_the_switch_turns_it_off() {
    let dir = std::env::temp_dir().join(format!("icx-resume-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut mmu = filled(2);
    ClaudeCodeAdapter::handle_hook("PreCompact", "{}", &mut mmu).unwrap();
    mmu.save_workspace_dir(&dir).unwrap();
    let mut next = InfinityContextMmu::default();
    next.set_resume_packets(true);
    next.load_workspace_dir(&dir).unwrap();
    assert!(
        next.take_resume_packet(AgentKind::ClaudeCode, 3_072)
            .is_some()
    );
    assert!(
        next.take_resume_packet(AgentKind::ClaudeCode, 3_072)
            .is_none()
    );

    let mut off = filled(2);
    off.set_resume_packets(false);
    ClaudeCodeAdapter::handle_hook("PreCompact", "{}", &mut off).unwrap();
    assert!(
        off.take_resume_packet(AgentKind::ClaudeCode, 3_072)
            .is_none()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hermes_gets_the_packet_in_the_session_start_after_compression() {
    use icx_adapters::HermesAdapter;
    let mut mmu = filled(3);
    let msgs =
        r#"{"messages":[{"role":"user","content":"hi"},{"role":"assistant","content":"ok"}]}"#;
    HermesAdapter::handle_hook("on_pre_compress", msgs, &mut mmu).unwrap();
    let reply: serde_json::Value =
        serde_json::from_str(&HermesAdapter::handle_hook("SessionStart", "{}", &mut mmu).unwrap())
            .unwrap();
    let tail = reply["systemMessage"].as_str().unwrap();
    assert!(tail.contains("ICX:RESUME"), "{tail}");
    assert!(tail.chars().count() <= icx_adapters::hermes::HERMES_MAX_TAIL_CHARS);
    let again: serde_json::Value =
        serde_json::from_str(&HermesAdapter::handle_hook("SessionStart", "{}", &mut mmu).unwrap())
            .unwrap();
    assert!(
        !again["systemMessage"]
            .as_str()
            .unwrap()
            .contains("ICX:RESUME")
    );
}

#[test]
fn the_hermes_reply_limit_matches_the_plugin() {
    let plugin = include_str!("../../../integrations/hermes/icx/__init__.py");
    let line = format!(
        "_MAX_TAIL_CHARS = {}",
        icx_adapters::hermes::HERMES_MAX_TAIL_CHARS
    );
    assert!(
        plugin.contains(&line),
        "plugin and adapter disagree on {line}"
    );
}
