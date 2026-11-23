//! Compaction and session end force an epoch seal in every adapter (owner decision,
//! 2026-11-22): compaction rewrites history, so the old cached prefix is gone whatever the
//! epoch's size. A repeated event with nothing new adds no empty epoch.

use icx_adapters::{
    AntigravityAdapter, ClaudeCodeAdapter, CodexAdapter, CursorAdapter, GeminiCliAdapter,
    InfinityContextMmu, MuseAdapter, OpenCodeAdapter,
};
use icx_core::AgentKind;

type Handler = fn(&str, &str, &mut InfinityContextMmu) -> Result<String, serde_json::Error>;

fn cases() -> Vec<(AgentKind, Handler, Vec<&'static str>)> {
    vec![
        (
            AgentKind::ClaudeCode,
            ClaudeCodeAdapter::handle_hook,
            vec!["PreCompact", "PostCompact", "SessionEnd"],
        ),
        (
            AgentKind::Codex,
            CodexAdapter::handle_hook,
            vec!["PreCompact", "PostCompact", "SessionEnd"],
        ),
        (
            AgentKind::Muse,
            MuseAdapter::handle_hook,
            vec!["PreCompact", "PostCompact", "SessionEnd"],
        ),
        (
            AgentKind::GeminiCli,
            GeminiCliAdapter::handle_hook,
            vec!["PreCompress", "SessionEnd"],
        ),
        (
            AgentKind::Cursor,
            CursorAdapter::handle_hook,
            vec!["preCompact", "sessionEnd"],
        ),
        (
            AgentKind::OpenCode,
            OpenCodeAdapter::handle_hook,
            vec!["compaction", "session.end"],
        ),
        (
            AgentKind::AntigravityCli,
            AntigravityAdapter::handle_hook,
            vec!["SessionEnd"],
        ),
    ]
}

#[test]
fn every_compaction_and_session_end_seals_a_small_epoch() {
    for (agent, handle, events) in cases() {
        for event in events {
            let mut mmu = InfinityContextMmu::default();
            mmu.ingest_payload(agent, "work", "a small amount of work\n", false);
            let before = mmu.epochs.sealed_epoch_count();
            handle(event, "{}", &mut mmu).unwrap();
            assert_eq!(
                mmu.epochs.sealed_epoch_count(),
                before + 1,
                "{agent:?} {event} seals"
            );
            // Nothing new since: a second event adds no empty epoch.
            handle(event, "{}", &mut mmu).unwrap();
            assert_eq!(
                mmu.epochs.sealed_epoch_count(),
                before + 1,
                "{agent:?} {event} twice"
            );
        }
    }
}
