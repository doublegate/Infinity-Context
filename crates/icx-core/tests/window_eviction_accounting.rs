//! v1.2.1 review follow-ups for saturated-window eviction and prefix byte accounting.
//!
//! * `total_cached_prefix_bytes` must equal the rendered prefix length even when the pinned
//!   head is empty (separator accounting).
//! * Step eviction must never evict more sealed epochs than `max(low-water target, smallest
//!   eviction that fits)`, and never empty the retained history when keeping some fits, even
//!   when the low-water target is unreachable (one large epoch, or a large active window).

use icx_core::dag::MerkleContextDag;
use icx_core::epoch::{
    AssembledWindow, CacheEpochController, EpochControllerConfig, HISTORY_OMITTED_MARKER_PREFIX,
    PromptWindowBudget,
};
use icx_core::{AgentKind, ArtifactId};

const HEAD: &str = "<<ICX:PINNED_HEAD│system=infinity-context-v1>>";

fn record(ctrl: &mut CacheEpochController, dag: &mut MerkleContextDag, i: u32, pad: usize) {
    let (raw_id, _) = ArtifactId::hash_bytes(&i.to_le_bytes());
    let ahtl = format!(
        "[[Δ:src/mod_{i}.rs#L1-50⊢⊕fn:step_{i}{}↦{raw_id}]]",
        "x".repeat(pad)
    );
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

fn cost(frame: &str) -> usize {
    frame.len() + 1
}

/// Assembles `ctrl` (eviction re-planned from zero) under every window size in `sizes` and
/// checks the eviction-minimality invariants against the real rendered frames.
fn sweep(mut ctrl: CacheEpochController, dag: &MerkleContextDag, sizes: std::ops::Range<usize>) {
    let frames: Vec<String> = ctrl
        .sealed_epochs()
        .iter()
        .map(|n| n.ahtl_representation().to_owned())
        .collect();
    let n = frames.len();
    assert!(n >= 2, "fixture needs several sealed epochs");
    let mut minimal_cases = 0usize;
    for w in sizes {
        ctrl.set_window_budget(PromptWindowBudget::new(w + 2, 1, 1).unwrap())
            .unwrap();
        let win = ctrl.assemble_window(dag, String::new());
        let cap = ctrl.window_budget().window_bytes();
        assert!(win.deterministic_prefix().len() + win.ephemeral_tail_slot.len() <= cap);
        let Some(m) = marker(&win) else {
            continue;
        };
        if !m.contains("│turns=0│") {
            continue; // Degenerate: the active window alone overflows.
        }
        let retained = win.sealed_epochs.len() - 1;
        let evicted = n - retained;
        assert!(m.contains(&format!("epochs={evicted}│")), "{m}");
        let active: usize = win.active_window.iter().map(|a| cost(a)).sum();
        let allowance = cap - HEAD.len() - ctrl.tail_slot_byte_limit();
        let used = cost(m) + frames[evicted..].iter().map(|f| cost(f)).sum::<usize>() + active;
        if used > allowance {
            // Nothing fits the planning allowance (which reserves the maximum tail); the
            // planner falls back to full eviction and assembly fits the actual tail.
            assert_eq!(retained, 0, "w={w}: kept epochs although nothing fits");
            continue;
        }
        // Upper bound on evicting one fewer epoch: that marker is never longer than this one
        // (fewer epochs, fewer bytes, handles of equal length), and vanishes at zero.
        let prev_marker = if evicted == 1 { 0 } else { cost(m) };
        let one_fewer =
            prev_marker + frames[evicted - 1..].iter().map(|f| cost(f)).sum::<usize>() + active;
        if one_fewer <= allowance {
            // Evicting one fewer would already fit, so this must be a low-water step that
            // keeps some history.
            minimal_cases += 1;
            assert!(
                used <= allowance / 2,
                "w={w}: evicted {evicted}/{n} epochs past the smallest fitting eviction \
                 without reaching the low-water mark ({used} > {})",
                allowance / 2
            );
            assert!(
                retained > 0,
                "w={w}: emptied the retained history although keeping some epochs fits"
            );
        }
    }
    assert!(minimal_cases > 0, "sweep never hit a fits-with-fewer case");
}

#[test]
fn empty_pinned_head_prefix_byte_count_matches_rendered_prefix() {
    let mut dag = MerkleContextDag::new();
    let mut ctrl = CacheEpochController::new(
        "",
        EpochControllerConfig {
            epoch_rollover_tokens: 50,
            protect_last_n_turns: 2,
            tail_slot_max_tokens: 64,
        },
    );
    ctrl.set_window_budget(PromptWindowBudget::new(2000, 100, 100).unwrap())
        .unwrap();
    let win = ctrl.assemble_window(&dag, String::new());
    assert_eq!(
        win.total_cached_prefix_bytes,
        win.deterministic_prefix().len()
    );
    for i in 0..120 {
        record(&mut ctrl, &mut dag, i, 0);
        let win = ctrl.assemble_window(&dag, format!("[[Θ:tail_{i}]]"));
        assert_eq!(
            win.total_cached_prefix_bytes,
            win.deterministic_prefix().len(),
            "turn {i}"
        );
        assert!(
            win.deterministic_prefix().len() + win.ephemeral_tail_slot.len()
                <= ctrl.window_budget().window_bytes()
        );
    }
}

#[test]
fn single_large_epoch_eviction_keeps_the_small_epochs() {
    // E0 is one large epoch; E1, E2 are small. Evicting E0 alone fits most budgets, so the
    // small epochs must not be dropped just because the low-water mark is unreachable.
    let mut dag = MerkleContextDag::new();
    let mut ctrl = CacheEpochController::new(
        HEAD,
        EpochControllerConfig {
            epoch_rollover_tokens: usize::MAX,
            protect_last_n_turns: 0,
            tail_slot_max_tokens: 64,
        },
    );
    for i in 0..30 {
        record(&mut ctrl, &mut dag, i, 0);
    }
    ctrl.force_seal_active_epoch(&mut dag);
    for e in 0..2 {
        record(&mut ctrl, &mut dag, 100 + e, 0);
        ctrl.force_seal_active_epoch(&mut dag);
    }
    for i in 200..203 {
        record(&mut ctrl, &mut dag, i, 40);
    }
    assert_eq!(ctrl.sealed_epoch_count(), 3);
    sweep(ctrl, &dag, 200..6000);
}

#[test]
fn large_active_window_eviction_keeps_some_history() {
    // Many protected active turns put the low-water mark out of reach; eviction must stop at
    // the smallest fitting step rather than drop every sealed epoch.
    let mut dag = MerkleContextDag::new();
    let mut ctrl = CacheEpochController::new(
        HEAD,
        EpochControllerConfig {
            epoch_rollover_tokens: 50,
            protect_last_n_turns: 12,
            tail_slot_max_tokens: 64,
        },
    );
    for i in 0..60 {
        record(&mut ctrl, &mut dag, i, 30);
    }
    sweep(ctrl, &dag, 200..12_000);
}

#[test]
fn incremental_run_with_large_active_window_never_empties_history_needlessly() {
    let mut dag = MerkleContextDag::new();
    let mut ctrl = CacheEpochController::new(
        HEAD,
        EpochControllerConfig {
            epoch_rollover_tokens: 50,
            protect_last_n_turns: 12,
            tail_slot_max_tokens: 64,
        },
    );
    ctrl.set_window_budget(PromptWindowBudget::new(4000, 100, 100).unwrap())
        .unwrap();
    let cap = ctrl.window_budget().window_bytes();
    let mut saw_omission = false;
    for i in 0..200 {
        record(&mut ctrl, &mut dag, i, 30);
        let win = ctrl.assemble_window(&dag, format!("[[Θ:tail_{i}]]"));
        assert!(win.deterministic_prefix().len() + win.ephemeral_tail_slot.len() <= cap);
        let n = ctrl.sealed_epoch_count();
        if let Some(m) = marker(&win) {
            saw_omission = true;
            assert!(m.contains("│turns=0│"), "turn {i}: {m}");
            let newest = ctrl.sealed_epochs()[n - 1].ahtl_representation();
            let active: usize = win.active_window.iter().map(|a| cost(a)).sum();
            let allowance = cap - HEAD.len() - ctrl.tail_slot_byte_limit();
            if cost(m) + cost(newest) + active <= allowance {
                assert!(
                    win.sealed_epochs.len() > 1,
                    "turn {i}: all {n} epochs evicted although the newest fits"
                );
            }
        }
    }
    assert!(saw_omission, "fixture must saturate the window");
}
