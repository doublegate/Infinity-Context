//! T-62-006: every Codex `additionalContext` stays within the default `additionalContextLimit`
//! (2,500 tokens, estimated by Codex as UTF-8 bytes / 4), so Codex never spills it to a file, and
//! no `#icx:` handle is ever cut.

use icx_adapters::{CODEX_INLINE_CONTEXT_MAX_BYTES, CodexAdapter, InfinityContextMmu};
use icx_bus::SeqlockTelemetryBus;
use icx_core::AgentKind;
use icx_core::epoch::{EpochControllerConfig, PromptWindowBudget};

const EVENTS: [(&str, &str); 3] = [
    ("SessionStart", r#"{"session_id":"s1"}"#),
    ("UserPromptSubmit", r#"{"prompt":"needle recall query"}"#),
    ("SubagentStart", r#"{"agent_id":"sub-1"}"#),
];

fn contexts(mmu: &mut InfinityContextMmu) -> Vec<(String, String)> {
    EVENTS
        .iter()
        .map(|(event, payload)| {
            let out = CodexAdapter::handle_hook(event, payload, mmu).unwrap();
            let v: serde_json::Value = serde_json::from_str(&out).unwrap();
            let ctx = v["hookSpecificOutput"]["additionalContext"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            ((*event).to_owned(), ctx)
        })
        .collect()
}

/// Every `#icx:` occurrence is followed by a complete 12- or 16-hex handle.
fn handles_are_whole(ctx: &str) -> bool {
    ctx.match_indices("#icx:").all(|(i, _)| {
        let hex: String = ctx[i + 5..]
            .chars()
            .take_while(char::is_ascii_hexdigit)
            .collect();
        hex.len() == 12 || hex.len() == 16
    })
}

fn populated(
    config: EpochControllerConfig,
    payloads: usize,
    subject_len: usize,
) -> InfinityContextMmu {
    let mut mmu = InfinityContextMmu::new(config, SeqlockTelemetryBus::new_in_memory());
    mmu.epochs
        .set_window_budget(PromptWindowBudget::new(1 << 20, 0, 0).unwrap())
        .unwrap();
    for i in 0..payloads {
        let subject = format!("needle_{i}_{}", "s".repeat(subject_len));
        let body = format!("needle recall query result {i}\n{}\n", "line ".repeat(200));
        mmu.ingest_payload(AgentKind::ClaudeCode, &subject, &body, false);
        mmu.ingest_payload(AgentKind::Codex, &subject, &body, false);
    }
    mmu
}

#[test]
fn codex_context_fits_the_inline_limit_with_the_largest_presets() {
    for config in [
        EpochControllerConfig::default(),
        EpochControllerConfig::anthropic_default(),
        EpochControllerConfig::openai_default(),
    ] {
        for (payloads, subject_len) in [(1, 8), (12, 64), (40, 400)] {
            let mut mmu = populated(config, payloads, subject_len);
            for (event, ctx) in contexts(&mut mmu) {
                assert!(
                    ctx.len() <= CODEX_INLINE_CONTEXT_MAX_BYTES,
                    "{event}: {} bytes exceeds the Codex inline limit",
                    ctx.len()
                );
                assert!(handles_are_whole(&ctx), "{event}: a handle was cut: {ctx}");
                assert!(!ctx.is_empty(), "{event}: context is present");
            }
        }
    }
}

#[test]
fn an_oversized_tail_is_replaced_whole_by_a_diagnostic() {
    // A tail allowance far above any preset: the reply guard, not the tail, must bound it.
    let config = EpochControllerConfig {
        tail_slot_max_tokens: 200_000,
        ..EpochControllerConfig::default()
    };
    let mut mmu = populated(config, 60, 2_000);
    for (event, ctx) in contexts(&mut mmu) {
        assert!(
            ctx.len() <= CODEX_INLINE_CONTEXT_MAX_BYTES,
            "{event}: {} bytes",
            ctx.len()
        );
        assert!(handles_are_whole(&ctx), "{event}: a handle was cut");
    }
}
