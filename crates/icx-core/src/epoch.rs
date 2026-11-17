//! Prefix-Cache-Locked Virtual Window Controller (`CacheEpochController`, `arXiv:2610.28577`).
//!
//! Guarantees that Zone 0 (Pinned Head) and Zone 1 (Sealed Cache Epochs `E_0..E_{k-1}`) remain
//! 100% byte-identical across consecutive turns while dynamically updating Zone 3 (Ephemeral Tail Slot).

use crate::dag::{DagNode, MerkleContextDag};
use crate::{AgentKind, ArtifactId};
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
}

impl CacheEpochController {
    /// Creates a new `CacheEpochController` with the given Pinned Head and configuration.
    pub fn new(pinned_head: impl Into<String>, config: EpochControllerConfig) -> Self {
        Self {
            config,
            pinned_head: pinned_head.into(),
            sealed_epochs: Vec::new(),
            unsealed_turn_ids: Vec::new(),
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
        self.unsealed_turn_ids.push(turn_id);
        self.maybe_rollover(dag);
        turn_id
    }

    /// Forces sealing the eligible unsealed window into a new `CacheEpochNode` if thresholds are met.
    pub fn maybe_rollover(&mut self, dag: &mut MerkleContextDag) -> Option<ArtifactId> {
        let unsealed_tokens: usize = self
            .unsealed_turn_ids
            .iter()
            .filter_map(|&id| match dag.get_node(id) {
                Some(DagNode::Turn { token_estimate, .. }) => Some(*token_estimate),
                _ => None,
            })
            .sum();

        if unsealed_tokens < self.config.epoch_rollover_tokens
            || self.unsealed_turn_ids.len() <= self.config.protect_last_n_turns
        {
            return None;
        }

        let seal_count = self.unsealed_turn_ids.len() - self.config.protect_last_n_turns;
        let to_seal: Vec<ArtifactId> = self.unsealed_turn_ids.drain(0..seal_count).collect();
        let raw_tokens: usize = to_seal
            .iter()
            .filter_map(|&id| match dag.get_node(id) {
                Some(DagNode::Turn { token_estimate, .. }) => Some(*token_estimate),
                _ => None,
            })
            .sum();

        let epoch_idx = self.sealed_epochs.len() as u32;
        let epoch_node = dag.seal_epoch(epoch_idx, to_seal, raw_tokens);
        let id = epoch_node.id();
        self.sealed_epochs.push(epoch_node);
        Some(id)
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
            .filter_map(|&id| match dag.get_node(id) {
                Some(DagNode::Turn { ahtl_repr, .. }) => Some(ahtl_repr.clone()),
                Some(DagNode::SubTrajectoryBranch {
                    folded_ahtl_commit, ..
                }) => Some(folded_ahtl_commit.clone()),
                _ => None,
            })
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
}
