//! Unified `InfinityContextMmu` coordinator integrating CAS, Merkle-DAG, Cache-Epoch Controller,
//! 10,240-bit HDC + Fractal index, Bi-Temporal PPR Graph, AHTL Skeletonizer, and `/dev/shm`
//! Seqlock Telepathy Bus.

use icx_bus::{SeqlockTelemetryBus, TelepathyEvent};
use icx_codec::{SecretRedactor, StructuralSkeletonizer};
use icx_core::cas::{CasStore, ColdMatch};
use icx_core::dag::MerkleContextDag;
use icx_core::epoch::{AssembledWindow, CacheEpochController, EpochControllerConfig};
use icx_core::{AgentKind, ArtifactId, IcxError};
use icx_hdc::hv::Hypervector;
use icx_hdc::{AssociativeHit, HdcAssociativeIndex};
use icx_store::BiTemporalGraph;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Default inline byte threshold above which tool outputs are virtualized into AHTL Holographic Stubs.
pub const DEFAULT_TOOL_VIRTUALIZE_THRESHOLD_BYTES: usize = 512;

/// Summary of an ingested tool or turn event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IngestResult {
    pub artifact_id: ArtifactId,
    pub raw_bytes: usize,
    pub compressed_ahtl: String,
    pub surprise: f32,
    pub was_virtualized: bool,
}

/// The central Virtual Context Memory Management Unit (`MMU`).
#[derive(Clone, Debug)]
pub struct InfinityContextMmu {
    pub cas: CasStore,
    pub dag: MerkleContextDag,
    pub epochs: CacheEpochController,
    pub hdc_index: HdcAssociativeIndex,
    pub graph: BiTemporalGraph,
    pub bus: SeqlockTelemetryBus,
    pub tool_virtualize_threshold: usize,
    pub workspace_hash: u64,
    agent_read_cursors: BTreeMap<u8, u64>,
    clock_ms: u64,
}

impl Default for InfinityContextMmu {
    fn default() -> Self {
        Self::new(
            EpochControllerConfig::default(),
            SeqlockTelemetryBus::new_in_memory(),
        )
    }
}

impl InfinityContextMmu {
    /// Creates a new `InfinityContextMmu`.
    pub fn new(epoch_cfg: EpochControllerConfig, bus: SeqlockTelemetryBus) -> Self {
        let legend = "⟪ICX:LEGEND│Δ=code_mut│Ψ=tool_stub│Ω=invariant│Θ=xagent│Φ=folded_branch│↦=deref_via_icx_tool⟫";
        Self {
            cas: CasStore::new(),
            dag: MerkleContextDag::new(),
            epochs: CacheEpochController::new(legend, epoch_cfg),
            hdc_index: HdcAssociativeIndex::new(),
            graph: BiTemporalGraph::new(),
            bus,
            tool_virtualize_threshold: DEFAULT_TOOL_VIRTUALIZE_THRESHOLD_BYTES,
            workspace_hash: 0x4943_585F_574F_524B,
            agent_read_cursors: BTreeMap::new(),
            clock_ms: 1_794_880_800_000,
        }
    }

    /// Ingests a tool output or turn, redacting secrets, storing lossless bytes in `CasStore`,
    /// indexing in 10,240-bit HDC + Fractal + Bi-Temporal Graph, recording in the Merkle-DAG,
    /// and broadcasting high-surprise events onto the `/dev/shm` Seqlock Telepathy Bus.
    pub fn ingest_payload(
        &mut self,
        agent: AgentKind,
        subject: &str,
        raw_text: &str,
        force_anomaly: bool,
    ) -> IngestResult {
        self.clock_ms += 10;
        let mut sanitized = SecretRedactor::sanitize(raw_text);
        if sanitized.len() > icx_core::ICX_MAX_INGEST_BYTES {
            let mut bound = icx_core::ICX_MAX_INGEST_BYTES;
            while !sanitized.is_char_boundary(bound) {
                bound -= 1;
            }
            sanitized.truncate(bound);
        }
        let raw_bytes = sanitized.len();
        let (artifact_id, _) = self.cas.put_bytes(sanitized.as_bytes());

        let receipt = self
            .hdc_index
            .observe_event(artifact_id, subject, &sanitized, force_anomaly);

        let packet = StructuralSkeletonizer::compress_to_packet(
            subject,
            &sanitized,
            Some(artifact_id),
            receipt.hologram_bytes,
        );
        let ahtl_rendered = packet.render();

        // Populate Bi-Temporal Graph with extracted clause relations
        for clause in &packet.clauses {
            if let Some(val) = &clause.value {
                let is_functional =
                    clause.key == "status" || clause.key == "fail" || clause.key == "pass";
                self.graph.upsert_edge(
                    subject,
                    &clause.key,
                    val,
                    is_functional,
                    self.clock_ms,
                    artifact_id,
                    receipt.verdict.momentum_surprise.max(0.2),
                );
            }
        }

        let est_tokens = (raw_bytes / 4).max(1);
        self.epochs.record_turn(
            &mut self.dag,
            agent,
            subject,
            artifact_id,
            ahtl_rendered.clone(),
            receipt.verdict.momentum_surprise,
            est_tokens,
        );

        // Broadcast onto cross-agent Seqlock ring
        let telepathy_frame = format!(
            "⟦Θ:{}@{}⊢{}⟧",
            agent.as_str(),
            subject,
            ahtl_rendered.trim_start_matches('⟦').trim_end_matches('⟧')
        );
        let hv_sketch = Hypervector::from_seed("subj", subject).sketch_256();
        let frac_q15 = receipt.fractal_coord.to_q15();
        let surp_q8 = (receipt.verdict.momentum_surprise.clamp(0.0, 1.0) * 255.0) as u8;

        self.bus.publish(
            agent,
            artifact_id,
            self.workspace_hash,
            surp_q8,
            hv_sketch,
            frac_q15,
            &telepathy_frame,
            self.clock_ms,
        );

        let was_virtualized = raw_bytes >= self.tool_virtualize_threshold;
        IngestResult {
            artifact_id,
            raw_bytes,
            compressed_ahtl: ahtl_rendered,
            surprise: receipt.verdict.momentum_surprise,
            was_virtualized,
        }
    }

    /// Builds the dynamic Ephemeral Tail Slot for `requesting_agent` given `query`, combining:
    /// 1. Unseen `/dev/shm` cross-agent telepathy commits from sibling agents.
    /// 2. Top-$K$ 10,240-bit HDC + Fractal associative matches.
    /// 3. Multi-hop Bi-Temporal Personalized PageRank (`PPR`) provenance hits.
    pub fn build_ephemeral_tail_slot(
        &mut self,
        requesting_agent: AgentKind,
        query: &str,
        top_k: usize,
    ) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "⟪ICX:TAIL_SLOT│agent={}│epochs={}│cas_artifacts={}⟫",
            requesting_agent.as_str(),
            self.epochs.sealed_epoch_count(),
            self.cas.artifact_count()
        ));

        // 1. Cross-agent telepathy updates
        let cursor = self
            .agent_read_cursors
            .get(&(requesting_agent as u8))
            .copied()
            .unwrap_or(0);
        let telepathy: Vec<TelepathyEvent> = self.bus.poll_since(cursor);
        if let Some(last) = telepathy.last() {
            self.agent_read_cursors
                .insert(requesting_agent as u8, last.seq);
        }
        for ev in telepathy
            .into_iter()
            .filter(|e| e.agent != requesting_agent)
            .take(4)
        {
            lines.push(ev.ahtl_packet);
        }

        // 2. HDC + Fractal associative hits
        let hdc_hits: Vec<AssociativeHit> = self.hdc_index.query_associative(query, top_k);
        for hit in &hdc_hits {
            lines.push(format!(
                "⟦Ψ:HDC_RECALL⊢subj={}⋮sim={:.2}⋮suffix={}↦{}⟧",
                hit.subject, hit.hdc_similarity, hit.shared_fractal_suffix, hit.artifact_id
            ));
        }

        // 3. Bi-Temporal Personalized PageRank multi-hop walk
        let seeds: Vec<&str> = query
            .split_whitespace()
            .filter(|w| w.len() >= 3)
            .take(4)
            .collect();
        if !seeds.is_empty() {
            let ppr_nodes = self
                .graph
                .personalized_pagerank(&seeds, None, 0.15, 6, top_k);
            for node in ppr_nodes {
                if let Some(&prov) = node.provenance_artifacts.first() {
                    lines.push(format!(
                        "⟦Ω:PPR_GRAPH⊢entity={}⋮score={:.3}↦{}⟧",
                        node.entity, node.score, prov
                    ));
                }
            }
        }

        lines.join("\n")
    }

    /// Assembles the complete 4-zone Prefix-Cache-Locked window for `agent` and `query`.
    pub fn assemble_prompt_window(&mut self, agent: AgentKind, query: &str) -> AssembledWindow {
        let tail = self.build_ephemeral_tail_slot(agent, query, 3);
        self.epochs.assemble_window(&self.dag, tail)
    }

    /// Dereferences full lossless bytes as UTF-8 text from an `#icx:<hex>` handle.
    pub fn deref_text(&self, id: ArtifactId) -> Result<String, IcxError> {
        let bytes = self.cas.deref_bytes(id)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Dereferences an inclusive 1-based line slice from an `#icx:<hex>` handle.
    pub fn deref_lines(
        &self,
        id: ArtifactId,
        start_line: usize,
        end_line: usize,
    ) -> Result<String, IcxError> {
        self.cas.deref_lines(id, start_line, end_line)
    }

    /// Searches cold storage for `pattern`.
    pub fn grep_cold(
        &self,
        id: Option<ArtifactId>,
        pattern: &str,
        max_matches: usize,
    ) -> Result<Vec<ColdMatch>, IcxError> {
        self.cas.grep_cold(id, pattern, max_matches)
    }
}
