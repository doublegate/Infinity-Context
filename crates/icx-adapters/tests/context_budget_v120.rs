use icx_adapters::InfinityContextMmu;
use icx_core::AgentKind;
use icx_core::epoch::PromptWindowBudget;

#[test]
fn production_prompt_assembly_is_bounded_without_test_only_retargeting() {
    let mut mmu = InfinityContextMmu::default();
    let first = mmu
        .ingest_payload(AgentKind::Codex, "log", "event0 ok\n", false)
        .artifact_id;
    for i in 1..500 {
        mmu.ingest_payload(AgentKind::Codex, "log", &format!("event{i} ok\n"), false);
    }
    let window = mmu.assemble_prompt_window(AgentKind::Codex, "event0");
    let bytes = window.deterministic_prefix().len() + window.ephemeral_tail_slot.len();
    // Default32k context,4k external input/tool allowance and4k completion reserve.
    assert!(bytes <= 24_576, "unbounded rendered window: {bytes}");
    assert_eq!(mmu.deref_text(first).unwrap(), "event0 ok\n");
}

#[test]
fn direct_hook_and_proxy_tail_path_obeys_the_same_budget() {
    let mut mmu = InfinityContextMmu::default();
    mmu.ingest_payload(
        AgentKind::Codex,
        &"publicname".repeat(2000),
        "small output",
        false,
    );
    let tail = mmu.build_ephemeral_tail_slot(AgentKind::Codex, "small output", 3);
    assert!(tail.len() <= 1024, "unbounded direct tail: {}", tail.len());
    assert!(tail.ends_with('⟫') || tail.ends_with('⟧'));
}

#[test]
fn smaller_model_policy_survives_restart_and_keeps_cold_history() {
    let root = std::env::temp_dir()
        .join(format!(
            "icx-v120-tests-{}",
            std::env::var("USER").unwrap_or_default()
        ))
        .join("budget-tests");
    std::fs::create_dir_all(&root).unwrap();
    let dir = root.join(format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut mmu = InfinityContextMmu::default();
    mmu.epochs
        .set_window_budget(PromptWindowBudget::new(2048, 512, 512).unwrap())
        .unwrap();
    let receipt = mmu.ingest_payload(AgentKind::Codex, "unicode", "你好 κόσμε привет\n", false);
    mmu.save_workspace_dir(&dir).unwrap();
    let mut restored = InfinityContextMmu::default();
    restored.load_workspace_dir(&dir).unwrap();
    assert_eq!(restored.epochs.window_budget().window_bytes(), 1024);
    let window = restored.assemble_prompt_window(AgentKind::Codex, "unicode");
    assert!(window.deterministic_prefix().len() + window.ephemeral_tail_slot.len() <= 1024);
    assert_eq!(
        restored.deref_text(receipt.artifact_id).unwrap(),
        "你好 κόσμε привет\n"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "release qualification: actual ingestion beyond ten thousand turns"]
fn ten_thousand_turns_remain_bounded_without_test_retargeting() {
    let mut mmu = InfinityContextMmu::default();
    let first = mmu
        .ingest_payload(AgentKind::Codex, "log", "event0 ok\n", false)
        .artifact_id;
    for i in 1..10_002 {
        mmu.ingest_payload(AgentKind::Codex, "log", &format!("event{i} ok\n"), false);
        if i % 100 == 0 {
            let window = mmu.assemble_prompt_window(AgentKind::Codex, "log");
            assert!(
                window.deterministic_prefix().len() + window.ephemeral_tail_slot.len() <= 24_576
            );
        }
    }
    assert_eq!(mmu.deref_text(first).unwrap(), "event0 ok\n");
}
