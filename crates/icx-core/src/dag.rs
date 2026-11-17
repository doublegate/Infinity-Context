//! Lossless Context Management (`LCM`) BLAKE3 Merkle-DAG (`arXiv:2605.04050`) and
//! Sub-Trajectory Context-Folding (`arXiv:2510.11967`).

use crate::{AgentKind, ArtifactId, Blake3Digest, IcxError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A node in the Hierarchical Lossless Context Management Merkle-DAG.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DagNode {
    /// `L1`: A single conversational turn or tool-execution artifact.
    Turn {
        id: ArtifactId,
        digest: Blake3Digest,
        ordinal: u64,
        agent: AgentKind,
        role: String,
        raw_artifact: ArtifactId,
        ahtl_repr: String,
        surprise: f32,
        token_estimate: usize,
    },
    /// `L2`: A folded sub-trajectory branch (`Context-Folding`, `arXiv:2510.11967`).
    SubTrajectoryBranch {
        id: ArtifactId,
        digest: Blake3Digest,
        label: String,
        child_turns: Vec<ArtifactId>,
        folded_ahtl_commit: String,
        raw_tokens_saved: usize,
    },
    /// `L3`: A cryptographically sealed, prefix-cache-aligned Cache Epoch (`arXiv:2610.28577`).
    CacheEpoch {
        epoch_index: u32,
        id: ArtifactId,
        merkle_root: Blake3Digest,
        child_nodes: Vec<ArtifactId>,
        raw_tokens: usize,
        compressed_tokens: usize,
        ahtl_frame: String,
    },
}

impl DagNode {
    /// Returns the `ArtifactId` of this DAG node.
    pub const fn id(&self) -> ArtifactId {
        match self {
            Self::Turn { id, .. }
            | Self::SubTrajectoryBranch { id, .. }
            | Self::CacheEpoch { id, .. } => *id,
        }
    }

    /// Returns the 256-bit cryptographic BLAKE3 digest of this node.
    pub const fn digest(&self) -> Blake3Digest {
        match self {
            Self::Turn { digest, .. } | Self::SubTrajectoryBranch { digest, .. } => *digest,
            Self::CacheEpoch { merkle_root, .. } => *merkle_root,
        }
    }
}

/// Active sub-trajectory branch state before folding.
#[derive(Clone, Debug)]
struct OpenBranch {
    label: String,
    turn_ids: Vec<ArtifactId>,
}

/// Hierarchical Lossless Context Management (`LCM`) Merkle-DAG.
#[derive(Clone, Debug, Default)]
pub struct MerkleContextDag {
    nodes: BTreeMap<ArtifactId, DagNode>,
    trunk_order: Vec<ArtifactId>,
    active_branch: Option<OpenBranch>,
    next_ordinal: u64,
}

impl MerkleContextDag {
    /// Creates an empty `MerkleContextDag`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Total number of nodes across all DAG levels.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Returns the ordered list of trunk node IDs (`L1` turns or `L2` folded branches).
    pub fn trunk_nodes(&self) -> &[ArtifactId] {
        &self.trunk_order
    }

    /// Looks up a `DagNode` by its `ArtifactId`.
    pub fn get_node(&self, id: ArtifactId) -> Option<&DagNode> {
        self.nodes.get(&id)
    }

    /// Appends a new `L1` turn/tool node to the active branch (if open) or main trunk.
    pub fn append_turn(
        &mut self,
        agent: AgentKind,
        role: &str,
        raw_artifact: ArtifactId,
        ahtl_repr: String,
        surprise: f32,
        token_estimate: usize,
    ) -> ArtifactId {
        let ordinal = self.next_ordinal;
        self.next_ordinal += 1;

        let mut hasher = blake3::Hasher::new();
        hasher.update(b"ICX_DAG_TURN_V1");
        hasher.update(&ordinal.to_be_bytes());
        hasher.update(&[agent as u8]);
        hasher.update(role.as_bytes());
        hasher.update(&raw_artifact.0.to_be_bytes());
        hasher.update(ahtl_repr.as_bytes());
        let digest = Blake3Digest(*hasher.finalize().as_bytes());
        let id = ArtifactId::from_digest(&digest);

        let node = DagNode::Turn {
            id,
            digest,
            ordinal,
            agent,
            role: role.to_owned(),
            raw_artifact,
            ahtl_repr,
            surprise,
            token_estimate,
        };
        self.nodes.insert(id, node);

        if let Some(branch) = &mut self.active_branch {
            branch.turn_ids.push(id);
        } else {
            self.trunk_order.push(id);
        }

        id
    }

    /// Opens a sub-trajectory branch (`Context-Folding`, `arXiv:2510.11967`) for an iterative task loop.
    pub fn begin_branch(&mut self, label: impl Into<String>) {
        self.active_branch = Some(OpenBranch {
            label: label.into(),
            turn_ids: Vec::new(),
        });
    }

    /// Folds the active sub-trajectory branch into a single `L2` commit node on the main trunk,
    /// retaining lossless pointers to every child `L1` turn.
    pub fn fold_branch(&mut self, outcome_summary: &str) -> Result<ArtifactId, IcxError> {
        let branch = self.active_branch.take().ok_or(IcxError::NoActiveBranch)?;

        let mut hasher = blake3::Hasher::new();
        hasher.update(b"ICX_DAG_BRANCH_V1");
        hasher.update(branch.label.as_bytes());
        let mut raw_tokens_saved = 0usize;

        for &tid in &branch.turn_ids {
            hasher.update(&tid.0.to_be_bytes());
            if let Some(DagNode::Turn { token_estimate, .. }) = self.nodes.get(&tid) {
                raw_tokens_saved += *token_estimate;
            }
        }
        hasher.update(outcome_summary.as_bytes());

        let digest = Blake3Digest(*hasher.finalize().as_bytes());
        let id = ArtifactId::from_digest(&digest);

        let folded_ahtl_commit = format!(
            "⟦Φ:{}⊢steps={}⋮✓{}↦{}⟧",
            branch.label,
            branch.turn_ids.len(),
            outcome_summary,
            id
        );

        let node = DagNode::SubTrajectoryBranch {
            id,
            digest,
            label: branch.label,
            child_turns: branch.turn_ids,
            folded_ahtl_commit,
            raw_tokens_saved,
        };

        self.nodes.insert(id, node);
        self.trunk_order.push(id);
        Ok(id)
    }

    /// Computes the cryptographic Merkle root over a slice of child DAG nodes and seals an `L3` `CacheEpoch`.
    pub fn seal_epoch(
        &mut self,
        epoch_index: u32,
        child_nodes: Vec<ArtifactId>,
        raw_tokens: usize,
    ) -> DagNode {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"ICX_DAG_EPOCH_V1");
        hasher.update(&epoch_index.to_be_bytes());

        let mut body_packets = Vec::with_capacity(child_nodes.len());
        for &cid in &child_nodes {
            if let Some(n) = self.nodes.get(&cid) {
                hasher.update(&n.digest().0);
                match n {
                    DagNode::Turn {
                        ahtl_repr,
                        surprise,
                        raw_artifact,
                        role,
                        ..
                    } => {
                        if *surprise >= 0.45 {
                            body_packets.push(ahtl_repr.clone());
                        } else {
                            body_packets.push(format!("⟦Ψ:{role}↦{raw_artifact}⟧"));
                        }
                    }
                    DagNode::SubTrajectoryBranch {
                        folded_ahtl_commit, ..
                    } => {
                        body_packets.push(folded_ahtl_commit.clone());
                    }
                    DagNode::CacheEpoch { ahtl_frame, .. } => {
                        body_packets.push(ahtl_frame.clone());
                    }
                }
            }
        }

        let merkle_root = Blake3Digest(*hasher.finalize().as_bytes());
        let id = ArtifactId::from_digest(&merkle_root);
        let root_short = &merkle_root.to_hex()[..8];
        let ahtl_frame = format!(
            "⟪ICX:v1│ep={epoch_index:04}│root={root_short}│nodes={}⟫\n{}",
            child_nodes.len(),
            body_packets.join("\n")
        );
        let compressed_tokens = (ahtl_frame.len() / 4).max(1);

        let epoch_node = DagNode::CacheEpoch {
            epoch_index,
            id,
            merkle_root,
            child_nodes,
            raw_tokens,
            compressed_tokens,
            ahtl_frame,
        };
        self.nodes.insert(id, epoch_node.clone());
        epoch_node
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_folding_collapses_subtrajectory_and_preserves_child_turns() {
        let mut dag = MerkleContextDag::new();
        dag.begin_branch("debug_allocator_alignment");

        let (raw1, _) = ArtifactId::hash_bytes(b"cargo test fail");
        let (raw2, _) = ArtifactId::hash_bytes(b"cargo test pass");
        let t1 = dag.append_turn(
            AgentKind::ClaudeCode,
            "tool",
            raw1,
            "⟦Ψ:cargo_test⊢!FAIL⟧".into(),
            0.9,
            1200,
        );
        let t2 = dag.append_turn(
            AgentKind::ClaudeCode,
            "tool",
            raw2,
            "⟦Ψ:cargo_test⊢✓PASS⟧".into(),
            0.8,
            1100,
        );

        let branch_id = dag
            .fold_branch("align_64_fixed")
            .expect("fold_branch succeeds");
        assert_eq!(dag.trunk_nodes(), &[branch_id]);

        if let Some(DagNode::SubTrajectoryBranch {
            child_turns,
            raw_tokens_saved,
            folded_ahtl_commit,
            ..
        }) = dag.get_node(branch_id)
        {
            assert_eq!(child_turns, &vec![t1, t2]);
            assert_eq!(*raw_tokens_saved, 2300);
            assert!(folded_ahtl_commit.contains("steps=2"));
        } else {
            panic!("expected SubTrajectoryBranch node");
        }
    }
}
