use icx_core::dag::MerkleContextDag;
use icx_core::epoch::{CacheEpochController, EpochControllerConfig, PromptWindowBudget};
use icx_core::{AgentKind, ArtifactId};

#[test]
fn invalid_or_insufficient_reservations_are_refused() {
    for (context, completion, external) in [(0, 0, 0), (10, 5, 5), (usize::MAX, usize::MAX, 1)] {
        assert!(PromptWindowBudget::new(context, completion, external).is_err());
    }
    let mut controller = CacheEpochController::new("pinned head", EpochControllerConfig::default());
    assert!(
        controller
            .set_window_budget(PromptWindowBudget::new(3, 1, 1).unwrap())
            .is_err()
    );
}

#[test]
fn oversized_constructor_head_emits_an_actionable_bounded_diagnostic() {
    // The public constructor preserves its infallible API. Assembly must explain why it
    // cannot use an oversized head, rather than silently returning an empty prompt.
    let controller =
        CacheEpochController::new("head".repeat(10_000), EpochControllerConfig::default());
    let window = controller.assemble_window(&MerkleContextDag::new(), "recall".into());
    assert_eq!(window.pinned_head, "ICX_CONTEXT_BUDGET_EXCEEDED");
    assert!(window.deterministic_prefix().len() <= controller.window_budget().window_bytes());
    assert_eq!(window.ephemeral_tail_slot, "");
    assert!(window.active_window.is_empty() && window.sealed_epochs.is_empty());
}

#[test]
fn fitting_recent_history_is_retained_and_oversized_frames_are_omitted_whole() {
    let mut controller = CacheEpochController::new(
        "head",
        EpochControllerConfig {
            epoch_rollover_tokens: 1,
            protect_last_n_turns: 2,
            tail_slot_max_tokens: 128,
        },
    );
    controller
        .set_window_budget(PromptWindowBudget::new(1024, 128, 128).unwrap())
        .unwrap();
    let mut dag = MerkleContextDag::new();
    for (i, frame) in [
        "sealed-first".to_owned(),
        "sealed-second".to_owned(),
        "oversized".repeat(2000),
        "active-new".to_owned(),
    ]
    .into_iter()
    .enumerate()
    {
        controller.record_turn(
            &mut dag,
            AgentKind::Codex,
            "user",
            ArtifactId(i as u64 + 1),
            frame,
            0.9,
            10,
        );
    }
    let window = controller.assemble_window(&dag, String::new());
    assert_eq!(window.active_window, ["active-new"]);
    assert_eq!(window.sealed_epochs.len(), 2);
    assert!(window.sealed_epochs[0].contains("sealed-first"));
    assert!(!window.deterministic_prefix().contains("oversized"));
    assert_eq!(
        window.total_cached_prefix_bytes,
        window.deterministic_prefix().len()
    );
    assert_eq!(dag.node_count(), 6); // Four source turns and both epochs remain cold-recoverable.
}

#[test]
fn oversized_tail_is_omitted_whole_and_legacy_policy_defaults_are_bounded() {
    let mut controller = CacheEpochController::new("head", EpochControllerConfig::default());
    controller
        .set_window_budget(PromptWindowBudget::new(128, 16, 16).unwrap())
        .unwrap();
    let window = controller.assemble_window(&MerkleContextDag::new(), "⟦large-packet⟧".repeat(100));
    assert!(window.ephemeral_tail_slot.contains("TAIL_OMITTED"));
    assert!(window.deterministic_prefix().len() + window.ephemeral_tail_slot.len() <= 96);
    let mut legacy = serde_json::to_value(&controller).unwrap();
    legacy.as_object_mut().unwrap().remove("window_budget");
    let restored: CacheEpochController = serde_json::from_value(legacy).unwrap();
    assert_eq!(restored.window_budget().window_bytes(), 24_576);
}
