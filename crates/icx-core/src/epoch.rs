//! Prefix-Cache-Locked Virtual Window Controller (`CacheEpochController`, `arXiv:2610.28577`).
//!
//! Guarantees that Zone 0 (Pinned Head) and Zone 1 (Sealed Cache Epochs `E_0..E_{k-1}`) remain
//! 100% byte-identical across consecutive turns while dynamically updating Zone 3 (Ephemeral Tail Slot).

use crate::dag::{DagNode, MerkleContextDag};
use crate::{AgentKind, ArtifactId, IcxError};
use serde::{Deserialize, Serialize};

/// Configuration for the `CacheEpochController`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpochControllerConfig {
    /// Token threshold in the active unsealed window that triggers sealing a new Cache Epoch.
    pub epoch_rollover_tokens: usize,
    /// Number of recent turns protected verbatim at the tail of the active window during rollover.
    pub protect_last_n_turns: usize,
    /// Maximum token budget allocated to the Ephemeral Tail Slot.
    pub tail_slot_max_tokens: usize,
}

impl Default for EpochControllerConfig {
    fn default() -> Self {
        Self {
            epoch_rollover_tokens: 4096,
            protect_last_n_turns: 4,
            tail_slot_max_tokens: 1024,
        }
    }
}

impl EpochControllerConfig {
    /// Normative preset for Anthropic 4-breakpoint explicit `cache_control` (`16,384` rollover tokens,
    /// `6` protected recent turns, `2,048` tail slot budget — `docs/virtual-memory-and-dag-engine.md` §4).
    pub const fn anthropic_default() -> Self {
        Self {
            epoch_rollover_tokens: 16_384,
            protect_last_n_turns: 6,
            tail_slot_max_tokens: 2_048,
        }
    }

    /// Normative preset for OpenAI automatic 1024-token block prefix caching (`8,192` rollover tokens,
    /// `6` protected recent turns, `2,048` tail slot budget).
    pub const fn openai_default() -> Self {
        Self {
            epoch_rollover_tokens: 8_192,
            protect_last_n_turns: 6,
            tail_slot_max_tokens: 2_048,
        }
    }
}

/// Assembled 4-zone prompt view ready for wire transmission or hook injection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssembledWindow {
    pub pinned_head: String,
    pub sealed_epochs: Vec<String>,
    pub active_window: Vec<String>,
    pub ephemeral_tail_slot: String,
    pub total_cached_prefix_bytes: usize,
    pub total_uncached_tail_bytes: usize,
}

impl AssembledWindow {
    /// Fraction of the prompt prefix that is byte-identical to the prior turn's cached prefix.
    pub fn prefix_cache_hit_ratio(&self, previous_prefix: &str) -> f64 {
        let current_prefix = self.deterministic_prefix();
        if previous_prefix.is_empty() || current_prefix.is_empty() {
            return 1.0;
        }
        let common_bytes = current_prefix
            .bytes()
            .zip(previous_prefix.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        let total = current_prefix.len().max(1);
        (common_bytes as f64) / (total as f64)
    }

    /// Concatenates Zone 0 (Pinned Head) + Zone 1 (Sealed Cache Epochs) + Zone 2 (Active Window).
    pub fn deterministic_prefix(&self) -> String {
        let mut out = self.pinned_head.clone();
        for ep in &self.sealed_epochs {
            out.push('\n');
            out.push_str(ep);
        }
        for act in &self.active_window {
            out.push('\n');
            out.push_str(act);
        }
        out
    }
}

/// Manages the 4-zone Prefix-Cache-Locked Virtual Context Window.
#[derive(Clone, Debug)]
pub struct CacheEpochController {
    config: EpochControllerConfig,
    pinned_head: String,
    sealed_epochs: Vec<DagNode>,
    unsealed_turn_ids: Vec<ArtifactId>,
    next_epoch_index: u32,
}

impl CacheEpochController {
    /// Creates a new `CacheEpochController` with the given Pinned Head and configuration.
    pub fn new(pinned_head: impl Into<String>, config: EpochControllerConfig) -> Self {
        Self {
            config,
            pinned_head: pinned_head.into(),
            sealed_epochs: Vec::new(),
            unsealed_turn_ids: Vec::new(),
            next_epoch_index: 0,
        }
    }

    /// Number of immutable sealed Cache Epochs in Zone 1.
    pub fn sealed_epoch_count(&self) -> usize {
        self.sealed_epochs.len()
    }

    /// Returns the slice of sealed Cache Epoch nodes.
    pub fn sealed_epochs(&self) -> &[DagNode] {
        &self.sealed_epochs
    }

    /// Total compressed token estimate across all sealed epochs in Zone 1.
    pub fn zone1_compressed_tokens(&self) -> usize {
        self.sealed_epochs
            .iter()
            .map(DagNode::token_estimate)
            .fold(0usize, usize::saturating_add)
    }

    /// Opens a sub-trajectory branch on `dag` (`Context-Folding`, `arXiv:2510.11967`).
    pub fn begin_branch(&mut self, dag: &mut MerkleContextDag, label: impl Into<String>) {
        dag.begin_branch(label);
    }

    /// Folds the active sub-trajectory branch on `dag` into an `L2` `SubTrajectoryBranch` commit node,
    /// replacing its constituent child turns in the active unsealed window (`Zone 2`) with the single
    /// `L2` branch commit node.
    pub fn fold_branch(
        &mut self,
        dag: &mut MerkleContextDag,
        outcome_summary: &str,
    ) -> Result<ArtifactId, IcxError> {
        let branch_id = dag.fold_branch(outcome_summary)?;
        self.record_node(dag, branch_id);
        Ok(branch_id)
    }

    /// Records an existing `L1` (`Turn`, `ToolArtifact`) or `L2` (`SubTrajectoryBranch`) node into
    /// the active window and triggers an amortized Epoch Rollover if thresholds are exceeded.
    pub fn record_node(&mut self, dag: &mut MerkleContextDag, node_id: ArtifactId) {
        if matches!(
            dag.get_node(node_id),
            Some(DagNode::SubTrajectoryBranch { .. })
        ) {
            let mut to_remove = Vec::new();
            dag.collect_all_descendant_ids(node_id, &mut to_remove);
            if let Some(first_pos) = self
                .unsealed_turn_ids
                .iter()
                .position(|id| to_remove.contains(id))
            {
                self.unsealed_turn_ids.retain(|id| !to_remove.contains(id));
                let insert_pos = first_pos.min(self.unsealed_turn_ids.len());
                self.unsealed_turn_ids.insert(insert_pos, node_id);
            } else if !self.unsealed_turn_ids.contains(&node_id) {
                self.unsealed_turn_ids.push(node_id);
            }
        } else if !self.unsealed_turn_ids.contains(&node_id) {
            self.unsealed_turn_ids.push(node_id);
        }
        self.maybe_rollover(dag);
    }

    /// Records a new turn into the active window and triggers an amortized Epoch Rollover if
    /// the unsealed token count exceeds `epoch_rollover_tokens`.
    pub fn record_turn(
        &mut self,
        dag: &mut MerkleContextDag,
        agent: AgentKind,
        role: &str,
        raw_artifact: ArtifactId,
        ahtl_repr: String,
        surprise: f32,
        token_estimate: usize,
    ) -> ArtifactId {
        let turn_id = dag.append_turn(
            agent,
            role,
            raw_artifact,
            ahtl_repr,
            surprise,
            token_estimate,
        );
        self.record_node(dag, turn_id);
        turn_id
    }

    /// Forces sealing the eligible unsealed window into a new `CacheEpochNode` if thresholds are met.
    /// Does not seal mid-branch while a sub-trajectory branch is open on `dag`.
    pub fn maybe_rollover(&mut self, dag: &mut MerkleContextDag) -> Option<ArtifactId> {
        if dag.active_branch_depth() > 0 {
            return None;
        }
        let unsealed_tokens: usize = self
            .unsealed_turn_ids
            .iter()
            .filter_map(|&id| dag.get_node(id).map(DagNode::token_estimate))
            .fold(0usize, usize::saturating_add);

        if unsealed_tokens < self.config.epoch_rollover_tokens
            || self.unsealed_turn_ids.len() <= self.config.protect_last_n_turns
        {
            return None;
        }

        let seal_count = self.unsealed_turn_ids.len() - self.config.protect_last_n_turns;
        let to_seal: Vec<ArtifactId> = self.unsealed_turn_ids.drain(0..seal_count).collect();
        let raw_tokens: usize = to_seal
            .iter()
            .filter_map(|&id| dag.get_node(id).map(DagNode::raw_token_count))
            .fold(0usize, usize::saturating_add);

        let epoch_idx = self.next_epoch_index;
        self.next_epoch_index = self.next_epoch_index.saturating_add(1);
        let epoch_node = dag.seal_epoch(epoch_idx, to_seal, raw_tokens);
        let id = epoch_node.id();
        self.sealed_epochs.push(epoch_node);
        Some(id)
    }

    /// Hierarchically folds existing `L3` epochs `(E_0, E_1) -> E_{0..1}^{(L4)}` in level-order pairs
    /// when switching mid-session to a model with a smaller Zone 1 token budget (`docs/virtual-memory-and-dag-engine.md` §5).
    /// Returns the number of pairwise epoch-fold operations performed.
    pub fn retarget_budget(
        &mut self,
        dag: &mut MerkleContextDag,
        max_zone1_tokens: usize,
    ) -> usize {
        let mut folds = 0usize;
        let mut pair_idx = 0usize;
        let mut unproductive_checks = 0usize;

        while self.zone1_compressed_tokens() > max_zone1_tokens && self.sealed_epochs.len() >= 2 {
            let max_pairs = self.sealed_epochs.len() - 1;
            if unproductive_checks >= max_pairs {
                break;
            }
            if pair_idx + 1 >= self.sealed_epochs.len() {
                pair_idx = 0;
            }
            let before_pair_tokens = self.sealed_epochs[pair_idx]
                .token_estimate()
                .saturating_add(self.sealed_epochs[pair_idx + 1].token_estimate());
            let id0 = self.sealed_epochs[pair_idx].id();
            let id1 = self.sealed_epochs[pair_idx + 1].id();

            let Ok(folded) = dag.build_folded_epoch(&[id0, id1]) else {
                pair_idx += 1;
                unproductive_checks += 1;
                continue;
            };
            if folded.token_estimate() >= before_pair_tokens {
                pair_idx += 1;
                unproductive_checks += 1;
                continue;
            }

            dag.insert_node(folded.clone());
            self.sealed_epochs.splice(pair_idx..=pair_idx + 1, [folded]);
            folds += 1;
            pair_idx += 1;
            unproductive_checks = 0;
        }

        folds
    }

    /// Assembles the 4-zone prompt view with the provided dynamic Ephemeral Tail Slot recall.
    pub fn assemble_window(
        &self,
        dag: &MerkleContextDag,
        ephemeral_tail_slot: String,
    ) -> AssembledWindow {
        let sealed_epochs: Vec<String> = self
            .sealed_epochs
            .iter()
            .filter_map(|n| match n {
                DagNode::CacheEpoch { ahtl_frame, .. } => Some(ahtl_frame.clone()),
                _ => None,
            })
            .collect();

        let active_window: Vec<String> = self
            .unsealed_turn_ids
            .iter()
            .filter_map(|&id| dag.get_node(id).map(|n| n.ahtl_representation().to_owned()))
            .collect();

        let total_cached_prefix_bytes = self.pinned_head.len()
            + sealed_epochs.iter().map(String::len).sum::<usize>()
            + active_window.iter().map(String::len).sum::<usize>();
        let total_uncached_tail_bytes = ephemeral_tail_slot.len();

        AssembledWindow {
            pinned_head: self.pinned_head.clone(),
            sealed_epochs,
            active_window,
            ephemeral_tail_slot,
            total_cached_prefix_bytes,
            total_uncached_tail_bytes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_epochs_remain_byte_identical_and_sustain_94_percent_cache_hit_rate() {
        let mut dag = MerkleContextDag::new();
        let cfg = EpochControllerConfig {
            epoch_rollover_tokens: 2400,
            protect_last_n_turns: 2,
            tail_slot_max_tokens: 256,
        };
        let mut ctrl = CacheEpochController::new(
            "⟪ICX:PINNED_HEAD│system=infinity-context-v1⟫".repeat(32),
            cfg,
        );

        let mut prev_prefix = String::new();
        let mut prev_epochs: Vec<String> = Vec::new();
        let mut hit_ratios = Vec::new();

        for i in 0..80u32 {
            let (raw_id, _) = ArtifactId::hash_bytes(&i.to_le_bytes());
            let ahtl = format!("⟦Δ:src/mod_{i}.rs#L1-50⊢⊕fn:step_{i}↦{raw_id}⟧");
            ctrl.record_turn(
                &mut dag,
                AgentKind::ClaudeCode,
                "assistant",
                raw_id,
                ahtl,
                0.7,
                300,
            );

            let win = ctrl.assemble_window(&dag, format!("⟦Θ:tail_recall_{i}⟧"));
            // Every previously sealed epoch must remain 100% byte-identical forever
            for (ep_idx, old_ep) in prev_epochs.iter().enumerate() {
                assert_eq!(&win.sealed_epochs[ep_idx], old_ep);
            }
            if !prev_prefix.is_empty() && win.sealed_epochs.len() == prev_epochs.len() {
                let ratio = win.prefix_cache_hit_ratio(&prev_prefix);
                hit_ratios.push(ratio);
            }
            prev_epochs.clone_from(&win.sealed_epochs);
            prev_prefix = win.deterministic_prefix();
        }

        assert!(ctrl.sealed_epoch_count() >= 8);
        let avg_hit: f64 = hit_ratios.iter().sum::<f64>() / (hit_ratios.len() as f64);
        assert!(
            avg_hit >= 0.94,
            "expected >= 94% average prefix cache hit ratio, got {avg_hit:.4}"
        );
    }

    #[test]
    fn prefix_cache_200_turn_simulation_and_retarget_budget_l4_folding() {
        let mut dag = MerkleContextDag::new();
        let anthropic_cfg = EpochControllerConfig::anthropic_default();
        assert_eq!(anthropic_cfg.epoch_rollover_tokens, 16_384);
        assert_eq!(anthropic_cfg.protect_last_n_turns, 6);
        let openai_cfg = EpochControllerConfig::openai_default();
        assert_eq!(openai_cfg.epoch_rollover_tokens, 8_192);

        let sim_cfg = EpochControllerConfig {
            epoch_rollover_tokens: 3000,
            protect_last_n_turns: 4,
            tail_slot_max_tokens: 512,
        };
        let mut ctrl = CacheEpochController::new(
            "⟪ICX:PINNED_HEAD│system=infinity-context-v1│tier=mmu⟫".repeat(36),
            sim_cfg,
        );

        let mut prev_prefix = String::new();
        let mut prev_epochs: Vec<String> = Vec::new();
        let mut hit_ratios = Vec::with_capacity(200);

        for turn in 0..200u32 {
            let (raw_id, _) = ArtifactId::hash_bytes(&turn.to_le_bytes());
            let ahtl =
                format!("⟦Δ:crates/icx-core/src/mod_{turn}.rs#L1-80⊢⊕fn:op_{turn}↦{raw_id}⟧");
            ctrl.record_turn(
                &mut dag,
                AgentKind::ClaudeCode,
                "assistant",
                raw_id,
                ahtl,
                if turn % 5 == 0 { 0.85 } else { 0.25 },
                350,
            );

            let win = ctrl.assemble_window(&dag, format!("⟦Θ:tail_recall_{turn}⟧"));
            for (ep_idx, old_ep) in prev_epochs.iter().enumerate() {
                assert_eq!(&win.sealed_epochs[ep_idx], old_ep);
            }
            if !prev_prefix.is_empty() && win.sealed_epochs.len() == prev_epochs.len() {
                hit_ratios.push(win.prefix_cache_hit_ratio(&prev_prefix));
            }
            prev_epochs.clone_from(&win.sealed_epochs);
            prev_prefix = win.deterministic_prefix();
        }

        assert!(ctrl.sealed_epoch_count() >= 15);
        let avg_200_hit: f64 = hit_ratios.iter().sum::<f64>() / (hit_ratios.len() as f64);
        assert!(
            avg_200_hit >= 0.94,
            "200-turn simulation expected >= 94% cache hit rate, got {avg_200_hit:.4}"
        );

        // Verify model-switch retarget_budget folds L3 epochs into L4 epochs while preserving Merkle integrity
        let initial_zone1_tokens = ctrl.zone1_compressed_tokens();
        let initial_epochs = ctrl.sealed_epoch_count();
        let target_budget = initial_zone1_tokens / 2;
        let folds = ctrl.retarget_budget(&mut dag, target_budget);
        assert!(folds >= 1);
        assert!(ctrl.sealed_epoch_count() < initial_epochs);
        assert!(ctrl.zone1_compressed_tokens() <= target_budget);
        for ep in ctrl.sealed_epochs() {
            assert!(dag.verify_merkle_integrity(ep.id()).unwrap());
        }
    }

    #[test]
    fn branch_folding_synchronizes_active_window_and_retarget_skips_non_shrinking_pairs() {
        let mut dag = MerkleContextDag::new();
        let cfg = EpochControllerConfig {
            epoch_rollover_tokens: 100,
            protect_last_n_turns: 0,
            tail_slot_max_tokens: 128,
        };
        let mut ctrl = CacheEpochController::new("⟪ICX:PINNED_HEAD⟫", cfg);

        // Two tiny 1-turn epochs whose L4 folded summary would not be smaller than their L3 frames
        for i in 0..2u32 {
            let (raw_id, _) = ArtifactId::hash_bytes(&i.to_le_bytes());
            ctrl.record_turn(
                &mut dag,
                AgentKind::ClaudeCode,
                "u",
                raw_id,
                "⟦Δ:x⟧".into(),
                0.1,
                120,
            );
        }
        assert_eq!(ctrl.sealed_epoch_count(), 2);

        // Followed by two large multi-turn epochs that DO shrink significantly when folded
        ctrl.begin_branch(&mut dag, "multi_step_branch");
        for i in 10..20u32 {
            let (raw_id, _) = ArtifactId::hash_bytes(&i.to_le_bytes());
            ctrl.record_turn(
                &mut dag,
                AgentKind::ClaudeCode,
                "assistant",
                raw_id,
                format!(
                    "⟦Δ:crates/icx-core/src/long_module_{i}.rs#L1-200⊢⊕fn:heavy_work_{i}↦{raw_id}⟧"
                ),
                0.9,
                50,
            );
        }
        // While branch is open, maybe_rollover defers sealing
        assert_eq!(ctrl.sealed_epoch_count(), 2);
        // Folding branch replaces the 10 child turns in active_window with 1 L2 commit node
        // and triggers rollover
        let branch_id = ctrl.fold_branch(&mut dag, "resolved_10_steps").unwrap();
        assert!(dag.verify_merkle_integrity(branch_id).unwrap());

        for i in 20..32u32 {
            let (raw_id, _) = ArtifactId::hash_bytes(&i.to_le_bytes());
            ctrl.record_turn(
                &mut dag,
                AgentKind::ClaudeCode,
                "assistant",
                raw_id,
                format!(
                    "⟦Δ:crates/icx-core/src/extra_module_{i}.rs#L1-200⊢⊕fn:extra_{i}↦{raw_id}⟧"
                ),
                0.9,
                25,
            );
        }
        assert!(ctrl.sealed_epoch_count() >= 4);

        let before_tokens = ctrl.zone1_compressed_tokens();
        let folds = ctrl.retarget_budget(&mut dag, before_tokens - 10);
        assert!(
            folds >= 1,
            "retarget_budget must skip non-shrinking head pair and fold later pairs"
        );
        assert!(ctrl.zone1_compressed_tokens() < before_tokens);
    }
}
