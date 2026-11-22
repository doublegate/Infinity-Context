//! Saturated-window eviction keeps the provider prefix stable (ICX-CODEX-101 follow-up).
//!
//! Once the bounded window is full, history must be evicted in discrete, hysteretic steps
//! behind one stable omission marker, rather than one epoch per turn (which shifts the whole
//! history portion of the prompt every turn and destroys prefix-cache reuse).

use icx_core::dag::{DagNode, MerkleContextDag};
use icx_core::epoch::{
    AssembledWindow, CacheEpochController, EpochControllerConfig, HISTORY_OMITTED_MARKER_PREFIX,
    PromptWindowBudget,
};
use icx_core::{AgentKind, ArtifactId};

const TURNS: u32 = 240;

fn controller() -> CacheEpochController {
    controller_with(2000, 100)
}

fn controller_with(context: usize, reserve: usize) -> CacheEpochController {
    let mut ctrl = CacheEpochController::new(
        "<<ICX:PINNED_HEAD│system=infinity-context-v1>>",
        EpochControllerConfig {
            epoch_rollover_tokens: 50,
            protect_last_n_turns: 2,
            tail_slot_max_tokens: 64,
        },
    );
    ctrl.set_window_budget(PromptWindowBudget::new(context, reserve, reserve).unwrap())
        .unwrap();
    ctrl
}

fn record(ctrl: &mut CacheEpochController, dag: &mut MerkleContextDag, i: u32) {
    let (raw_id, _) = ArtifactId::hash_bytes(&i.to_le_bytes());
    let ahtl = format!("[[Δ:src/mod_{i}.rs#L1-50⊢⊕fn:step_{i}↦{raw_id}]]");
    ctrl.record_turn(
        dag,
        AgentKind::ClaudeCode,
        "assistant",
        raw_id,
        ahtl,
        0.5,
        20,
    );
}

fn marker(win: &AssembledWindow) -> Option<&str> {
    win.sealed_epochs
        .first()
        .map(String::as_str)
        .filter(|f| f.starts_with(HISTORY_OMITTED_MARKER_PREFIX))
}

fn retained_epochs(win: &AssembledWindow) -> &[String] {
    if marker(win).is_some() {
        &win.sealed_epochs[1..]
    } else {
        &win.sealed_epochs
    }
}

fn epoch_frames(ctrl: &CacheEpochController) -> Vec<String> {
    ctrl.sealed_epochs()
        .iter()
        .map(|n| n.ahtl_representation().to_owned())
        .collect()
}

/// Drives `turns` turns through `ctrl`, asserting the per-turn invariants (a), (c), (d), and
/// returns the byte-weighted prefix reuse between consecutive windows over the last 100 turns.
fn run_saturated(mut ctrl: CacheEpochController, turns: u32) -> f64 {
    let mut dag = MerkleContextDag::new();
    let cap = ctrl.window_budget().window_bytes();

    let mut prev: Option<AssembledWindow> = None;
    let (mut reused, mut total) = (0usize, 0usize);
    let mut eviction_steps = 0usize;
    let mut first_saturated_turn = None;

    for i in 0..turns {
        record(&mut ctrl, &mut dag, i);
        let win = ctrl.assemble_window(&dag, format!("[[Θ:tail_recall_{i}]]"));

        // (a) The budget is never exceeded.
        assert!(
            win.deterministic_prefix().len() + win.ephemeral_tail_slot.len() <= cap,
            "turn {i}: window exceeds {cap} bytes"
        );
        assert_eq!(
            win.total_cached_prefix_bytes,
            win.deterministic_prefix().len()
        );

        // (d) Omitted history is always announced, and retained history is a contiguous
        // newest suffix of the sealed epochs (no silent gap).
        let all = epoch_frames(&ctrl);
        let kept = retained_epochs(&win);
        assert_eq!(
            kept,
            &all[all.len() - kept.len()..],
            "turn {i}: gap in history"
        );
        if kept.len() < all.len() {
            let m = marker(&win).unwrap_or_else(|| panic!("turn {i}: omission marker missing"));
            assert!(
                m.contains(&format!("epochs={}", all.len() - kept.len())),
                "{m}"
            );
            first_saturated_turn.get_or_insert(i);
        } else {
            assert!(marker(&win).is_none(), "turn {i}: marker without omission");
        }

        if let Some(p) = &prev {
            let marker_changed = marker(p) != marker(&win);
            if marker_changed {
                eviction_steps += 1;
            } else {
                // (c) Between eviction steps head + marker + retained epochs never move: the
                // previous turn's Zone 0/1 is a byte prefix of this turn's.
                assert_eq!(p.pinned_head, win.pinned_head);
                assert!(
                    win.sealed_epochs.starts_with(&p.sealed_epochs),
                    "turn {i}: sealed zone shifted without an eviction step"
                );
            }
            if i >= turns - 100 {
                let (r, t) = win.prefix_reuse_bytes(&p.deterministic_prefix());
                reused += r;
                total += t;
            }
        }
        prev = Some(win);
    }

    let saturated_at = first_saturated_turn.expect("fixture must saturate the window");
    assert!(
        saturated_at < turns / 2,
        "saturated only at turn {saturated_at}"
    );
    let saturated_turns = (turns - saturated_at) as usize;
    assert!(
        eviction_steps * 5 <= saturated_turns,
        "{eviction_steps} eviction steps over {saturated_turns} saturated turns is not discrete"
    );
    let reuse = reused as f64 / total as f64;
    eprintln!(
        "steady-state byte prefix reuse = {reuse:.4} ({reused}/{total}); eviction steps = \
         {eviction_steps} over {saturated_turns} saturated turns"
    );
    reuse
}

#[test]
fn saturated_window_evicts_in_discrete_steps_and_keeps_prefix_stable() {
    // 7,168-byte window, saturated from turn 77 onward.
    let reuse = run_saturated(controller_with(8192, 512), 400);
    // (b) Steady-state byte reuse between consecutive turns.
    assert!(reuse >= 0.9, "steady-state prefix reuse {reuse:.4} < 0.9");
}

#[test]
fn review_probe_budget_no_longer_shifts_the_prefix_every_turn() {
    // The review probe: (2000, 100, 100), rollover 50. It measured 0.013 steady-state reuse
    // under one-epoch-per-turn eviction. Here every turn seals an epoch, so ~229 bytes of
    // Zone 2 change per turn whatever the eviction policy; in a 1,800-byte window that alone
    // caps reuse near 0.83, so this asserts the discrete-step invariants and a 0.75 floor.
    let reuse = run_saturated(controller(), 240);
    assert!(reuse >= 0.75, "probe prefix reuse {reuse:.4} < 0.75");
}

#[test]
fn omitted_history_stays_cold_dereferenceable_through_the_marker() {
    let mut ctrl = controller();
    let mut dag = MerkleContextDag::new();
    for i in 0..TURNS {
        record(&mut ctrl, &mut dag, i);
    }
    let win = ctrl.assemble_window(&dag, String::new());
    let m = marker(&win).expect("history must be omitted").to_owned();
    let first = ctrl.sealed_epochs()[0].id().to_string();
    assert!(m.contains(&format!("first={first}")), "{m}");
    let text = dag
        .deref_node_text(ctrl.sealed_epochs()[0].id())
        .expect("evicted epoch remains in cold storage");
    assert!(text.contains("ep=0000"), "{text}");
    let DagNode::CacheEpoch { child_nodes, .. } = &ctrl.sealed_epochs()[0] else {
        panic!("Zone 1 holds cache epochs");
    };
    let turn = dag.deref_node_text(child_nodes[0]).unwrap();
    assert!(turn.contains("step_0"), "{turn}");
}

#[test]
fn serialization_mid_run_preserves_the_eviction_point() {
    let mut ctrl = controller();
    let mut dag = MerkleContextDag::new();
    for i in 0..150 {
        record(&mut ctrl, &mut dag, i);
    }
    let before = ctrl.assemble_window(&dag, "[[Θ:tail]]".into());
    assert!(marker(&before).is_some());

    let json = serde_json::to_string(&ctrl).unwrap();
    let mut restored: CacheEpochController = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.assemble_window(&dag, "[[Θ:tail]]".into()), before);

    // Both copies continue identically after the restart.
    let mut dag2 = dag.clone();
    for i in 150..200 {
        record(&mut ctrl, &mut dag, i);
        record(&mut restored, &mut dag2, i);
        assert_eq!(
            ctrl.assemble_window(&dag, format!("t{i}")),
            restored.assemble_window(&dag2, format!("t{i}"))
        );
    }

    // State written before the eviction point existed still loads and stays bounded.
    let mut legacy = serde_json::to_value(&ctrl).unwrap();
    legacy.as_object_mut().unwrap().remove("evicted_epochs");
    let legacy: CacheEpochController = serde_json::from_value(legacy).unwrap();
    let win = legacy.assemble_window(&dag, "[[Θ:tail]]".into());
    assert!(
        win.deterministic_prefix().len() + win.ephemeral_tail_slot.len()
            <= legacy.window_budget().window_bytes()
    );
    assert!(marker(&win).is_some());
}
