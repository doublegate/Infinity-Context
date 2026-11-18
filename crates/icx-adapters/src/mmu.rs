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
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

static MMU_TMP_COUNTER: AtomicU64 = AtomicU64::new(0);
static WORKSPACE_IO_LOCK: Mutex<()> = Mutex::new(());

/// Default inline byte threshold above which tool outputs are virtualized into AHTL Holographic Stubs.
pub const DEFAULT_TOOL_VIRTUALIZE_THRESHOLD_BYTES: usize = 512;

/// Canonical AHTL v1.0 operator legend injected at session start (`Zone 0`).
pub const ICX_AHTL_LEGEND: &str =
    "⟪ICX:LEGEND│Δ=code_mut│Ψ=tool_stub│Ω=invariant│Θ=xagent│Φ=folded_branch│↦=deref_via_icx_tool⟫";

/// Summary of an ingested tool or turn event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IngestResult {
    pub artifact_id: ArtifactId,
    pub raw_bytes: usize,
    pub compressed_ahtl: String,
    pub surprise: f32,
    pub was_virtualized: bool,
}

type SubagentStep = (String, ArtifactId, String, f32, usize);

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct PersistedMmuState {
    clock_ms: u64,
    redacted_ids: Vec<u64>,
    #[serde(default)]
    subagent_branches: BTreeMap<String, Vec<SubagentStep>>,
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
    redacted_artifacts: BTreeSet<ArtifactId>,
    subagent_branches: BTreeMap<String, Vec<SubagentStep>>,
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
        Self {
            cas: CasStore::new(),
            dag: MerkleContextDag::new(),
            epochs: CacheEpochController::new(ICX_AHTL_LEGEND, epoch_cfg),
            hdc_index: HdcAssociativeIndex::new(),
            graph: BiTemporalGraph::new(),
            bus,
            tool_virtualize_threshold: DEFAULT_TOOL_VIRTUALIZE_THRESHOLD_BYTES,
            workspace_hash: 0x4943_585F_574F_524B,
            agent_read_cursors: BTreeMap::new(),
            redacted_artifacts: BTreeSet::new(),
            subagent_branches: BTreeMap::new(),
            clock_ms: 1_794_880_800_000,
        }
    }

    /// Extracts an optional process `exit_code` from a structured hook JSON payload
    /// (checking top-level and object-typed nested `"tool_response"`, `"toolResponse"`,
    /// `"toolResult"`, or `"result"` containers for `"exit_code"` and `"exitCode"`).
    pub fn extract_exit_code(payload: &serde_json::Value) -> Option<i32> {
        let parse_code = |v: &serde_json::Value| -> Option<i32> {
            v.as_i64().and_then(|n| i32::try_from(n).ok())
        };
        let extract_from_obj = |obj: &serde_json::Value| -> Option<i32> {
            if !obj.is_object() {
                return None;
            }
            for key in ["exit_code", "exitCode"] {
                if let Some(code) = obj.get(key).and_then(parse_code) {
                    return Some(code);
                }
            }
            None
        };
        if let Some(code) = extract_from_obj(payload) {
            return Some(code);
        }
        for container in ["tool_response", "toolResponse", "toolResult", "result"] {
            if let Some(resp) = payload.get(container)
                && let Some(code) = extract_from_obj(resp)
            {
                return Some(code);
            }
        }
        None
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
        self.ingest_payload_with_exit_code(agent, subject, raw_text, None, force_anomaly)
    }

    /// Ingests a tool output or turn with an optional process `exit_code` (`Some(code != 0)`
    /// triggers Titans anomaly boost and records a functional `exit_code` edge in `BiTemporalGraph`).
    pub fn ingest_payload_with_exit_code(
        &mut self,
        agent: AgentKind,
        subject: &str,
        raw_text: &str,
        exit_code: Option<i32>,
        force_anomaly: bool,
    ) -> IngestResult {
        self.ingest_payload_for_subagent(agent, subject, raw_text, exit_code, force_anomaly, None)
    }

    /// Bounds `raw_text` to `max_bytes` (backing up to a valid UTF-8 and newline boundary when
    /// possible), applies `SecretRedactor::sanitize`, and re-bounds post-sanitization output.
    pub fn bound_and_sanitize_raw(raw_text: &str, max_bytes: usize) -> (String, bool) {
        let bounded_raw = if raw_text.len() > max_bytes {
            let mut bound = max_bytes;
            while !raw_text.is_char_boundary(bound) {
                bound -= 1;
            }
            let search_floor = bound.saturating_sub(4096);
            if let Some(rel_nl) = raw_text.as_bytes()[search_floor..bound]
                .iter()
                .rposition(|&b| b == b'\n')
            {
                bound = search_floor + rel_nl + 1;
            }
            &raw_text[..bound]
        } else {
            raw_text
        };
        let mut sanitized = SecretRedactor::sanitize(bounded_raw);
        let was_redacted = bounded_raw != sanitized;
        if sanitized.len() > max_bytes {
            let mut bound = max_bytes;
            while !sanitized.is_char_boundary(bound) {
                bound -= 1;
            }
            sanitized.truncate(bound);
        }
        (sanitized, was_redacted)
    }

    /// Ingests a tool output or turn with an optional process `exit_code` and optional
    /// `subagent_id` scope. When `subagent_id` is `Some(...)`, the turn is buffered inside that
    /// subagent's isolated trajectory branch rather than being appended directly onto the parent's
    /// active epoch window.
    pub fn ingest_payload_for_subagent(
        &mut self,
        agent: AgentKind,
        raw_subject: &str,
        raw_text: &str,
        exit_code: Option<i32>,
        force_anomaly: bool,
        subagent_id: Option<&str>,
    ) -> IngestResult {
        self.clock_ms += 10;
        let clean_subject = Self::sanitize_hook_label(raw_subject, "event");
        let subject = clean_subject.as_str();
        let (sanitized, body_redacted) =
            Self::bound_and_sanitize_raw(raw_text, icx_core::ICX_MAX_INGEST_BYTES);
        let was_redacted = body_redacted || subject != raw_subject.trim();
        let raw_bytes = sanitized.len();
        let (artifact_id, _) = self.cas.put_bytes(sanitized.as_bytes());

        let mut receipt = self.hdc_index.observe_event_with_exit_code(
            artifact_id,
            subject,
            &sanitized,
            exit_code,
            force_anomaly,
        );
        if was_redacted {
            receipt.hologram_bytes[7] |= icx_codec::FLAG_IS_REDACTED;
        }
        if body_redacted {
            self.redacted_artifacts.insert(artifact_id);
        }

        let mut packet = StructuralSkeletonizer::compress_sanitized_to_packet(
            subject,
            &sanitized,
            Some(artifact_id),
            receipt.hologram_bytes,
        );
        packet.clauses.retain(|c| c.key != "exit_code");
        if let Some(code) = exit_code {
            packet.clauses.push(icx_codec::AhtlClause::new(
                if code == 0 {
                    icx_codec::ClauseOp::Verified
                } else {
                    icx_codec::ClauseOp::Anomaly
                },
                "exit_code",
                Some(code.to_string()),
            ));
        }
        let ahtl_rendered = packet.render();

        // Populate Bi-Temporal Graph with extracted clause relations
        let edge_weight = receipt.verdict.momentum_surprise.max(0.2);
        let mut saw_pass = false;
        let mut saw_fail = false;
        let mut saw_anomaly_clause = false;
        for clause in &packet.clauses {
            if clause.op == icx_codec::ClauseOp::Anomaly {
                saw_anomaly_clause = true;
            }
            if let Some(val) = clause.value.as_deref().filter(|v| !v.trim().is_empty())
                && !clause.key.trim().is_empty()
            {
                let key = clause.key.as_str();
                saw_pass |= key == "pass";
                saw_fail |= key == "fail";
                let is_functional = matches!(
                    key,
                    "status" | "test_status" | "return_type" | "fail" | "pass" | "exit_code"
                );
                self.graph.upsert_edge(
                    subject,
                    key,
                    val,
                    is_functional,
                    self.clock_ms,
                    artifact_id,
                    edge_weight,
                );
            }
        }
        // Clear-state functional supersession: non-zero exit codes, force_anomaly, or any
        // diagnostic anomaly/failure clause take strict precedence and record `status = FAIL`;
        // only clean passing runs (`saw_pass` or `exit_code == Some(0)`) supersede prior failures.
        let has_failure =
            force_anomaly || exit_code.is_some_and(|c| c != 0) || saw_fail || saw_anomaly_clause;
        if has_failure {
            self.graph.upsert_edge(
                subject,
                "status",
                "FAIL",
                true,
                self.clock_ms,
                artifact_id,
                edge_weight,
            );
        } else if saw_pass || exit_code == Some(0) {
            self.graph.upsert_edge(
                subject,
                "fail",
                "0",
                true,
                self.clock_ms,
                artifact_id,
                edge_weight,
            );
            self.graph.upsert_edge(
                subject,
                "status",
                "PASS",
                true,
                self.clock_ms,
                artifact_id,
                edge_weight,
            );
        }

        let est_tokens = (raw_bytes / 4).max(1);
        if let Some(raw_sub_id) = subagent_id {
            let clean_sub_id = Self::sanitize_hook_label(raw_sub_id, "subagent");
            self.subagent_branches
                .entry(clean_sub_id)
                .or_default()
                .push((
                    clean_subject.clone(),
                    artifact_id,
                    ahtl_rendered.clone(),
                    receipt.verdict.momentum_surprise,
                    est_tokens,
                ));
        } else {
            self.epochs.record_turn(
                &mut self.dag,
                agent,
                subject,
                artifact_id,
                ahtl_rendered.clone(),
                receipt.verdict.momentum_surprise,
                est_tokens,
            );
            self.sync_global_dag_root();
        }

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

        // 1. Cross-agent telepathy updates (workspace-scoped)
        let cursor = self
            .agent_read_cursors
            .get(&(requesting_agent as u8))
            .copied()
            .unwrap_or(0);
        let telepathy: Vec<TelepathyEvent> =
            self.bus.poll_workspace_since(cursor, self.workspace_hash);
        let mut new_cursor = cursor;
        let mut sibling_taken = 0usize;
        for ev in telepathy {
            if ev.agent == requesting_agent {
                new_cursor = new_cursor.max(ev.seq);
            } else if sibling_taken < 4 {
                new_cursor = new_cursor.max(ev.seq);
                sibling_taken += 1;
                lines.push(ev.ahtl_packet);
            } else {
                break;
            }
        }
        if new_cursor > cursor {
            self.agent_read_cursors
                .insert(requesting_agent as u8, new_cursor);
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

    /// Resolves either a 64-bit (`#icx:<16-hex>`) or 48-bit (`#icx:<12-hex>`) handle across both
    /// `CasStore` (`L0` artifacts) and `MerkleContextDag` (`L1`–`L4` turns, folded branches, and epochs).
    pub fn resolve_handle(&self, handle_or_prefix: &str) -> Result<ArtifactId, IcxError> {
        let cas_res = self.cas.resolve_handle(handle_or_prefix);
        let dag_res = self.dag.resolve_handle(handle_or_prefix);
        match (cas_res, dag_res) {
            (Ok(cid), Ok(did)) => {
                if cid == did {
                    Ok(cid)
                } else {
                    Err(IcxError::AmbiguousHandlePrefix(handle_or_prefix.to_owned()))
                }
            }
            (Ok(cid), Err(IcxError::ArtifactNotFound(_))) => Ok(cid),
            (Err(IcxError::ArtifactNotFound(_)), Ok(did)) => Ok(did),
            (Err(e), _) | (_, Err(e)) => Err(e),
        }
    }

    /// Dereferences full lossless bytes as UTF-8 text from an `#icx:<hex>` handle (`CasStore` or `MerkleContextDag`).
    pub fn deref_text(&self, id: ArtifactId) -> Result<String, IcxError> {
        match self.cas.deref_bytes(id) {
            Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
            Err(IcxError::ArtifactNotFound(_)) => self.dag.deref_node_text(id),
            Err(e) => Err(e),
        }
    }

    /// Dereferences an inclusive 1-based line slice from an `#icx:<hex>` handle (`CasStore` or `MerkleContextDag`).
    pub fn deref_lines(
        &self,
        id: ArtifactId,
        start_line: usize,
        end_line: usize,
    ) -> Result<String, IcxError> {
        match self.cas.deref_lines(id, start_line, end_line) {
            Ok(text) => Ok(text),
            Err(IcxError::ArtifactNotFound(_)) => {
                self.dag.deref_node_lines(id, start_line, end_line)
            }
            Err(e) => Err(e),
        }
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

    /// Returns `true` if pre-CAS secret redaction or delimiter escaping modified `id` during ingestion.
    #[inline]
    pub fn is_artifact_redacted(&self, id: ArtifactId) -> bool {
        self.redacted_artifacts.contains(&id)
    }

    /// Returns the current logical ingestion clock timestamp in milliseconds.
    #[inline]
    pub const fn clock_ms(&self) -> u64 {
        self.clock_ms
    }

    /// Sanitizes a subject, subagent identifier, or summary string through `SecretRedactor::sanitize`
    /// and strips frame-breaking AHTL delimiters and newline characters.
    pub fn sanitize_hook_label(raw: &str, fallback: &str) -> String {
        let s = SecretRedactor::sanitize(raw)
            .replace(['│', '\n', '\r', '⟦', '⟧', '⟪', '⟫', '⊢', '⋮', '↦'], " ")
            .trim()
            .to_owned();
        if s.is_empty() { fallback.to_owned() } else { s }
    }

    /// Returns the sole open subagent ID when exactly one subagent scope is active.
    pub fn single_active_subagent_id(&self) -> Option<String> {
        if self.subagent_branches.len() == 1 {
            self.subagent_branches.keys().next().cloned()
        } else {
            None
        }
    }

    /// Opens an ID-keyed subagent trajectory scope (`SubagentStart`), isolating concurrent
    /// sibling subagents from each other and from the parent trajectory.
    pub fn begin_subagent(&mut self, raw_subagent_id: &str) -> String {
        let clean_id = Self::sanitize_hook_label(raw_subagent_id, "subagent");
        self.subagent_branches.entry(clean_id.clone()).or_default();
        clean_id
    }

    /// Folds any remaining open subagent trajectory scopes (e.g., on `Stop` or `SessionEnd`).
    pub fn fold_all_open_subagents(&mut self, agent: AgentKind, reason: &str) {
        let keys: Vec<String> = self.subagent_branches.keys().cloned().collect();
        for key in keys {
            let _ = self.finish_subagent(agent, Some(&key), reason);
        }
    }

    /// Synchronizes the latest sealed `L3`/`L4` epoch digest onto the `/dev/shm` `FabricHeader`.
    pub fn sync_global_dag_root(&mut self) {
        if let Some(last_ep) = self.epochs.sealed_epochs().last() {
            self.bus.set_global_dag_root(last_ep.digest().0);
        }
    }

    /// Triggers `CacheEpochController::maybe_rollover` and synchronizes `global_dag_root` on `self.bus`.
    pub fn rollover_and_sync_dag_root(&mut self) -> Option<ArtifactId> {
        let ep = self.epochs.maybe_rollover(&mut self.dag);
        self.sync_global_dag_root();
        ep
    }

    /// Completes and folds the trajectory belonging to `raw_subagent_id` (`SubagentStop`),
    /// sanitizing `raw_summary` through `SecretRedactor::sanitize` before committing the `L2`
    /// `SubTrajectoryBranch` node to `MerkleContextDag`.
    pub fn finish_subagent(
        &mut self,
        agent: AgentKind,
        raw_subagent_id: Option<&str>,
        raw_summary: &str,
    ) -> String {
        let clean_summary = Self::sanitize_hook_label(raw_summary, "subagent_completed");
        let target_key = match raw_subagent_id {
            Some(id) => Some(Self::sanitize_hook_label(id, "subagent")),
            None => self.single_active_subagent_id(),
        };
        if let Some((clean_id, steps)) =
            target_key.and_then(|k| self.subagent_branches.remove_entry(&k))
        {
            self.epochs.begin_branch(&mut self.dag, &clean_id);
            if steps.is_empty() {
                let (aid, _) = self.cas.put_bytes(clean_summary.as_bytes());
                if clean_summary != raw_summary.trim() {
                    self.redacted_artifacts.insert(aid);
                }
                let _ = self.dag.append_turn(
                    agent,
                    &clean_id,
                    aid,
                    format!("⟦Φ:{clean_id}⊢{clean_summary}⟧"),
                    0.3,
                    (clean_summary.len() / 4).max(1),
                );
            } else {
                for (role, aid, ahtl, surp, est) in steps {
                    let _ = self.dag.append_turn(agent, &role, aid, ahtl, surp, est);
                }
            }
            let bid = self
                .epochs
                .fold_branch(&mut self.dag, &clean_summary)
                .expect("active subagent branch must fold");
            self.sync_global_dag_root();
            return format!("⟪ICX:SUBAGENT_FOLDED│{bid}│{clean_id}│{clean_summary}⟫");
        }
        format!("⟪ICX:SUBAGENT_STOP│{clean_summary}⟫")
    }

    fn map_io_err(err: impl std::fmt::Display) -> IcxError {
        IcxError::Io(err.to_string())
    }

    /// Acquires the shared workspace lock (`cas_store.lock` + in-process `WORKSPACE_IO_LOCK`)
    /// coordinating all workspace readers and writers across `icx-adapters` and `icx-cli`.
    pub fn acquire_workspace_lock(
        icx_dir: &Path,
    ) -> Result<(std::sync::MutexGuard<'static, ()>, std::fs::File), IcxError> {
        let mem_guard = WORKSPACE_IO_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        std::fs::create_dir_all(icx_dir).map_err(Self::map_io_err)?;
        let lock_path = icx_dir.join("cas_store.lock");
        let mut opts = std::fs::OpenOptions::new();
        opts.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let lock_file = opts.open(&lock_path).map_err(Self::map_io_err)?;
        lock_file.lock().map_err(Self::map_io_err)?;
        Ok((mem_guard, lock_file))
    }

    /// Loads persisted `CasStore`, `BiTemporalGraph`, `redacted_artifacts`, `subagent_branches`,
    /// logical `clock_ms`, and shared-memory `SeqlockTelemetryBus` state from `icx_dir` atomically
    /// under an exclusive workspace lock (only mutating `self` after all components succeed).
    pub fn load_workspace_dir(&mut self, icx_dir: &Path) -> Result<(), IcxError> {
        let _lock = Self::acquire_workspace_lock(icx_dir)?;
        let cas_path = icx_dir.join("cas_store.json");
        let new_cas = if cas_path.exists() {
            Some(CasStore::load_from_dir(icx_dir)?)
        } else {
            None
        };
        let graph_path = icx_dir.join("bitemporal_graph.json");
        let new_graph = if graph_path.exists() {
            Some(BiTemporalGraph::load_from_dir(icx_dir)?)
        } else {
            None
        };
        let state_path = icx_dir.join("mmu_state.json");
        let persisted_state: Option<PersistedMmuState> = if state_path.exists() {
            let bytes = std::fs::read(&state_path).map_err(Self::map_io_err)?;
            Some(serde_json::from_slice(&bytes).map_err(Self::map_io_err)?)
        } else {
            None
        };
        let redacted_path = icx_dir.join("redacted_artifacts.json");
        let raw_redacted: Option<Vec<u64>> = if persisted_state.is_none() && redacted_path.exists()
        {
            let bytes = std::fs::read(&redacted_path).map_err(Self::map_io_err)?;
            Some(serde_json::from_slice(&bytes).map_err(Self::map_io_err)?)
        } else {
            None
        };
        let shm_path = icx_dir.join("icx-fabric-v1.shm");
        let new_bus = if shm_path.exists() {
            Some(SeqlockTelemetryBus::open_shared_file(&shm_path).map_err(Self::map_io_err)?)
        } else {
            None
        };

        if let Some(cas) = new_cas {
            self.cas = cas;
        }
        if let Some(graph) = new_graph {
            self.graph = graph;
        }
        let max_graph_ms = self
            .graph
            .edges()
            .iter()
            .map(|e| {
                e.t_valid
                    .max(e.t_invalid.unwrap_or(0))
                    .max(e.t_tx_start)
                    .max(e.t_tx_end.unwrap_or(0))
            })
            .max()
            .unwrap_or(0);
        self.redacted_artifacts.clear();
        if let Some(st) = persisted_state {
            self.clock_ms = self.clock_ms.max(st.clock_ms).max(max_graph_ms);
            self.subagent_branches = st
                .subagent_branches
                .into_iter()
                .map(|(k, steps)| {
                    let valid_steps = steps
                        .into_iter()
                        .filter(|(_, aid, _, _, _)| self.cas.get_meta(*aid).is_some())
                        .collect();
                    (k, valid_steps)
                })
                .collect();
            self.redacted_artifacts.extend(
                st.redacted_ids
                    .into_iter()
                    .map(ArtifactId)
                    .filter(|&aid| self.cas.get_meta(aid).is_some()),
            );
        } else {
            self.clock_ms = self.clock_ms.max(max_graph_ms);
        }
        if let Some(ids) = raw_redacted {
            self.redacted_artifacts.extend(
                ids.into_iter()
                    .map(ArtifactId)
                    .filter(|&aid| self.cas.get_meta(aid).is_some()),
            );
        }
        if let Some(bus) = new_bus {
            self.bus = bus;
        }
        Ok(())
    }

    fn write_atomic_file(icx_dir: &Path, filename: &str, bytes: &[u8]) -> Result<(), IcxError> {
        let final_path = icx_dir.join(filename);
        let seq = MMU_TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let tmp_path = icx_dir.join(format!("{filename}.tmp.{}.{seq}", std::process::id()));
        {
            use std::io::Write;
            let mut opts = std::fs::OpenOptions::new();
            opts.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(0o600);
            }
            let mut file = opts.open(&tmp_path).map_err(Self::map_io_err)?;
            file.write_all(bytes).map_err(Self::map_io_err)?;
            file.sync_all().map_err(Self::map_io_err)?;
        }
        std::fs::rename(&tmp_path, &final_path).map_err(Self::map_io_err)
    }

    /// Atomically persists `CasStore`, `BiTemporalGraph`, `redacted_artifacts`, `subagent_branches`,
    /// and logical `clock_ms` state into `icx_dir` under an exclusive workspace lock.
    pub fn save_workspace_dir(&self, icx_dir: &Path) -> Result<(), IcxError> {
        let _lock = Self::acquire_workspace_lock(icx_dir)?;
        self.cas.save_to_dir(icx_dir)?;
        self.graph.save_to_dir(icx_dir)?;
        let redacted_ids: Vec<u64> = self.redacted_artifacts.iter().map(|id| id.0).collect();
        let state = PersistedMmuState {
            clock_ms: self.clock_ms,
            redacted_ids,
            subagent_branches: self.subagent_branches.clone(),
        };
        let state_bytes = serde_json::to_vec(&state).map_err(Self::map_io_err)?;
        Self::write_atomic_file(icx_dir, "mmu_state.json", &state_bytes)
    }
}
