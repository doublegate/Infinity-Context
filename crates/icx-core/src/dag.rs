//! Lossless Context Management (`LCM`) BLAKE3 Merkle-DAG (`arXiv:2605.04050`) and
//! Sub-Trajectory Context-Folding (`arXiv:2510.11967`).

use crate::cas::LineIndexEntry;
use crate::{AgentKind, ArtifactId, Blake3Digest, IcxError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Default Titans surprise threshold (`>= 0.65`) for retaining full `L1` AHTL clauses inside sealed epochs (`docs/virtual-memory-and-dag-engine.md` §4.2).
pub const EPOCH_HIGH_SURPRISE_THRESHOLD: f32 = 0.65;

/// `L1`: Tool execution artifact metadata, chunk list, line index, and AHTL stub (`docs/data-model.md` §2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolArtifactNode {
    /// Unique `L1` DAG node identifier derived from `dag_digest`.
    pub id: ArtifactId,
    /// Domain-separated `b"ICX_DAG_TOOL_V1"` Merkle digest of this `L1` DAG node.
    pub dag_digest: Blake3Digest,
    /// Underlying `L0` `CasStore` artifact handle (`#icx:<hex>`) for lossless byte/line dereferencing.
    pub raw_artifact: ArtifactId,
    /// Full 256-bit `BLAKE3` digest of the raw uncompressed payload in `CasStore`.
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
    /// Computes the domain-separated `L1` Merkle digest (`b"ICX_DAG_TOOL_V1"`) over all fields of this tool artifact node.
    pub fn compute_merkle_digest(&self) -> Blake3Digest {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"ICX_DAG_TOOL_V1");
        hasher.update(&self.raw_artifact.0.to_be_bytes());
        hasher.update(&self.full_digest.0);
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
        hasher.update(&(self.line_index.len() as u64).to_be_bytes());
        for entry in &self.line_index {
            hasher.update(&entry.start_line.to_be_bytes());
            hasher.update(&entry.end_line.to_be_bytes());
            hasher.update(&entry.byte_offset.to_be_bytes());
            hasher.update(&entry.byte_len.to_be_bytes());
            hasher.update(&entry.chunk_id.0.to_be_bytes());
        }
        hasher.update(&(self.ahtl_stub.len() as u64).to_be_bytes());
        hasher.update(self.ahtl_stub.as_bytes());
        let surprise_bits = if !self.surprise_score.is_finite() || self.surprise_score == 0.0 {
            0u32
        } else {
            self.surprise_score.to_bits()
        };
        hasher.update(&surprise_bits.to_be_bytes());
        hasher.update(&self.created_at_ms.to_be_bytes());
        hasher.update(&(self.token_estimate as u64).to_be_bytes());
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
            Self::ToolArtifact(node) => node.dag_digest,
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

    /// Returns the uncompressed/raw token count represented by this node (including tokens saved inside folded branches).
    pub fn raw_token_count(&self) -> usize {
        match self {
            Self::Turn { token_estimate, .. } => *token_estimate,
            Self::ToolArtifact(node) => node.token_estimate,
            Self::SubTrajectoryBranch {
                raw_tokens_saved, ..
            } => (*raw_tokens_saved).max(self.token_estimate()),
            Self::CacheEpoch { raw_tokens, .. } => *raw_tokens,
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
#[derive(Clone, Debug, Serialize, Deserialize)]
struct OpenBranch {
    label: String,
    turn_ids: Vec<ArtifactId>,
}

type TurnIndexEntry = (String, u64, ArtifactId);

/// Hierarchical Lossless Context Management (`LCM`) Merkle-DAG.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MerkleContextDag {
    nodes: BTreeMap<ArtifactId, DagNode>,
    trunk_order: Vec<ArtifactId>,
    branch_stack: Vec<OpenBranch>,
    next_ordinal: u64,
    #[serde(skip)]
    turn_index: BTreeMap<(u8, ArtifactId), Vec<TurnIndexEntry>>,
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

    /// Rebuilds the in-memory `turn_index` from `self.nodes` after deserialization.
    pub fn rebuild_turn_index(&mut self) {
        self.turn_index.clear();
        for (&id, node) in &self.nodes {
            if let DagNode::Turn {
                ordinal,
                agent,
                role,
                raw_artifact,
                ..
            } = node
            {
                self.turn_index
                    .entry((*agent as u8, *raw_artifact))
                    .or_default()
                    .push((role.clone(), *ordinal, id));
            }
        }
        for bucket in self.turn_index.values_mut() {
            bucket.sort_by_key(|(_, ord, _)| *ord);
        }
    }

    /// Finds the most recent `DagNode::Turn` matching `(agent, raw_artifact)` regardless of `role`,
    /// returning `(role, ahtl_repr, surprise)` in $O(1)$ via `turn_index` (falling back to an
    /// $O(N)$ scan over `nodes` if the secondary index was not populated after deserialization).
    pub fn find_turn_any_role(
        &self,
        agent: AgentKind,
        raw_artifact: ArtifactId,
    ) -> Option<(&str, &str, f32)> {
        if let Some(entries) = self.turn_index.get(&(agent as u8, raw_artifact))
            && let Some((_, _, tid)) = entries.iter().max_by_key(|(_, ord, _)| *ord)
            && let Some(DagNode::Turn {
                role,
                ahtl_repr,
                surprise,
                ..
            }) = self.nodes.get(tid)
        {
            return Some((role.as_str(), ahtl_repr.as_str(), *surprise));
        }
        self.nodes
            .values()
            .filter_map(|n| match n {
                DagNode::Turn {
                    ordinal,
                    agent: n_agent,
                    role: n_role,
                    raw_artifact: n_raw,
                    ahtl_repr,
                    surprise,
                    ..
                } if *n_agent == agent && *n_raw == raw_artifact => {
                    Some((*ordinal, n_role.as_str(), ahtl_repr.as_str(), *surprise))
                }
                _ => None,
            })
            .max_by_key(|(ord, _, _, _)| *ord)
            .map(|(_, role, ahtl, s)| (role, ahtl, s))
    }

    /// Finds the most recent `DagNode::Turn` matching `(agent, role, raw_artifact)` in $O(\log N)$
    /// via `turn_index` without heap allocation (falling back to a max-ordinal scan for
    /// deserialized DAGs) and returns `(ahtl_repr, surprise)`.
    pub fn find_turn(
        &self,
        agent: AgentKind,
        role: &str,
        raw_artifact: ArtifactId,
    ) -> Option<(&str, f32)> {
        if let Some(entries) = self.turn_index.get(&(agent as u8, raw_artifact))
            && let Some((_, _, tid)) = entries
                .iter()
                .filter(|(r, _, _)| r == role)
                .max_by_key(|(_, ord, _)| *ord)
            && let Some(DagNode::Turn {
                ahtl_repr,
                surprise,
                ..
            }) = self.nodes.get(tid)
        {
            return Some((ahtl_repr.as_str(), *surprise));
        }
        self.nodes
            .values()
            .filter_map(|n| match n {
                DagNode::Turn {
                    ordinal,
                    agent: n_agent,
                    role: n_role,
                    raw_artifact: n_raw,
                    ahtl_repr,
                    surprise,
                    ..
                } if *n_agent == agent && n_role == role && *n_raw == raw_artifact => {
                    Some((*ordinal, ahtl_repr.as_str(), *surprise))
                }
                _ => None,
            })
            .max_by_key(|(ord, _, _)| *ord)
            .map(|(_, repr, surp)| (repr, surp))
    }

    /// Resolves either a full 64-bit handle (`#icx:<16-hex>`) or a compact 48-bit Profile A prefix
    /// (`#icx:<12-hex>`) against stored DAG nodes (`L1`–`L4`), failing closed with
    /// `IcxError::AmbiguousHandlePrefix` if multiple DAG nodes share the same 48-bit prefix.
    pub fn resolve_handle(&self, handle_or_prefix: &str) -> Result<ArtifactId, IcxError> {
        let (min_id, max_id) = ArtifactId::parse_prefix_range(handle_or_prefix)?;
        if min_id == max_id {
            return if self.nodes.contains_key(&min_id) {
                Ok(min_id)
            } else {
                Err(IcxError::ArtifactNotFound(min_id))
            };
        }
        let mut matches = self.nodes.range(min_id..=max_id);
        let Some((&first, _)) = matches.next() else {
            return Err(IcxError::ArtifactNotFound(min_id));
        };
        if matches.next().is_some() {
            return Err(IcxError::AmbiguousHandlePrefix(handle_or_prefix.to_owned()));
        }
        Ok(first)
    }

    /// Reconstructs and cryptographically verifies the full text representation of a DAG node
    /// (`Turn`, `ToolArtifact`, `SubTrajectoryBranch`, or `L3`/`L4` `CacheEpoch`), expanding
    /// folded child turns or child epoch frames so `#icx:<hex>` references in `L2`/`L4` commits
    /// can be paged back losslessly.
    pub fn deref_node_text(&self, id: ArtifactId) -> Result<String, IcxError> {
        self.verify_merkle_integrity(id)?;
        let mut ancestors = Vec::new();
        self.deref_node_text_with_ancestors(id, &mut ancestors)
    }

    fn deref_node_text_with_ancestors(
        &self,
        id: ArtifactId,
        ancestors: &mut Vec<ArtifactId>,
    ) -> Result<String, IcxError> {
        if ancestors.contains(&id) {
            return Err(IcxError::IntegrityViolation(id));
        }
        let node = self.nodes.get(&id).ok_or(IcxError::ArtifactNotFound(id))?;
        ancestors.push(id);
        let result = match node {
            DagNode::Turn { ahtl_repr, .. } => Ok(ahtl_repr.clone()),
            DagNode::ToolArtifact(tool) => Ok(tool.ahtl_stub.clone()),
            DagNode::SubTrajectoryBranch {
                folded_ahtl_commit,
                child_turns,
                ..
            } => {
                let mut lines = Vec::with_capacity(1 + child_turns.len());
                lines.push(folded_ahtl_commit.clone());
                for &cid in child_turns {
                    lines.push(self.deref_node_text_with_ancestors(cid, ancestors)?);
                }
                Ok(lines.join("\n"))
            }
            DagNode::CacheEpoch {
                ahtl_frame,
                child_nodes,
                ..
            } => {
                let is_l4_folded = ahtl_frame
                    .lines()
                    .next()
                    .is_some_and(|hdr| hdr.contains("│folded="));
                if is_l4_folded {
                    let mut parts = Vec::with_capacity(1 + child_nodes.len());
                    parts.push(ahtl_frame.clone());
                    for &cid in child_nodes {
                        parts.push(self.deref_node_text_with_ancestors(cid, ancestors)?);
                    }
                    Ok(parts.join("\n"))
                } else {
                    Ok(ahtl_frame.clone())
                }
            }
        };
        ancestors.pop();
        result
    }

    /// Reconstructs and slices a 1-based inclusive line range `[start_line, end_line]` from a DAG node.
    pub fn deref_node_lines(
        &self,
        id: ArtifactId,
        start_line: usize,
        end_line: usize,
    ) -> Result<String, IcxError> {
        let full = self.deref_node_text(id)?;
        let lines: Vec<&str> = full.lines().collect();
        let total = lines.len();
        if start_line == 0 || start_line > end_line || end_line > total {
            return Err(IcxError::InvalidLineRange {
                start: start_line,
                end: end_line,
                total,
            });
        }
        Ok(lines[start_line - 1..end_line].join("\n"))
    }

    fn record_node_in_active_scope(&mut self, id: ArtifactId) {
        if let Some(branch) = self.branch_stack.last_mut() {
            branch.turn_ids.push(id);
        } else {
            self.trunk_order.push(id);
        }
    }

    fn compute_turn_digest(
        ordinal: u64,
        agent: AgentKind,
        role: &str,
        raw_artifact: ArtifactId,
        ahtl_repr: &str,
        surprise: f32,
        token_estimate: usize,
    ) -> Blake3Digest {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"ICX_DAG_TURN_V1");
        hasher.update(&ordinal.to_be_bytes());
        hasher.update(&[agent as u8]);
        hasher.update(&(role.len() as u64).to_be_bytes());
        hasher.update(role.as_bytes());
        hasher.update(&raw_artifact.0.to_be_bytes());
        hasher.update(&(ahtl_repr.len() as u64).to_be_bytes());
        hasher.update(ahtl_repr.as_bytes());
        let surprise_bits = if !surprise.is_finite() || surprise == 0.0 {
            0u32
        } else {
            surprise.to_bits()
        };
        hasher.update(&surprise_bits.to_be_bytes());
        hasher.update(&(token_estimate as u64).to_be_bytes());
        Blake3Digest(*hasher.finalize().as_bytes())
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

        let digest = Self::compute_turn_digest(
            ordinal,
            agent,
            role,
            raw_artifact,
            &ahtl_repr,
            surprise,
            token_estimate,
        );
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
        self.turn_index
            .entry((agent as u8, raw_artifact))
            .or_default()
            .push((role.to_owned(), ordinal, id));
        self.nodes.insert(id, node);
        self.record_node_in_active_scope(id);
        id
    }

    /// Appends a structured `L1` `ToolArtifactNode` to the innermost active branch or main trunk,
    /// computing its canonical domain-separated `b"ICX_DAG_TOOL_V1"` Merkle digest while preserving
    /// `raw_artifact` and `full_digest` for lossless `CasStore` dereferencing.
    pub fn append_tool_artifact(&mut self, mut node: ToolArtifactNode) -> ArtifactId {
        if node.raw_artifact == ArtifactId(0) {
            node.raw_artifact = ArtifactId::from_digest(&node.full_digest);
        }
        let dag_digest = node.compute_merkle_digest();
        let id = ArtifactId::from_digest(&dag_digest);
        node.id = id;
        node.dag_digest = dag_digest;
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
        hasher.update(&(branch.label.len() as u64).to_be_bytes());
        hasher.update(branch.label.as_bytes());
        hasher.update(&(branch.turn_ids.len() as u64).to_be_bytes());
        let mut raw_tokens_saved = 0usize;

        for &tid in &branch.turn_ids {
            let child_node = self
                .nodes
                .get(&tid)
                .ok_or(IcxError::ArtifactNotFound(tid))?;
            hasher.update(&tid.0.to_be_bytes());
            hasher.update(&child_node.digest().0);
            raw_tokens_saved = raw_tokens_saved.saturating_add(child_node.raw_token_count());
        }
        hasher.update(&(outcome_summary.len() as u64).to_be_bytes());
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
        let mut ancestors = Vec::new();
        self.expand_branch_with_ancestors(branch_id, &mut ancestors)
    }

    fn expand_branch_with_ancestors(
        &self,
        branch_id: ArtifactId,
        ancestors: &mut Vec<ArtifactId>,
    ) -> Result<Vec<&DagNode>, IcxError> {
        if ancestors.contains(&branch_id) {
            return Err(IcxError::IntegrityViolation(branch_id));
        }
        let node = self
            .nodes
            .get(&branch_id)
            .ok_or(IcxError::ArtifactNotFound(branch_id))?;
        let child_turns = match node {
            DagNode::SubTrajectoryBranch { child_turns, .. } => child_turns,
            _ => return Err(IcxError::IntegrityViolation(branch_id)),
        };

        ancestors.push(branch_id);
        let mut expanded = Vec::with_capacity(child_turns.len());
        for &cid in child_turns {
            let child = self
                .nodes
                .get(&cid)
                .ok_or(IcxError::ArtifactNotFound(cid))?;
            match child {
                DagNode::SubTrajectoryBranch { .. } => {
                    let nested = self.expand_branch_with_ancestors(cid, ancestors)?;
                    expanded.extend(nested);
                }
                _ => expanded.push(child),
            }
        }
        ancestors.pop();
        Ok(expanded)
    }

    /// Recursively collects all descendant node IDs (including intermediate `L2` branch IDs and `L1` leaves)
    /// under `branch_id`.
    pub(crate) fn collect_all_descendant_ids(
        &self,
        branch_id: ArtifactId,
        out: &mut Vec<ArtifactId>,
    ) {
        let mut ancestors = Vec::new();
        self.collect_all_descendant_ids_with_ancestors(branch_id, out, &mut ancestors);
    }

    fn collect_all_descendant_ids_with_ancestors(
        &self,
        branch_id: ArtifactId,
        out: &mut Vec<ArtifactId>,
        ancestors: &mut Vec<ArtifactId>,
    ) {
        if ancestors.contains(&branch_id) {
            return;
        }
        if let Some(DagNode::SubTrajectoryBranch { child_turns, .. }) = self.nodes.get(&branch_id) {
            ancestors.push(branch_id);
            for &cid in child_turns {
                out.push(cid);
                self.collect_all_descendant_ids_with_ancestors(cid, out, ancestors);
            }
            ancestors.pop();
        }
    }

    fn format_l3_epoch_body(&self, child_nodes: &[ArtifactId]) -> Result<Vec<String>, IcxError> {
        let mut body_packets = Vec::with_capacity(child_nodes.len());
        for &cid in child_nodes {
            let n = self
                .nodes
                .get(&cid)
                .ok_or(IcxError::ArtifactNotFound(cid))?;
            match n {
                DagNode::Turn {
                    ahtl_repr,
                    surprise,
                    raw_artifact,
                    role,
                    ..
                } => {
                    if *surprise >= EPOCH_HIGH_SURPRISE_THRESHOLD {
                        body_packets.push(ahtl_repr.clone());
                    } else {
                        body_packets.push(format!("⟦Ψ:{role}↦{raw_artifact}⟧"));
                    }
                }
                DagNode::ToolArtifact(tool) => {
                    if tool.surprise_score >= EPOCH_HIGH_SURPRISE_THRESHOLD {
                        body_packets.push(tool.ahtl_stub.clone());
                    } else {
                        body_packets.push(format!("⟦Ψ:{}↦{}⟧", tool.tool_name, tool.raw_artifact));
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
        Ok(body_packets)
    }

    /// Computes the cryptographic Merkle root over a slice of child DAG nodes and seals an `L3` `CacheEpoch`.
    pub fn seal_epoch(
        &mut self,
        epoch_index: u32,
        child_nodes: Vec<ArtifactId>,
        raw_tokens: usize,
    ) -> DagNode {
        let valid_children: Vec<ArtifactId> = child_nodes
            .into_iter()
            .filter(|cid| self.nodes.contains_key(cid))
            .collect();

        let mut hasher = blake3::Hasher::new();
        hasher.update(b"ICX_DAG_EPOCH_V1");
        hasher.update(&epoch_index.to_be_bytes());
        hasher.update(&(valid_children.len() as u64).to_be_bytes());
        hasher.update(&(raw_tokens as u64).to_be_bytes());

        let body_packets = self
            .format_l3_epoch_body(&valid_children)
            .unwrap_or_default();
        for (&cid, pkt) in valid_children.iter().zip(body_packets.iter()) {
            if let Some(n) = self.nodes.get(&cid) {
                hasher.update(&n.digest().0);
                hasher.update(&(pkt.len() as u64).to_be_bytes());
                hasher.update(pkt.as_bytes());
            }
        }

        let merkle_root = Blake3Digest(*hasher.finalize().as_bytes());
        let id = ArtifactId::from_digest(&merkle_root);
        let root_short = &merkle_root.to_hex()[..8];
        let ahtl_frame = format!(
            "⟪ICX:v1│ep={epoch_index:04}│root={root_short}│nodes={}⟫\n{}",
            valid_children.len(),
            body_packets.join("\n")
        );
        let compressed_tokens = (ahtl_frame.len() / 4).max(1);

        let epoch_node = DagNode::CacheEpoch {
            epoch_index,
            id,
            merkle_root,
            child_nodes: valid_children,
            raw_tokens,
            compressed_tokens,
            ahtl_frame,
        };
        self.nodes.insert(id, epoch_node.clone());
        epoch_node
    }

    /// Constructs a candidate `L4` folded `CacheEpoch` without inserting it into `self.nodes`.
    /// Any input epoch that is already an `L4` folded epoch is flattened into its underlying `L3`
    /// `child_nodes` so the resulting `L4` node always stays at depth 1 above `L3`.
    pub fn build_folded_epoch(&self, epoch_ids: &[ArtifactId]) -> Result<DagNode, IcxError> {
        if epoch_ids.is_empty() {
            return Err(IcxError::IntegrityViolation(ArtifactId(0)));
        }
        let mut flat_l3_ids = Vec::with_capacity(epoch_ids.len());
        for &eid in epoch_ids {
            let node = self
                .nodes
                .get(&eid)
                .ok_or(IcxError::ArtifactNotFound(eid))?;
            match node {
                DagNode::CacheEpoch {
                    child_nodes,
                    ahtl_frame,
                    ..
                } => {
                    let is_l4 = ahtl_frame
                        .lines()
                        .next()
                        .is_some_and(|hdr| hdr.contains("│folded="));
                    if is_l4 {
                        flat_l3_ids.extend_from_slice(child_nodes);
                    } else {
                        flat_l3_ids.push(eid);
                    }
                }
                _ => return Err(IcxError::IntegrityViolation(eid)),
            }
        }

        let mut first_epoch_index = 0u32;
        let mut total_raw_tokens = 0usize;
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"ICX_DAG_EPOCH_FOLD_V1");

        let mut summary_lines = Vec::with_capacity(flat_l3_ids.len());
        for (i, &eid) in flat_l3_ids.iter().enumerate() {
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
                        hasher.update(&(flat_l3_ids.len() as u64).to_be_bytes());
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
            flat_l3_ids.len(),
            summary_lines.join("\n")
        );
        let compressed_tokens = (ahtl_frame.len() / 4).max(1);

        Ok(DagNode::CacheEpoch {
            epoch_index: first_epoch_index,
            id,
            merkle_root,
            child_nodes: flat_l3_ids,
            raw_tokens: total_raw_tokens,
            compressed_tokens,
            ahtl_frame,
        })
    }

    /// Inserts a pre-built `DagNode` into the DAG.
    pub(crate) fn insert_node(&mut self, node: DagNode) -> ArtifactId {
        let id = node.id();
        self.nodes.insert(id, node);
        id
    }

    /// Hierarchically folds multiple sealed `L3` `CacheEpoch` nodes into a compact `L4` `CacheEpoch`
    /// node during a mid-session model budget reduction (`docs/virtual-memory-and-dag-engine.md` §5),
    /// retaining every child epoch and leaf in the DAG.
    pub fn fold_epochs(&mut self, epoch_ids: &[ArtifactId]) -> Result<DagNode, IcxError> {
        let folded_epoch = self.build_folded_epoch(epoch_ids)?;
        self.nodes.insert(folded_epoch.id(), folded_epoch.clone());
        Ok(folded_epoch)
    }

    /// Cryptographically recomputes and verifies the domain-separated BLAKE3 Merkle digest of every
    /// node stored in `self.nodes` in $O(V + E)$ time using a memoized `verified` set, returning the
    /// total number of verified DAG nodes.
    pub fn verify_all_merkle_integrity(&self) -> Result<usize, IcxError> {
        let mut verified = std::collections::BTreeSet::new();
        let mut ancestors = Vec::new();
        for &id in self.nodes.keys() {
            self.verify_merkle_integrity_with_ancestors(id, &mut ancestors, &mut verified)?;
        }
        Ok(self.nodes.len())
    }

    /// Cryptographically recomputes and verifies the domain-separated BLAKE3 Merkle digest of `id`
    /// and (recursively) all of its descendant DAG nodes.
    pub fn verify_merkle_integrity(&self, id: ArtifactId) -> Result<bool, IcxError> {
        let mut ancestors = Vec::new();
        let mut verified = std::collections::BTreeSet::new();
        self.verify_merkle_integrity_with_ancestors(id, &mut ancestors, &mut verified)
    }

    fn verify_merkle_integrity_with_ancestors(
        &self,
        id: ArtifactId,
        ancestors: &mut Vec<ArtifactId>,
        verified: &mut std::collections::BTreeSet<ArtifactId>,
    ) -> Result<bool, IcxError> {
        if ancestors.contains(&id) {
            return Err(IcxError::IntegrityViolation(id));
        }
        if verified.contains(&id) {
            return Ok(true);
        }
        let node = self.nodes.get(&id).ok_or(IcxError::ArtifactNotFound(id))?;
        ancestors.push(id);
        let res = match node {
            DagNode::Turn {
                id: node_id,
                digest,
                ordinal,
                agent,
                role,
                raw_artifact,
                ahtl_repr,
                surprise,
                token_estimate,
            } => {
                let recomputed = Self::compute_turn_digest(
                    *ordinal,
                    *agent,
                    role,
                    *raw_artifact,
                    ahtl_repr,
                    *surprise,
                    *token_estimate,
                );
                if recomputed != *digest
                    || ArtifactId::from_digest(&recomputed) != *node_id
                    || *node_id != id
                {
                    Err(IcxError::IntegrityViolation(id))
                } else {
                    Ok(true)
                }
            }
            DagNode::ToolArtifact(tool) => {
                let recomputed = tool.compute_merkle_digest();
                if recomputed != tool.dag_digest
                    || ArtifactId::from_digest(&recomputed) != tool.id
                    || tool.id != id
                {
                    Err(IcxError::IntegrityViolation(id))
                } else {
                    Ok(true)
                }
            }
            DagNode::SubTrajectoryBranch {
                id: node_id,
                digest,
                label,
                child_turns,
                folded_ahtl_commit,
                raw_tokens_saved,
            } => {
                let expected_prefix = format!("⟦Φ:{}⊢steps={}⋮✓", label, child_turns.len());
                let expected_suffix = format!("↦{node_id}⟧");
                if !folded_ahtl_commit.starts_with(&expected_prefix)
                    || !folded_ahtl_commit.ends_with(&expected_suffix)
                    || folded_ahtl_commit.len() < expected_prefix.len() + expected_suffix.len()
                {
                    ancestors.pop();
                    return Err(IcxError::IntegrityViolation(id));
                }
                let outcome_summary = &folded_ahtl_commit
                    [expected_prefix.len()..folded_ahtl_commit.len() - expected_suffix.len()];

                let mut hasher = blake3::Hasher::new();
                hasher.update(b"ICX_DAG_BRANCH_V1");
                hasher.update(&(label.len() as u64).to_be_bytes());
                hasher.update(label.as_bytes());
                hasher.update(&(child_turns.len() as u64).to_be_bytes());
                let mut expected_saved = 0usize;
                for &tid in child_turns {
                    self.verify_merkle_integrity_with_ancestors(tid, ancestors, verified)?;
                    let child = self
                        .nodes
                        .get(&tid)
                        .ok_or(IcxError::ArtifactNotFound(tid))?;
                    hasher.update(&tid.0.to_be_bytes());
                    hasher.update(&child.digest().0);
                    expected_saved = expected_saved.saturating_add(child.raw_token_count());
                }
                hasher.update(&(outcome_summary.len() as u64).to_be_bytes());
                hasher.update(outcome_summary.as_bytes());
                let recomputed = Blake3Digest(*hasher.finalize().as_bytes());
                if recomputed != *digest
                    || ArtifactId::from_digest(&recomputed) != *node_id
                    || *node_id != id
                    || *raw_tokens_saved != expected_saved
                {
                    Err(IcxError::IntegrityViolation(id))
                } else {
                    Ok(true)
                }
            }
            DagNode::CacheEpoch {
                epoch_index,
                id: node_id,
                merkle_root,
                child_nodes,
                raw_tokens,
                compressed_tokens,
                ahtl_frame,
            } => {
                for &cid in child_nodes {
                    self.verify_merkle_integrity_with_ancestors(cid, ancestors, verified)?;
                }
                let is_l4_folded = ahtl_frame
                    .lines()
                    .next()
                    .is_some_and(|hdr| hdr.contains("│folded="));
                if is_l4_folded {
                    let expected = self.build_folded_epoch(child_nodes)?;
                    if let DagNode::CacheEpoch {
                        epoch_index: exp_idx,
                        id: exp_id,
                        merkle_root: exp_root,
                        raw_tokens: exp_raw,
                        compressed_tokens: exp_comp,
                        ahtl_frame: exp_frame,
                        ..
                    } = expected
                    {
                        if exp_idx != *epoch_index
                            || exp_root != *merkle_root
                            || exp_id != *node_id
                            || *node_id != id
                            || exp_raw != *raw_tokens
                            || exp_comp != *compressed_tokens
                            || exp_frame != *ahtl_frame
                        {
                            ancestors.pop();
                            return Err(IcxError::IntegrityViolation(id));
                        }
                    }
                    Ok(true)
                } else {
                    let mut hasher = blake3::Hasher::new();
                    hasher.update(b"ICX_DAG_EPOCH_V1");
                    hasher.update(&epoch_index.to_be_bytes());
                    hasher.update(&(child_nodes.len() as u64).to_be_bytes());
                    hasher.update(&(*raw_tokens as u64).to_be_bytes());
                    let body_packets = self.format_l3_epoch_body(child_nodes)?;
                    for (&cid, pkt) in child_nodes.iter().zip(body_packets.iter()) {
                        let child = self
                            .nodes
                            .get(&cid)
                            .ok_or(IcxError::ArtifactNotFound(cid))?;
                        hasher.update(&child.digest().0);
                        hasher.update(&(pkt.len() as u64).to_be_bytes());
                        hasher.update(pkt.as_bytes());
                    }
                    let recomputed = Blake3Digest(*hasher.finalize().as_bytes());
                    let root_short = &recomputed.to_hex()[..8];
                    let expected_frame = format!(
                        "⟪ICX:v1│ep={epoch_index:04}│root={root_short}│nodes={}⟫\n{}",
                        child_nodes.len(),
                        body_packets.join("\n")
                    );
                    let expected_comp = (expected_frame.len() / 4).max(1);
                    if recomputed != *merkle_root
                        || ArtifactId::from_digest(&recomputed) != *node_id
                        || *node_id != id
                        || *ahtl_frame != expected_frame
                        || *compressed_tokens != expected_comp
                    {
                        Err(IcxError::IntegrityViolation(id))
                    } else {
                        Ok(true)
                    }
                }
            }
        };
        ancestors.pop();
        if res.is_ok() {
            verified.insert(id);
        }
        res
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
                    tool.dag_digest.0[0] ^= 0xFF;
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
            dag_digest: Blake3Digest([0u8; 32]),
            raw_artifact: chunk_id,
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
            surprise_score: 0.35,
            created_at_ms: 1_794_880_800_000,
            token_estimate: 600,
        };
        let tool_id = dag.append_tool_artifact(tool_node);
        assert!(dag.verify_merkle_integrity(tool_id).unwrap());
        if let Some(DagNode::ToolArtifact(stored_tool)) = dag.get_node(tool_id) {
            assert_eq!(stored_tool.raw_artifact, chunk_id);
            assert_eq!(stored_tool.full_digest, chunk_dig);
        } else {
            panic!("expected ToolArtifact");
        }

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
        // Verify low-surprise ToolArtifact compact stub points to raw_artifact (chunk_id)
        assert!(ep0.ahtl_representation().contains(&chunk_id.to_string()));
        let ep1 = dag.seal_epoch(1, vec![outer_branch_id], 1500);
        assert!(dag.verify_merkle_integrity(ep0.id()).unwrap());
        assert!(dag.verify_merkle_integrity(ep1.id()).unwrap());

        let folded_l4 = dag.fold_epochs(&[ep0.id(), ep1.id()]).unwrap();
        assert!(dag.verify_merkle_integrity(folded_l4.id()).unwrap());
        assert!(folded_l4.ahtl_representation().contains("folded=2"));

        // Verify DAG-aware handle resolution and dereferencing on L2, L3, and L4 handles
        let outer_deref = dag.deref_node_text(outer_branch_id).unwrap();
        assert!(outer_deref.contains("borrow_resolved"));
        assert!(outer_deref.contains(&inner_branch_id.to_string()));
        assert_eq!(
            dag.resolve_handle(&ep0.id().to_short_handle()).unwrap(),
            ep0.id()
        );
        let l4_deref = dag.deref_node_text(folded_l4.id()).unwrap();
        assert!(l4_deref.contains("ep=0000"));
        assert!(l4_deref.contains("ep=0001"));
        let l4_slice = dag.deref_node_lines(folded_l4.id(), 1, 2).unwrap();
        assert!(l4_slice.contains("folded=2"));

        // Corrupt a leaf turn digest and verify recursive Merkle check fails closed
        dag.corrupt_node_digest_for_test(t1);
        assert!(matches!(
            dag.verify_merkle_integrity(folded_l4.id()),
            Err(IcxError::IntegrityViolation(_))
        ));
    }
}
