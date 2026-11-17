//! Lossless Context Management (`LCM`) BLAKE3 Merkle-DAG (`arXiv:2605.04050`) and
//! Sub-Trajectory Context-Folding (`arXiv:2510.11967`).

use crate::cas::LineIndexEntry;
use crate::{AgentKind, ArtifactId, Blake3Digest, IcxError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// `L1`: Tool execution artifact metadata, chunk list, line index, and AHTL stub (`docs/data-model.md` §2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolArtifactNode {
    pub id: ArtifactId,
    pub full_digest: Blake3Digest,
    pub agent: AgentKind,
    pub session_id: String,
    pub tool_name: String,
    pub exit_code: Option<i32>,
    pub total_bytes: usize,
    pub total_lines: usize,
    pub chunks: Vec<ArtifactId>,
    pub line_index: Vec<LineIndexEntry>,
    pub ahtl_stub: String,
    pub surprise_score: f32,
    pub created_at_ms: u64,
    pub token_estimate: usize,
}

impl ToolArtifactNode {
    /// Computes the domain-separated `L1` Merkle digest (`b"ICX_DAG_TOOL_V1"`) over this tool artifact node.
    pub fn compute_merkle_digest(&self) -> Blake3Digest {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"ICX_DAG_TOOL_V1");
        hasher.update(&[self.agent as u8]);
        hasher.update(&(self.session_id.len() as u64).to_be_bytes());
        hasher.update(self.session_id.as_bytes());
        hasher.update(&(self.tool_name.len() as u64).to_be_bytes());
        hasher.update(self.tool_name.as_bytes());
        match self.exit_code {
            Some(code) => {
                hasher.update(&[1u8]);
                hasher.update(&code.to_be_bytes());
            }
            None => {
                hasher.update(&[0u8]);
            }
        }
        hasher.update(&(self.total_bytes as u64).to_be_bytes());
        hasher.update(&(self.total_lines as u64).to_be_bytes());
        hasher.update(&(self.chunks.len() as u64).to_be_bytes());
        for &cid in &self.chunks {
            hasher.update(&cid.0.to_be_bytes());
        }
        hasher.update(&(self.ahtl_stub.len() as u64).to_be_bytes());
        hasher.update(self.ahtl_stub.as_bytes());
        hasher.update(&self.created_at_ms.to_be_bytes());
        Blake3Digest(*hasher.finalize().as_bytes())
    }
}

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
    /// `L1`: A structured tool-execution artifact node with chunk & line-index provenance.
    ToolArtifact(ToolArtifactNode),
    /// `L2`: A folded sub-trajectory branch (`Context-Folding`, `arXiv:2510.11967`).
    SubTrajectoryBranch {
        id: ArtifactId,
        digest: Blake3Digest,
        label: String,
        child_turns: Vec<ArtifactId>,
        folded_ahtl_commit: String,
        raw_tokens_saved: usize,
    },
    /// `L3` / `L4`: A cryptographically sealed, prefix-cache-aligned Cache Epoch (`arXiv:2610.28577`).
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
            Self::ToolArtifact(node) => node.id,
        }
    }

    /// Returns the 256-bit cryptographic BLAKE3 digest of this node.
    pub const fn digest(&self) -> Blake3Digest {
        match self {
            Self::Turn { digest, .. } | Self::SubTrajectoryBranch { digest, .. } => *digest,
            Self::ToolArtifact(node) => node.full_digest,
            Self::CacheEpoch { merkle_root, .. } => *merkle_root,
        }
    }

    /// Returns the active token estimate of this node when rendered in the context window.
    pub fn token_estimate(&self) -> usize {
        match self {
            Self::Turn { token_estimate, .. } => *token_estimate,
            Self::ToolArtifact(node) => node.token_estimate,
            Self::SubTrajectoryBranch {
                folded_ahtl_commit, ..
            } => (folded_ahtl_commit.len() / 4).max(1),
            Self::CacheEpoch {
                compressed_tokens, ..
            } => *compressed_tokens,
        }
    }

    /// Returns the rendered AHTL representation of this node for active window or epoch assembly.
    pub fn ahtl_representation(&self) -> &str {
        match self {
            Self::Turn { ahtl_repr, .. } => ahtl_repr,
            Self::ToolArtifact(node) => &node.ahtl_stub,
            Self::SubTrajectoryBranch {
                folded_ahtl_commit, ..
            } => folded_ahtl_commit,
            Self::CacheEpoch { ahtl_frame, .. } => ahtl_frame,
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
    branch_stack: Vec<OpenBranch>,
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

    /// Current nesting depth of open sub-trajectory branches (`0` when on main trunk).
    pub fn active_branch_depth(&self) -> usize {
        self.branch_stack.len()
    }

    /// Returns the ordered list of trunk node IDs (`L1` turns/artifacts or `L2` folded branches).
    pub fn trunk_nodes(&self) -> &[ArtifactId] {
        &self.trunk_order
    }

    /// Looks up a `DagNode` by its `ArtifactId`.
    pub fn get_node(&self, id: ArtifactId) -> Option<&DagNode> {
        self.nodes.get(&id)
    }

    fn record_node_in_active_scope(&mut self, id: ArtifactId) {
        if let Some(branch) = self.branch_stack.last_mut() {
            branch.turn_ids.push(id);
        } else {
            self.trunk_order.push(id);
        }
    }

    /// Appends a new `L1` turn node to the innermost active branch (if open) or main trunk.
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
        self.record_node_in_active_scope(id);
        id
    }

    /// Appends a structured `L1` `ToolArtifactNode` to the innermost active branch or main trunk,
    /// computing its canonical domain-separated `b"ICX_DAG_TOOL_V1"` Merkle digest.
    pub fn append_tool_artifact(&mut self, mut node: ToolArtifactNode) -> ArtifactId {
        let digest = node.compute_merkle_digest();
        let id = ArtifactId::from_digest(&digest);
        node.id = id;
        node.full_digest = digest;
        self.nodes.insert(id, DagNode::ToolArtifact(node));
        self.record_node_in_active_scope(id);
        id
    }

    /// Opens a sub-trajectory branch (`Context-Folding`, `arXiv:2510.11967`) for an iterative task loop.
    /// Supports nested branches via an internal stack.
    pub fn begin_branch(&mut self, label: impl Into<String>) {
        self.branch_stack.push(OpenBranch {
            label: label.into(),
            turn_ids: Vec::new(),
        });
    }

    /// Folds the innermost active sub-trajectory branch into a single `L2` commit node on the
    /// enclosing branch or main trunk, retaining lossless pointers to every child `L1`/`L2` node.
    pub fn fold_branch(&mut self, outcome_summary: &str) -> Result<ArtifactId, IcxError> {
        let branch = self.branch_stack.pop().ok_or(IcxError::NoActiveBranch)?;

        let mut hasher = blake3::Hasher::new();
        hasher.update(b"ICX_DAG_BRANCH_V1");
        hasher.update(branch.label.as_bytes());
        let mut raw_tokens_saved = 0usize;

        for &tid in &branch.turn_ids {
            hasher.update(&tid.0.to_be_bytes());
            if let Some(child_node) = self.nodes.get(&tid) {
                match child_node {
                    DagNode::Turn { token_estimate, .. } => {
                        raw_tokens_saved = raw_tokens_saved.saturating_add(*token_estimate);
                    }
                    DagNode::ToolArtifact(tool) => {
                        raw_tokens_saved = raw_tokens_saved.saturating_add(tool.token_estimate);
                    }
                    DagNode::SubTrajectoryBranch {
                        raw_tokens_saved: child_saved,
                        ..
                    } => {
                        raw_tokens_saved = raw_tokens_saved.saturating_add(*child_saved);
                    }
                    DagNode::CacheEpoch { raw_tokens, .. } => {
                        raw_tokens_saved = raw_tokens_saved.saturating_add(*raw_tokens);
                    }
                }
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
        self.record_node_in_active_scope(id);
        Ok(id)
    }

    /// Losslessly expands an `L2` `SubTrajectoryBranch` node into its ordered sequence of underlying
    /// `L1` child nodes (`Turn` and `ToolArtifact`), recursively unfolding any nested `L2` branches.
    pub fn expand_branch(&self, branch_id: ArtifactId) -> Result<Vec<&DagNode>, IcxError> {
        let node = self
            .nodes
            .get(&branch_id)
            .ok_or(IcxError::ArtifactNotFound(branch_id))?;
        let child_turns = match node {
            DagNode::SubTrajectoryBranch { child_turns, .. } => child_turns,
            _ => return Err(IcxError::ArtifactNotFound(branch_id)),
        };

        let mut expanded = Vec::with_capacity(child_turns.len());
        for &cid in child_turns {
            let child = self
                .nodes
                .get(&cid)
                .ok_or(IcxError::ArtifactNotFound(cid))?;
            match child {
                DagNode::SubTrajectoryBranch { .. } => {
                    let nested = self.expand_branch(cid)?;
                    expanded.extend(nested);
                }
                _ => expanded.push(child),
            }
        }
        Ok(expanded)
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
                    DagNode::ToolArtifact(tool) => {
                        if tool.surprise_score >= 0.45 {
                            body_packets.push(tool.ahtl_stub.clone());
                        } else {
                            body_packets.push(format!("⟦Ψ:{}↦{}⟧", tool.tool_name, tool.id));
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

    /// Hierarchically folds multiple sealed `L3` `CacheEpoch` nodes into a compact `L4` `CacheEpoch`
    /// node during a mid-session model budget reduction (`docs/virtual-memory-and-dag-engine.md` §5),
    /// retaining every child epoch and leaf in the DAG.
    pub fn fold_epochs(&mut self, epoch_ids: &[ArtifactId]) -> Result<DagNode, IcxError> {
        if epoch_ids.is_empty() {
            return Err(IcxError::ArtifactNotFound(ArtifactId(0)));
        }
        let mut first_epoch_index = 0u32;
        let mut total_raw_tokens = 0usize;
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"ICX_DAG_EPOCH_V1");

        let mut summary_lines = Vec::with_capacity(epoch_ids.len());
        for (i, &eid) in epoch_ids.iter().enumerate() {
            let node = self
                .nodes
                .get(&eid)
                .ok_or(IcxError::ArtifactNotFound(eid))?;
            match node {
                DagNode::CacheEpoch {
                    epoch_index,
                    id,
                    merkle_root,
                    child_nodes,
                    raw_tokens,
                    ..
                } => {
                    if i == 0 {
                        first_epoch_index = *epoch_index;
                        hasher.update(&first_epoch_index.to_be_bytes());
                    }
                    hasher.update(&merkle_root.0);
                    total_raw_tokens = total_raw_tokens.saturating_add(*raw_tokens);
                    let root_prefix = &merkle_root.to_hex()[..8];
                    summary_lines.push(format!(
                        "⟦Φ:epoch_{epoch_index:04}⊢nodes={}⋮root={root_prefix}↦{id}⟧",
                        child_nodes.len()
                    ));
                }
                _ => return Err(IcxError::IntegrityViolation(eid)),
            }
        }

        let merkle_root = Blake3Digest(*hasher.finalize().as_bytes());
        let id = ArtifactId::from_digest(&merkle_root);
        let root_short = &merkle_root.to_hex()[..8];
        let ahtl_frame = format!(
            "⟪ICX:v1│ep={first_epoch_index:04}│root={root_short}│folded={}⟫\n{}",
            epoch_ids.len(),
            summary_lines.join("\n")
        );
        let compressed_tokens = (ahtl_frame.len() / 4).max(1);

        let folded_epoch = DagNode::CacheEpoch {
            epoch_index: first_epoch_index,
            id,
            merkle_root,
            child_nodes: epoch_ids.to_vec(),
            raw_tokens: total_raw_tokens,
            compressed_tokens,
            ahtl_frame,
        };
        self.nodes.insert(id, folded_epoch.clone());
        Ok(folded_epoch)
    }

    /// Cryptographically recomputes and verifies the domain-separated BLAKE3 Merkle digest of `id`
    /// and (recursively) all of its descendant DAG nodes.
    pub fn verify_merkle_integrity(&self, id: ArtifactId) -> Result<bool, IcxError> {
        let node = self.nodes.get(&id).ok_or(IcxError::ArtifactNotFound(id))?;
        match node {
            DagNode::Turn {
                id: node_id,
                digest,
                ordinal,
                agent,
                role,
                raw_artifact,
                ahtl_repr,
                ..
            } => {
                let mut hasher = blake3::Hasher::new();
                hasher.update(b"ICX_DAG_TURN_V1");
                hasher.update(&ordinal.to_be_bytes());
                hasher.update(&[*agent as u8]);
                hasher.update(role.as_bytes());
                hasher.update(&raw_artifact.0.to_be_bytes());
                hasher.update(ahtl_repr.as_bytes());
                let recomputed = Blake3Digest(*hasher.finalize().as_bytes());
                if recomputed != *digest
                    || ArtifactId::from_digest(&recomputed) != *node_id
                    || *node_id != id
                {
                    return Err(IcxError::IntegrityViolation(id));
                }
                Ok(true)
            }
            DagNode::ToolArtifact(tool) => {
                let recomputed = tool.compute_merkle_digest();
                if recomputed != tool.full_digest
                    || ArtifactId::from_digest(&recomputed) != tool.id
                    || tool.id != id
                {
                    return Err(IcxError::IntegrityViolation(id));
                }
                Ok(true)
            }
            DagNode::SubTrajectoryBranch {
                id: node_id,
                digest,
                label,
                child_turns,
                folded_ahtl_commit,
                ..
            } => {
                let expected_prefix = format!("⟦Φ:{}⊢steps={}⋮✓", label, child_turns.len());
                let expected_suffix = format!("↦{node_id}⟧");
                if !folded_ahtl_commit.starts_with(&expected_prefix)
                    || !folded_ahtl_commit.ends_with(&expected_suffix)
                    || folded_ahtl_commit.len() < expected_prefix.len() + expected_suffix.len()
                {
                    return Err(IcxError::IntegrityViolation(id));
                }
                let outcome_summary = &folded_ahtl_commit
                    [expected_prefix.len()..folded_ahtl_commit.len() - expected_suffix.len()];

                let mut hasher = blake3::Hasher::new();
                hasher.update(b"ICX_DAG_BRANCH_V1");
                hasher.update(label.as_bytes());
                for &tid in child_turns {
                    self.verify_merkle_integrity(tid)?;
                    hasher.update(&tid.0.to_be_bytes());
                }
                hasher.update(outcome_summary.as_bytes());
                let recomputed = Blake3Digest(*hasher.finalize().as_bytes());
                if recomputed != *digest
                    || ArtifactId::from_digest(&recomputed) != *node_id
                    || *node_id != id
                {
                    return Err(IcxError::IntegrityViolation(id));
                }
                Ok(true)
            }
            DagNode::CacheEpoch {
                epoch_index,
                id: node_id,
                merkle_root,
                child_nodes,
                ..
            } => {
                let mut hasher = blake3::Hasher::new();
                hasher.update(b"ICX_DAG_EPOCH_V1");
                hasher.update(&epoch_index.to_be_bytes());
                for &cid in child_nodes {
                    self.verify_merkle_integrity(cid)?;
                    let child = self
                        .nodes
                        .get(&cid)
                        .ok_or(IcxError::ArtifactNotFound(cid))?;
                    hasher.update(&child.digest().0);
                }
                let recomputed = Blake3Digest(*hasher.finalize().as_bytes());
                if recomputed != *merkle_root
                    || ArtifactId::from_digest(&recomputed) != *node_id
                    || *node_id != id
                {
                    return Err(IcxError::IntegrityViolation(id));
                }
                Ok(true)
            }
        }
    }

    /// Test-only helper to corrupt a stored node's digest and verify `IntegrityViolation` detection.
    #[cfg(test)]
    pub fn corrupt_node_digest_for_test(&mut self, id: ArtifactId) {
        if let Some(node) = self.nodes.get_mut(&id) {
            match node {
                DagNode::Turn { digest, .. } | DagNode::SubTrajectoryBranch { digest, .. } => {
                    digest.0[0] ^= 0xFF;
                }
                DagNode::ToolArtifact(tool) => {
                    tool.full_digest.0[0] ^= 0xFF;
                }
                DagNode::CacheEpoch { merkle_root, .. } => {
                    merkle_root.0[0] ^= 0xFF;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_folding_collapses_subtrajectory_and_preserves_child_turns() {
        let mut dag = MerkleContextDag::new();
        dag.begin_branch("debug_allocator_alignment");
        assert_eq!(dag.active_branch_depth(), 1);

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
        assert_eq!(dag.active_branch_depth(), 0);
        assert_eq!(dag.trunk_nodes(), &[branch_id]);
        assert!(dag.verify_merkle_integrity(branch_id).unwrap());

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

    #[test]
    fn nested_branches_tool_artifacts_epoch_folding_and_merkle_integrity() {
        let mut dag = MerkleContextDag::new();
        assert!(matches!(
            dag.fold_branch("empty"),
            Err(IcxError::NoActiveBranch)
        ));

        dag.begin_branch("outer_refactor");
        assert_eq!(dag.active_branch_depth(), 1);

        let (chunk_id, chunk_dig) = ArtifactId::hash_bytes(b"tool chunk payload\n");
        let tool_node = ToolArtifactNode {
            id: ArtifactId(0),
            full_digest: chunk_dig,
            agent: AgentKind::Codex,
            session_id: "sess_01".into(),
            tool_name: "cargo_check".into(),
            exit_code: Some(1),
            total_bytes: 19,
            total_lines: 1,
            chunks: vec![chunk_id],
            line_index: vec![LineIndexEntry {
                start_line: 1,
                end_line: 1,
                byte_offset: 0,
                byte_len: 19,
                chunk_id,
            }],
            ahtl_stub: "⟦Ψ:cargo_check⊢!E0502⟧".into(),
            surprise_score: 0.85,
            created_at_ms: 1_794_880_800_000,
            token_estimate: 600,
        };
        let tool_id = dag.append_tool_artifact(tool_node);
        assert!(dag.verify_merkle_integrity(tool_id).unwrap());

        // Open nested inner branch
        dag.begin_branch("inner_borrow_fix");
        assert_eq!(dag.active_branch_depth(), 2);
        let (raw1, _) = ArtifactId::hash_bytes(b"fix step 1");
        let (raw2, _) = ArtifactId::hash_bytes(b"fix step 2");
        let t1 = dag.append_turn(
            AgentKind::Codex,
            "assistant",
            raw1,
            "⟦Δ:src/cas.rs⊢⊕borrow_split⟧".into(),
            0.7,
            400,
        );
        let t2 = dag.append_turn(
            AgentKind::Codex,
            "tool",
            raw2,
            "⟦Ψ:cargo_test⊢✓ok⟧".into(),
            0.3,
            500,
        );
        let inner_branch_id = dag.fold_branch("borrow_resolved").unwrap();
        assert_eq!(dag.active_branch_depth(), 1);

        let outer_branch_id = dag.fold_branch("refactor_complete").unwrap();
        assert_eq!(dag.active_branch_depth(), 0);
        assert_eq!(dag.trunk_nodes(), &[outer_branch_id]);

        // Recursive expand_branch returns [tool_id, t1, t2] in order
        let expanded = dag.expand_branch(outer_branch_id).unwrap();
        let expanded_ids: Vec<ArtifactId> = expanded.iter().map(|n| n.id()).collect();
        assert_eq!(expanded_ids, vec![tool_id, t1, t2]);

        // Seal two L3 epochs and fold them into an L4 epoch
        let ep0 = dag.seal_epoch(0, vec![tool_id, inner_branch_id], 1500);
        let ep1 = dag.seal_epoch(1, vec![outer_branch_id], 1500);
        assert!(dag.verify_merkle_integrity(ep0.id()).unwrap());
        assert!(dag.verify_merkle_integrity(ep1.id()).unwrap());

        let folded_l4 = dag.fold_epochs(&[ep0.id(), ep1.id()]).unwrap();
        assert!(dag.verify_merkle_integrity(folded_l4.id()).unwrap());
        assert!(folded_l4.ahtl_representation().contains("folded=2"));

        // Corrupt a leaf turn digest and verify recursive Merkle check fails closed
        dag.corrupt_node_digest_for_test(t1);
        assert!(matches!(
            dag.verify_merkle_integrity(folded_l4.id()),
            Err(IcxError::IntegrityViolation(_))
        ));
    }
}
