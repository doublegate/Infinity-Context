//! Useful whole-frame tails and delivery-aware bus cursors under small prompt budgets.
use icx_adapters::{ICX_AHTL_LEGEND, InfinityContextMmu};
use icx_core::epoch::PromptWindowBudget;
use icx_core::{AgentKind, ArtifactId};

fn tail_allowance(mmu: &mut InfinityContextMmu, bytes: usize) {
    mmu.epochs
        .set_window_budget(PromptWindowBudget::new(ICX_AHTL_LEGEND.len() + bytes, 0, 0).unwrap())
        .unwrap();
}

#[test]
fn bounded_tail_keeps_header_and_best_recall_instead_of_omitting_everything() {
    let mut mmu = InfinityContextMmu::default();
    let target = mmu
        .ingest_payload(AgentKind::Codex, "needle", "unique_needle_x120\n", false)
        .artifact_id;
    for i in 0..6 {
        mmu.ingest_payload(
            AgentKind::ClaudeCode,
            &format!("other_{i}"),
            &format!("other data {i}\n"),
            false,
        );
    }
    tail_allowance(&mut mmu, 300);
    let tail = mmu.build_ephemeral_tail_slot(AgentKind::Codex, "unique_needle_x120", 5);
    assert!(tail.starts_with("⟪ICX:TAIL_SLOT"), "{tail}");
    assert!(
        tail.contains(&target.to_string()),
        "most relevant handle lost: {tail}"
    );
    assert!(tail.len() <= 300);
}

#[test]
fn unsent_bus_frame_stays_queued_until_budget_allows_delivery() {
    let mut mmu = InfinityContextMmu::default();
    let frame = format!("⟦Θ:claude-code@queued⊢{}⟧", "你好 ".repeat(80));
    mmu.bus.publish(
        AgentKind::ClaudeCode,
        ArtifactId(17),
        mmu.workspace_hash,
        0,
        [0; 4],
        [0; 16],
        &frame,
        1,
    );
    mmu.bus.publish(
        AgentKind::Codex,
        ArtifactId(18),
        mmu.workspace_hash,
        0,
        [0; 4],
        [0; 16],
        "own event",
        2,
    );
    let later = "⟦Θ:claude-code@later⊢later event⟧";
    mmu.bus.publish(
        AgentKind::ClaudeCode,
        ArtifactId(19),
        mmu.workspace_hash,
        0,
        [0; 4],
        [0; 16],
        later,
        3,
    );
    tail_allowance(&mut mmu, 130);
    let first = mmu.build_ephemeral_tail_slot(AgentKind::Codex, "", 0);
    assert!(!first.contains(&frame));
    assert!(first.len() <= 130);
    tail_allowance(&mut mmu, 1024);
    let delivered = mmu.build_ephemeral_tail_slot(AgentKind::Codex, "", 0);
    assert!(
        delivered.contains(&frame),
        "unsent event was consumed: {delivered}"
    );
    assert!(delivered.contains(later));
    assert!(delivered.len() <= 1024);
    let next = mmu.build_ephemeral_tail_slot(AgentKind::Codex, "", 0);
    assert!(!next.contains(&frame), "delivered event repeated");
    assert!(!next.contains(later), "later delivered event repeated");
}

#[test]
fn oversized_stored_event_uses_a_complete_handle_without_starving_following_event() {
    let mut mmu = InfinityContextMmu::default();
    let stored = mmu
        .ingest_payload(
            AgentKind::ClaudeCode,
            &"large_label".repeat(1000),
            "stored event\n",
            false,
        )
        .artifact_id;
    let next = "⟦Θ:claude-code@next⊢small next event⟧";
    mmu.bus.publish(
        AgentKind::ClaudeCode,
        ArtifactId(18),
        mmu.workspace_hash,
        0,
        [0; 4],
        [0; 16],
        next,
        2,
    );
    let tail = mmu.build_ephemeral_tail_slot(AgentKind::Codex, "stored event", 1);
    assert!(tail.starts_with("⟪ICX:TAIL_SLOT"), "{tail}");
    assert!(tail.contains(&stored.to_string()), "{tail}");
    assert!(
        tail.contains(&format!("⟦Θ:claude-code@paged⊢seq=1↦{stored}⟧")),
        "{tail}"
    );
    assert!(tail.contains(next), "following event starved: {tail}");
    assert!(tail.len() <= 1024);
    assert_eq!(mmu.deref_text(stored).unwrap(), "stored event\n");
}

#[test]
fn paging_a_stored_bus_event_keeps_its_short_agent_and_subject_header() {
    let mut mmu = InfinityContextMmu::default();
    let body = "stored tool output\n".repeat(150);
    let stored = mmu
        .ingest_payload(AgentKind::Codex, "tool", &body, false)
        .artifact_id;
    let frame = format!("⟦Θ:claude-code@Bash:cargo test --workspace⊢{body}⟧");
    mmu.bus.publish(
        AgentKind::ClaudeCode,
        stored,
        mmu.workspace_hash,
        0,
        [0; 4],
        [0; 16],
        &frame,
        2,
    );
    let tail = mmu.build_ephemeral_tail_slot(AgentKind::Codex, "", 0);
    assert!(
        tail.contains(&format!(
            "⟦Θ:claude-code@Bash:cargo test --workspace⊢paged↦{stored}⟧"
        )),
        "{tail}"
    );
    assert!(tail.len() <= 1024);
}

#[test]
fn punctuated_query_retains_graph_recall_when_space_is_available() {
    let mut mmu = InfinityContextMmu::default();
    let aid = mmu
        .ingest_payload(AgentKind::Codex, "graph fixture", "ordinary body\n", false)
        .artifact_id;
    mmu.graph
        .upsert_edge("needle", "calls", "downstream", false, 100, aid, 1.0);
    let tail = mmu.build_ephemeral_tail_slot(AgentKind::Codex, "`needle`?", 3);
    assert!(tail.contains("⟦Ω:PPR_GRAPH⊢entity=downstream"), "{tail}");
    assert!(tail.len() <= 1024);
}
