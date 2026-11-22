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

/// Recall entries weaker than this on similarity, fractal suffix and combined score are not
/// shown (v1.3.2).
const MIN_RECALL_SIMILARITY: f32 = 0.10;

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
type HdcEventRecord = (ArtifactId, String, Option<i32>, bool);
/// Indexed occurrence record stored per conversation slot `(agent_u8, clean_subject, artifact_id, exit_code, force_anomaly, ingest_result)`.
pub type IndexedOccurrenceRecord = (u8, String, ArtifactId, Option<i32>, bool, IngestResult);

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct PersistedMmuState {
    clock_ms: u64,
    redacted_ids: Vec<u64>,
    #[serde(default)]
    subagent_branches: BTreeMap<String, Vec<SubagentStep>>,
    #[serde(default)]
    dag: MerkleContextDag,
    #[serde(default)]
    epochs: Option<CacheEpochController>,
    #[serde(default)]
    hdc_events: Vec<HdcEventRecord>,
    #[serde(default)]
    agent_read_cursors: BTreeMap<u8, u64>,
    #[serde(default)]
    indexed_occurrences: BTreeMap<usize, IndexedOccurrenceRecord>,
    #[serde(default)]
    reordered_occurrences: BTreeMap<usize, IndexedOccurrenceRecord>,
    #[serde(default)]
    agent_occurrences: BTreeMap<u8, BTreeMap<usize, IndexedOccurrenceRecord>>,
    #[serde(default)]
    agent_reordered_occurrences: BTreeMap<u8, BTreeMap<usize, IndexedOccurrenceRecord>>,
    /// Legacy JSON workspaces only; the SQLite store keeps these in its append-only `emitted`
    /// table so saves stay incremental (PR #20 review).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    emitted_blocks: BTreeSet<[u8; 32]>,
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
    hdc_events: Vec<HdcEventRecord>,
    indexed_occurrences: BTreeMap<usize, IndexedOccurrenceRecord>,
    reordered_occurrences: BTreeMap<usize, IndexedOccurrenceRecord>,
    agent_occurrences: BTreeMap<u8, BTreeMap<usize, IndexedOccurrenceRecord>>,
    agent_reordered_occurrences: BTreeMap<u8, BTreeMap<usize, IndexedOccurrenceRecord>>,
    clock_ms: u64,
    /// CAS rows known to be present in the SQLite store (keeps saves incremental).
    persisted_ids: crate::store::PersistedIds,
    /// `(canonical workspace dir, store generation)` this MMU state mirrors, if any.
    loaded_generation: Option<(std::path::PathBuf, u64)>,
    /// When set, saves MAC-stamp the store and loads reject stores without a valid stamp.
    store_seal_key: Option<[u8; 32]>,
    /// Set when the seal key changes: the next load skips the unchanged-generation fast path.
    reload_required: bool,
    /// BLAKE3 digests of every icx-formatted block this MMU handed to an agent (ICX-AUD-011).
    emitted_blocks: BTreeSet<[u8; 32]>,
    /// When `true`, event times come from the wall clock (milliseconds since the Unix epoch,
    /// kept strictly monotonic); otherwise a deterministic logical counter (ICX-AUD-048).
    wall_clock: bool,
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
            hdc_events: Vec::new(),
            indexed_occurrences: BTreeMap::new(),
            reordered_occurrences: BTreeMap::new(),
            agent_occurrences: BTreeMap::new(),
            agent_reordered_occurrences: BTreeMap::new(),
            clock_ms: 1_794_880_800_000,
            persisted_ids: crate::store::PersistedIds::default(),
            loaded_generation: None,
            store_seal_key: None,
            reload_required: false,
            emitted_blocks: BTreeSet::new(),
            wall_clock: false,
        }
    }

    /// Returns `true` if `tool_name` refers to Infinity-Context's own MCP pager tool (`icx` /
    /// `mcp__*__icx`), whose dereferenced pages must pass through to the model without
    /// re-virtualization in `PostToolUse`.
    pub fn is_icx_pager_tool(tool_name: &str) -> bool {
        let t = tool_name.trim();
        t.eq_ignore_ascii_case("icx")
            || t.ends_with("__icx")
            || t.starts_with("mcp__icx__")
            || t.starts_with("mcp__infinity_context")
            || t.starts_with("mcp__infinity-context")
    }

    /// Extracts a command-scoped subject (`"<tool_name>:<command>"`) from a hook JSON payload
    /// when a command/target input is present so distinct commands sharing a tool name (e.g.
    /// `Bash`) do not overwrite each other's functional graph state.
    pub fn extract_hook_subject(payload: &serde_json::Value, default_tool: &str) -> String {
        let tool_name = payload
            .get("tool_name")
            .or_else(|| payload.get("toolName"))
            .or_else(|| payload.get("toolCall").and_then(|tc| tc.get("name")))
            .and_then(serde_json::Value::as_str)
            .unwrap_or(default_tool);
        let cmd_opt = ["tool_input", "toolInput", "input"]
            .into_iter()
            .find_map(|container| {
                payload.get(container).and_then(|obj| {
                    ["command", "cmd", "CommandLine"]
                        .into_iter()
                        .find_map(|k| obj.get(k).and_then(serde_json::Value::as_str))
                })
            })
            .or_else(|| {
                payload
                    .get("toolCall")
                    .and_then(|tc| tc.get("args"))
                    .and_then(|args| {
                        ["command", "cmd", "CommandLine"]
                            .into_iter()
                            .find_map(|k| args.get(k).and_then(serde_json::Value::as_str))
                    })
            })
            .or_else(|| {
                ["command", "cmd", "CommandLine"]
                    .into_iter()
                    .find_map(|k| payload.get(k).and_then(serde_json::Value::as_str))
            })
            .map(str::trim)
            .filter(|s| !s.is_empty());
        match cmd_opt {
            Some(cmd) if cmd.len() <= 64 => format!("{tool_name}:{cmd}"),
            Some(cmd) => {
                let mut end = 48;
                while !cmd.is_char_boundary(end) {
                    end -= 1;
                }
                let cmd_hash =
                    ArtifactId::from_digest(&icx_core::Blake3Digest::hash(cmd.as_bytes())).0;
                format!("{tool_name}:{}#{cmd_hash:016x}", &cmd[..end])
            }
            None => tool_name.to_owned(),
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

    /// Whether the store already holds exactly the artifact that ingesting `raw_text` would
    /// create, after the same bounding and redaction. Used to skip a copy of an output that a hook
    /// already ingested, so recall does not index it twice.
    pub fn holds_ingested(&self, raw_text: &str) -> bool {
        let (sanitized, _) = Self::bound_and_sanitize_raw(raw_text, icx_core::ICX_MAX_INGEST_BYTES);
        let (id, _) = ArtifactId::hash_bytes(sanitized.as_bytes());
        self.cas.get_meta(id).is_some()
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
        // Storage keeps the redacted text byte-for-byte (ICX-AUD-021); Tier-0 delimiter escaping
        // is applied where text is shown to a model (packets, skeletons, deref output).
        let mut sanitized = SecretRedactor::redact(bounded_raw);
        let is_unredacted = bounded_raw == sanitized;
        let mut was_redacted = raw_text.len() > max_bytes || !is_unredacted;
        if sanitized.len() > max_bytes {
            was_redacted = true;
            let mut bound = max_bytes;
            while !sanitized.is_char_boundary(bound) {
                bound -= 1;
            }
            sanitized.truncate(bound);
        }
        (sanitized, was_redacted)
    }

    fn truncate_agent_suffix(
        map: &mut BTreeMap<usize, IndexedOccurrenceRecord>,
        occurrence_idx: usize,
        agent_u8: u8,
    ) {
        if map
            .last_key_value()
            .is_none_or(|(&k, _)| k < occurrence_idx)
        {
            return;
        }
        let mut tail = map.split_off(&occurrence_idx);
        tail.retain(|_, entry| entry.0 != agent_u8);
        map.append(&mut tail);
    }

    /// Ingests or reuses a message at conversation position `occurrence_idx`. If `occurrence_idx`
    /// was already processed with the exact same `(agent, clean_subject, sanitized_artifact_id)`,
    /// reuses the cached `IngestResult` without duplicating DAG nodes or `/dev/shm` broadcasts.
    /// If `occurrence_idx` is a new message position (even if its payload matches an earlier turn
    /// prior to an intervening state change, e.g. `PASS -> FAIL -> PASS`), executes full ingestion
    /// so `BiTemporalGraph` functional edges and temporal state are updated accurately.
    pub fn ingest_or_reuse_at_index(
        &mut self,
        occurrence_idx: usize,
        agent: AgentKind,
        raw_subject: &str,
        raw_text: &str,
        exit_code: Option<i32>,
        force_anomaly: bool,
    ) -> (IngestResult, bool) {
        let clean_subject = Self::sanitize_hook_label(raw_subject, "event");
        let (sanitized, _) = Self::bound_and_sanitize_raw(raw_text, icx_core::ICX_MAX_INGEST_BYTES);
        let (aid, _) = ArtifactId::hash_bytes(sanitized.as_bytes());
        if let Some((prev_agent, prev_subj, prev_aid, prev_ec, prev_fa, prev_res)) = self
            .agent_occurrences
            .get(&(agent as u8))
            .and_then(|m| m.get(&occurrence_idx))
            && *prev_agent == agent as u8
            && prev_subj == &clean_subject
            && *prev_aid == aid
            && *prev_ec == exit_code
            && *prev_fa == force_anomaly
        {
            return (prev_res.clone(), false);
        }
        if let Some((prev_agent, prev_subj, prev_aid, prev_ec, prev_fa, prev_res)) = self
            .agent_reordered_occurrences
            .get(&(agent as u8))
            .and_then(|m| m.get(&occurrence_idx))
            && *prev_agent == agent as u8
            && prev_subj == &clean_subject
            && *prev_aid == aid
            && *prev_ec == exit_code
            && *prev_fa == force_anomaly
        {
            return (prev_res.clone(), false);
        }
        if let Some((prev_agent, prev_subj, prev_aid, prev_ec, prev_fa, prev_res)) =
            self.indexed_occurrences.get(&occurrence_idx)
        {
            if *prev_agent == agent as u8
                && prev_subj == &clean_subject
                && *prev_aid == aid
                && *prev_ec == exit_code
                && *prev_fa == force_anomaly
            {
                return (prev_res.clone(), false);
            }
        }
        if let Some((prev_agent, prev_subj, prev_aid, prev_ec, prev_fa, prev_res)) =
            self.reordered_occurrences.get(&occurrence_idx)
            && *prev_agent == agent as u8
            && prev_subj == &clean_subject
            && *prev_aid == aid
            && *prev_ec == exit_code
            && *prev_fa == force_anomaly
        {
            return (prev_res.clone(), false);
        }
        let agent_map = self.agent_occurrences.entry(agent as u8).or_default();
        let _ = agent_map.split_off(&occurrence_idx);
        let agent_ro_map = self
            .agent_reordered_occurrences
            .entry(agent as u8)
            .or_default();
        let _ = agent_ro_map.split_off(&occurrence_idx);
        Self::truncate_agent_suffix(&mut self.indexed_occurrences, occurrence_idx, agent as u8);
        Self::truncate_agent_suffix(&mut self.reordered_occurrences, occurrence_idx, agent as u8);
        let res = self.ingest_payload_for_subagent(
            agent,
            raw_subject,
            raw_text,
            exit_code,
            force_anomaly,
            None,
        );
        let record = (
            agent as u8,
            clean_subject,
            aid,
            exit_code,
            force_anomaly,
            res.clone(),
        );
        self.agent_occurrences
            .entry(agent as u8)
            .or_default()
            .insert(occurrence_idx, record.clone());
        if agent == AgentKind::Hermes
            || self
                .indexed_occurrences
                .get(&occurrence_idx)
                .is_none_or(|e| e.0 == agent as u8)
        {
            self.indexed_occurrences.insert(occurrence_idx, record);
        }
        (res, true)
    }

    /// Returns `true` if `occurrence_idx` is already recorded for `agent` in
    /// `self.agent_occurrences` or `self.indexed_occurrences`.
    #[inline]
    pub fn has_indexed_occurrence_for(&self, agent: AgentKind, occurrence_idx: usize) -> bool {
        self.agent_occurrences
            .get(&(agent as u8))
            .is_some_and(|m| m.contains_key(&occurrence_idx))
            || self
                .indexed_occurrences
                .get(&occurrence_idx)
                .is_some_and(|e| e.0 == agent as u8)
    }

    /// Returns `true` if `occurrence_idx` is already recorded for `AgentKind::Hermes` in
    /// `self.agent_occurrences` or `self.indexed_occurrences`.
    #[inline]
    pub fn has_indexed_occurrence(&self, occurrence_idx: usize) -> bool {
        self.has_indexed_occurrence_for(AgentKind::Hermes, occurrence_idx)
    }

    /// Returns the occurrence record for `agent` at `occurrence_idx` matching `sanitized_aid` from
    /// either `self.agent_occurrences` / `self.agent_reordered_occurrences` or
    /// `self.indexed_occurrences` / `self.reordered_occurrences`.
    #[must_use]
    pub fn get_occurrence_record_for(
        &self,
        agent: AgentKind,
        occurrence_idx: usize,
        sanitized_aid: Option<ArtifactId>,
    ) -> Option<IndexedOccurrenceRecord> {
        let agent_idx_opt = self
            .agent_occurrences
            .get(&(agent as u8))
            .and_then(|m| m.get(&occurrence_idx));
        let agent_ro_opt = self
            .agent_reordered_occurrences
            .get(&(agent as u8))
            .and_then(|m| m.get(&occurrence_idx));
        let legacy_idx_opt = self
            .indexed_occurrences
            .get(&occurrence_idx)
            .filter(|e| e.0 == agent as u8);
        let legacy_ro_opt = self
            .reordered_occurrences
            .get(&occurrence_idx)
            .filter(|e| e.0 == agent as u8);
        if let Some(target_aid) = sanitized_aid
            && let Some(matched) = [agent_idx_opt, agent_ro_opt, legacy_idx_opt, legacy_ro_opt]
                .into_iter()
                .flatten()
                .find(|entry| entry.2 == target_aid)
        {
            return Some(matched.clone());
        }
        agent_idx_opt
            .or(agent_ro_opt)
            .or(legacy_idx_opt)
            .or(legacy_ro_opt)
            .cloned()
    }

    /// Returns the occurrence record at `occurrence_idx` matching `sanitized_aid` from either
    /// `self.agent_occurrences` / `self.agent_reordered_occurrences` (preferring
    /// `AgentKind::Hermes`) or `self.indexed_occurrences` / `self.reordered_occurrences`.
    #[must_use]
    pub fn get_occurrence_record(
        &self,
        occurrence_idx: usize,
        sanitized_aid: Option<ArtifactId>,
    ) -> Option<IndexedOccurrenceRecord> {
        self.get_occurrence_record_for(AgentKind::Hermes, occurrence_idx, sanitized_aid)
    }

    /// Records a reordered outbound-view occurrence at `occurrence_idx` without mutating the
    /// canonical `self.indexed_occurrences` table.
    pub fn set_reordered_occurrence(
        &mut self,
        occurrence_idx: usize,
        entry: IndexedOccurrenceRecord,
    ) {
        self.agent_reordered_occurrences
            .entry(entry.0)
            .or_default()
            .insert(occurrence_idx, entry.clone());
        self.reordered_occurrences.insert(occurrence_idx, entry);
    }

    /// Records a carried-forward synthetic/stubbed AHTL frame at `occurrence_idx` so subsequent
    /// message positions remain aligned across compactions without overwriting an existing raw
    /// occurrence entry for the same agent.
    pub fn record_carried_occurrence(
        &mut self,
        occurrence_idx: usize,
        agent: AgentKind,
        clean_subject: String,
        aid: ArtifactId,
        res: IngestResult,
    ) {
        let record = (agent as u8, clean_subject, aid, None, false, res);
        self.agent_occurrences
            .entry(agent as u8)
            .or_default()
            .entry(occurrence_idx)
            .or_insert_with(|| record.clone());
        if self
            .indexed_occurrences
            .get(&occurrence_idx)
            .is_none_or(|e| e.0 != agent as u8)
        {
            self.indexed_occurrences.insert(occurrence_idx, record);
        }
    }

    /// Ingests a tool output or turn if the most recent observation for `clean_subject` in `self`
    /// does not already match `(aid, exit_code, force_anomaly)`; otherwise reuses the existing
    /// `DagNode::Turn` representation. Returns `(IngestResult, was_newly_ingested)`.
    pub fn ingest_or_reuse_payload(
        &mut self,
        agent: AgentKind,
        raw_subject: &str,
        raw_text: &str,
        exit_code: Option<i32>,
        force_anomaly: bool,
    ) -> (IngestResult, bool) {
        let clean_subject = Self::sanitize_hook_label(raw_subject, "event");
        let (sanitized, _) = Self::bound_and_sanitize_raw(raw_text, icx_core::ICX_MAX_INGEST_BYTES);
        let raw_bytes = sanitized.len();
        let (aid, _) = ArtifactId::hash_bytes(sanitized.as_bytes());
        let latest_for_subj_matches = self
            .hdc_events
            .iter()
            .rev()
            .find(|(_, s, _, _)| s == &clean_subject)
            .is_some_and(|(last_aid, _, last_ec, last_fa)| {
                *last_aid == aid && *last_ec == exit_code && *last_fa == force_anomaly
            });
        if latest_for_subj_matches
            && self.cas.get_meta(aid).is_some()
            && let Some((ahtl_repr, surprise)) = self.dag.find_turn(agent, &clean_subject, aid)
        {
            return (
                IngestResult {
                    artifact_id: aid,
                    raw_bytes,
                    compressed_ahtl: ahtl_repr.to_owned(),
                    surprise,
                    was_virtualized: raw_bytes >= self.tool_virtualize_threshold,
                },
                false,
            );
        }
        (
            self.ingest_payload_for_subagent(
                agent,
                raw_subject,
                raw_text,
                exit_code,
                force_anomaly,
                None,
            ),
            true,
        )
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
        self.clock_ms = self.next_clock_ms();
        let clean_subject = Self::sanitize_hook_label(raw_subject, "event");
        let subject = clean_subject.as_str();
        let (sanitized, body_redacted) =
            Self::bound_and_sanitize_raw(raw_text, icx_core::ICX_MAX_INGEST_BYTES);
        let was_redacted = body_redacted || subject != raw_subject.trim();
        let raw_bytes = sanitized.len();
        // ICX-AUD-022: never fall back to the unchecked insert. If the checked insert fails (a
        // 64-bit id collision with different content), the output is not virtualized, so the
        // agent keeps the real bytes instead of a stub that would dereference someone else's.
        let (artifact_id, cas_ok) = match self.cas.put_bytes_checked(sanitized.as_bytes()) {
            Ok((id, _)) => (id, true),
            Err(e) => {
                eprintln!("icx: refusing to virtualize payload: {e}");
                (ArtifactId::hash_bytes(sanitized.as_bytes()).0, false)
            }
        };

        let model_view = SecretRedactor::escape_ahtl_delimiters(&sanitized);
        let mut receipt = self.hdc_index.observe_event_with_exit_code(
            artifact_id,
            subject,
            &model_view,
            exit_code,
            force_anomaly,
        );
        self.hdc_events
            .push((artifact_id, clean_subject.clone(), exit_code, force_anomaly));
        if was_redacted {
            receipt.hologram_bytes[7] |= icx_codec::FLAG_IS_REDACTED;
        }
        if body_redacted {
            self.redacted_artifacts.insert(artifact_id);
        }

        let mut packet = StructuralSkeletonizer::compress_sanitized_to_packet(
            subject,
            &model_view,
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
                saw_fail |= key == "fail" && val.trim() != "0";
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
        // only clean passing test-suite runs (`saw_pass` without failure) supersede `fail -> 0`
        // and `status -> PASS` so unrelated non-test commands cannot erase an active test failure.
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
        } else if saw_pass {
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

        let was_virtualized = cas_ok && raw_bytes >= self.tool_virtualize_threshold;
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
        let raw_slot = self.build_ephemeral_tail_slot_inner(requesting_agent, query, top_k);
        let slot = self.epochs.bound_tail_slot(raw_slot);
        self.record_emitted(&slot);
        slot
    }

    fn build_ephemeral_tail_slot_inner(
        &mut self,
        requesting_agent: AgentKind,
        query: &str,
        top_k: usize,
    ) -> String {
        let limit = self.epochs.tail_slot_byte_limit();
        let header = format!(
            "⟪ICX:TAIL_SLOT│agent={}│epochs={}│cas_artifacts={}⟫",
            requesting_agent.as_str(),
            self.epochs.sealed_epoch_count(),
            self.cas.artifact_count()
        );
        if header.len() > limit {
            return self.epochs.bound_tail_slot(header);
        }
        let mut used = header.len();
        let mut lines = vec![header];

        // Reserve useful recall before consuming space with sibling events. A large public
        // subject can fall back to a complete handle frame instead of dropping all recall.
        let hdc_hits: Vec<AssociativeHit> = self.hdc_index.query_associative(query, top_k);
        if let Some(hit) = hdc_hits.first() {
            Self::append_tail_recall(&mut lines, &mut used, limit, hit);
        }

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
            } else {
                if sibling_taken >= 4 {
                    break;
                }
                let delivered =
                    if ev.ahtl_packet.len().saturating_add(1) <= limit.saturating_sub(used) {
                        Self::append_tail_line(&mut lines, &mut used, limit, ev.ahtl_packet)
                    } else if self.cas.get_meta(ev.artifact_id).is_some() {
                        let expected = format!("⟦Θ:{}@", ev.agent.as_str());
                        let named = ev
                            .ahtl_packet
                            .split_once('⊢')
                            .filter(|(prefix, _)| {
                                prefix.len() <= 256
                                    && prefix.strip_prefix(&expected).is_some_and(|label| {
                                        !label.contains(['⟦', '⟧', '\n', '\r'])
                                    })
                            })
                            .is_some_and(|(prefix, _)| {
                                Self::append_tail_line(
                                    &mut lines,
                                    &mut used,
                                    limit,
                                    format!("{prefix}⊢paged↦{}⟧", ev.artifact_id),
                                )
                            });
                        named
                            || Self::append_tail_line(
                                &mut lines,
                                &mut used,
                                limit,
                                format!(
                                    "⟦Θ:{}@paged⊢seq={}↦{}⟧",
                                    ev.agent.as_str(),
                                    ev.seq,
                                    ev.artifact_id
                                ),
                            )
                    } else {
                        false
                    };
                if !delivered {
                    // Preserve a contiguous cursor. Unsent/unrecoverable events stay queued;
                    // later events must not advance the cursor beyond this blocked event.
                    break;
                }
                new_cursor = new_cursor.max(ev.seq);
                sibling_taken += 1;
            }
        }
        if new_cursor > cursor {
            self.agent_read_cursors
                .insert(requesting_agent as u8, new_cursor);
        }

        // 2. Remaining HDC + Fractal associative hits, only as complete frames.
        for hit in hdc_hits.iter().skip(1) {
            Self::append_tail_recall(&mut lines, &mut used, limit, hit);
        }

        // 3. Bi-Temporal Personalized PageRank multi-hop walk
        let seeds: Vec<&str> = query
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric() && c != '_'))
            .filter(|w| w.len() >= 3 && !Self::is_ppr_stop_word(w))
            .take(4)
            .collect();
        if !seeds.is_empty() {
            let ppr_nodes = self
                .graph
                .personalized_pagerank(&seeds, None, 0.15, 6, top_k);
            for node in ppr_nodes {
                if let Some(&prov) = node.provenance_artifacts.first() {
                    Self::append_tail_line(
                        &mut lines,
                        &mut used,
                        limit,
                        format!(
                            "⟦Ω:PPR_GRAPH⊢entity={}⋮score={:.3}↦{}⟧",
                            node.entity, node.score, prov
                        ),
                    );
                }
            }
        }

        lines.join("\n")
    }

    fn append_tail_line(
        lines: &mut Vec<String>,
        used: &mut usize,
        limit: usize,
        line: String,
    ) -> bool {
        let cost = line.len().saturating_add(1); // separator inserted by join
        if cost <= limit.saturating_sub(*used) {
            *used += cost;
            lines.push(line);
            true
        } else {
            false
        }
    }

    fn append_tail_recall(
        lines: &mut Vec<String>,
        used: &mut usize,
        limit: usize,
        hit: &AssociativeHit,
    ) {
        // A hit that is weak on every signal is noise in the agent's context (v1.3.2: 6% of
        // injected recall lines in the field had sim < 0.1). Exact-identifier (lexical) hits
        // carry a combined score and are kept.
        if hit.hdc_similarity < MIN_RECALL_SIMILARITY
            && hit.shared_fractal_suffix == 0
            && hit.combined_score < MIN_RECALL_SIMILARITY
        {
            return;
        }
        if !Self::append_tail_line(
            lines,
            used,
            limit,
            format!(
                "⟦Ψ:HDC_RECALL⊢subj={}⋮sim={:.2}⋮suffix={}↦{}⟧",
                hit.subject, hit.hdc_similarity, hit.shared_fractal_suffix, hit.artifact_id
            ),
        ) {
            Self::append_tail_line(
                lines,
                used,
                limit,
                format!(
                    "⟦Ψ:HDC_RECALL⊢sim={:.2}⋮suffix={}↦{}⟧",
                    hit.hdc_similarity, hit.shared_fractal_suffix, hit.artifact_id
                ),
            );
        }
    }

    #[inline]
    fn is_ppr_stop_word(w: &str) -> bool {
        const STOP_WORDS: &[&str] = &[
            "what",
            "where",
            "when",
            "which",
            "who",
            "why",
            "how",
            "the",
            "and",
            "for",
            "with",
            "from",
            "that",
            "this",
            "into",
            "failed",
            "broke",
            "check",
            "investigate",
        ];
        STOP_WORDS.iter().any(|&sw| w.eq_ignore_ascii_case(sw))
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
        // An end past the last line is clamped (v1.3.2), for MCP `slice` and `icx deref --lines`
        // alike. A start past the end is still an error that reports the requested range.
        let clamp = |total: usize| {
            if (1..=total).contains(&start_line) {
                end_line.min(total)
            } else {
                end_line
            }
        };
        let end = self
            .cas
            .get_meta(id)
            .map_or(end_line, |m| clamp(m.total_lines));
        match self.cas.deref_lines(id, start_line, end) {
            Ok(text) => Ok(text),
            Err(IcxError::ArtifactNotFound(_)) => {
                // DAG nodes (turns, folds, epochs): one verified reconstruction, sliced here.
                let full = self.dag.deref_node_text(id)?;
                let lines: Vec<&str> = full.lines().collect();
                let total = lines.len();
                let end = clamp(total);
                if start_line == 0 || start_line > end || end > total {
                    return Err(IcxError::InvalidLineRange {
                        start: start_line,
                        end: end_line,
                        total,
                    });
                }
                Ok(lines[start_line - 1..end].join("\n"))
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

    /// Returns `true` if `id` is stored directly in the `L0` Content-Addressed Storage (`CasStore`).
    #[inline]
    pub fn is_cas_artifact(&self, id: ArtifactId) -> bool {
        self.cas.get_meta(id).is_some()
    }

    /// Returns `true` if pre-CAS secret redaction or delimiter escaping modified `id` (or any
    /// descendant `L0` CAS artifact of a `MerkleContextDag` node `id`) during ingestion.
    pub fn is_artifact_redacted(&self, id: ArtifactId) -> bool {
        if self.redacted_artifacts.contains(&id) {
            return true;
        }
        let mut stack = vec![id];
        let mut visited = BTreeSet::new();
        while let Some(cur) = stack.pop() {
            if !visited.insert(cur) {
                continue;
            }
            if self.redacted_artifacts.contains(&cur) {
                return true;
            }
            if let Some(node) = self.dag.get_node(cur) {
                match node {
                    icx_core::dag::DagNode::Turn { raw_artifact, .. } => stack.push(*raw_artifact),
                    icx_core::dag::DagNode::ToolArtifact(tn) => stack.push(tn.raw_artifact),
                    icx_core::dag::DagNode::SubTrajectoryBranch { child_turns, .. } => {
                        stack.extend(child_turns.iter().copied());
                    }
                    icx_core::dag::DagNode::CacheEpoch { child_nodes, .. } => {
                        stack.extend(child_nodes.iter().copied());
                    }
                }
            }
        }
        false
    }

    /// Returns the number of artifacts in `self` that had pre-CAS secret redaction or delimiter
    /// escaping applied.
    #[inline]
    pub fn redacted_artifact_count(&self) -> usize {
        self.redacted_artifacts.len()
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

    /// Forces sealing the active epoch via `CacheEpochController::force_seal_active_epoch` and
    /// synchronizes `global_dag_root` on `self.bus` (using the returned `L3`/`L4` epoch digest).
    pub fn force_rollover_and_sync_dag_root(&mut self) -> Option<ArtifactId> {
        let ep = self.epochs.force_seal_active_epoch(&mut self.dag);
        if let Some(id) = ep
            && let Some(node) = self.dag.get_node(id)
        {
            self.bus.set_global_dag_root(node.digest().0);
        } else {
            self.sync_global_dag_root();
        }
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
            if steps.is_empty()
                // Collision-checked (PR #20 review): on a 64-bit id clash the branch folds
                // without a leaf rather than pointing at another artifact's bytes.
                && let Ok((aid, _)) = self.cas.put_bytes_checked(clean_summary.as_bytes())
            {
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
            let frame = format!("⟪ICX:SUBAGENT_FOLDED│{bid}│{clean_id}│{clean_summary}⟫");
            self.record_emitted(&frame);
            return frame;
        }
        let frame = format!("⟪ICX:SUBAGENT_STOP│{clean_summary}⟫");
        self.record_emitted(&frame);
        frame
    }

    fn map_io_err(err: impl std::fmt::Display) -> IcxError {
        IcxError::Io(err.to_string())
    }

    /// Acquires the shared workspace lock (`cas_store.lock` + in-process `WORKSPACE_IO_LOCK`)
    /// coordinating all workspace readers and writers across `icx-adapters` and `icx-cli`.
    /// Returns `(lock_file, mem_guard)` so the OS file lock drops before the in-process mutex guard.
    pub fn acquire_workspace_lock(
        icx_dir: &Path,
    ) -> Result<(std::fs::File, std::sync::MutexGuard<'static, ()>), IcxError> {
        let mem_guard = WORKSPACE_IO_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        crate::store::ensure_private_dir(icx_dir)?;
        let lock_path = icx_dir.join("cas_store.lock");
        let mut opts = std::fs::OpenOptions::new();
        opts.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let lock_file = opts.open(&lock_path).map_err(Self::map_io_err)?;
        for _ in 0..5_000u32 {
            match lock_file.try_lock() {
                Ok(()) => return Ok((lock_file, mem_guard)),
                Err(std::fs::TryLockError::WouldBlock) => {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                Err(std::fs::TryLockError::Error(e)) => return Err(Self::map_io_err(e)),
            }
        }
        Err(IcxError::Io(format!(
            "timed out acquiring workspace lock `{}`",
            lock_path.display()
        )))
    }

    /// Loads persisted `CasStore`, `BiTemporalGraph`, `MerkleContextDag`, `CacheEpochController`,
    /// `HdcAssociativeIndex`, `redacted_artifacts`, `subagent_branches`, logical `clock_ms`, and
    /// shared-memory `SeqlockTelemetryBus` state from `icx_dir` atomically under an exclusive
    /// workspace lock (only mutating `self` after all components succeed).
    pub fn load_workspace_dir(&mut self, icx_dir: &Path) -> Result<(), IcxError> {
        let _lock = Self::acquire_workspace_lock(icx_dir)?;
        self.load_workspace_dir_unlocked(icx_dir)
    }

    /// Loads workspace state from `icx_dir` assuming the caller already holds
    /// `Self::acquire_workspace_lock(icx_dir)`.
    pub fn load_workspace_dir_unlocked(&mut self, icx_dir: &Path) -> Result<(), IcxError> {
        let canonical = std::fs::canonicalize(icx_dir).unwrap_or_else(|_| icx_dir.to_path_buf());
        if crate::store::exists(icx_dir) {
            let mut conn = crate::store::open(icx_dir)?;
            let current_gen = crate::store::generation(&conn)?;
            if !self.reload_required
                && self
                    .loaded_generation
                    .as_ref()
                    .is_some_and(|(d, g)| *d == canonical && *g == current_gen)
            {
                // No writer has touched the store since this MMU last loaded or saved it; only
                // re-establish the per-directory binding (workspace hash + shared bus file).
                self.workspace_hash = Self::workspace_hash_for(&canonical);
                if self.bus.in_process_layout().is_some() {
                    let shm_path = Self::shm_path_for(icx_dir);
                    if shm_path.exists() {
                        self.bus = SeqlockTelemetryBus::open_shared_file(&shm_path)
                            .map_err(Self::map_io_err)?;
                    }
                }
                return Ok(());
            }
            let stored = crate::store::load(&mut conn)?;
            if let Some(key) = &self.store_seal_key {
                let digest = crate::store::state_digest(
                    stored.chunk_config_raw.as_deref().unwrap_or_default(),
                    stored.graph_json.as_deref().unwrap_or_default(),
                    stored.mmu_state_json.as_deref().unwrap_or_default(),
                    stored.hdc_snapshot.as_deref().unwrap_or_default(),
                    stored.hdc_snapshot_events.unwrap_or(0),
                    stored.artifacts.iter().map(|a| (a.id, a.full_digest)),
                    &stored.emitted,
                );
                let expected =
                    crate::store::store_seal(key, &canonical, stored.generation, &digest);
                let valid = stored
                    .seal
                    .as_deref()
                    .and_then(|s| <[u8; 32]>::try_from(s).ok())
                    // `blake3::Hash` equality is constant-time.
                    .is_some_and(|s| {
                        blake3::Hash::from_bytes(s) == blake3::Hash::from_bytes(expected)
                    });
                if !valid {
                    return Err(IcxError::Io(format!(
                        "workspace store at {} was not written by this icx install (seal mismatch); \
                         run `icx trust --dir {}` if you replaced it yourself",
                        icx_dir.display(),
                        icx_dir.display()
                    )));
                }
            }
            let persisted = crate::store::PersistedIds {
                chunks: stored.chunks.iter().map(|(id, _, _)| *id).collect(),
                artifacts: stored.artifacts.iter().map(|a| a.id).collect(),
                emitted: stored.emitted.clone(),
            };
            let new_cas = CasStore::from_parts(
                stored.chunk_config.unwrap_or_default(),
                stored.chunks,
                stored.artifacts,
            )?;
            let new_graph = match stored.graph_json {
                Some(bytes) => BiTemporalGraph::from_json_bytes(&bytes)?,
                None => BiTemporalGraph::new(),
            };
            let persisted_state: Option<PersistedMmuState> = match stored.mmu_state_json {
                Some(bytes) => Some(serde_json::from_slice(&bytes).map_err(Self::map_io_err)?),
                None => None,
            };
            let snapshot = match (stored.hdc_snapshot, stored.hdc_snapshot_events) {
                (Some(bytes), Some(n)) => Some((
                    HdcAssociativeIndex::decode_snapshot(&bytes).map_err(Self::map_io_err)?,
                    n,
                )),
                _ => None,
            };
            self.apply_loaded(icx_dir, new_cas, new_graph, persisted_state, snapshot)?;
            self.emitted_blocks.extend(stored.emitted);
            self.persisted_ids = persisted;
            self.loaded_generation = Some((canonical, stored.generation));
            self.reload_required = false;
            return Ok(());
        }
        if icx_dir.join("cas_store.json").exists() {
            // v1.0.x JSON workspace: load it, then migrate into the SQLite store. The legacy files
            // are left in place (non-destructive); the store takes precedence from now on.
            self.load_legacy_json_unlocked(icx_dir)?;
            self.save_workspace_dir_unlocked(icx_dir)?;
            return Ok(());
        }
        self.apply_loaded(icx_dir, CasStore::new(), BiTemporalGraph::new(), None, None)?;
        self.persisted_ids = crate::store::PersistedIds::default();
        // No store yet: this state mirrors generation 0, so a store that appears empty (a first
        // save that failed after creating the file) does not trigger a reload over unsaved
        // state (PR #20 review).
        self.loaded_generation = Some((canonical, 0));
        self.reload_required = false;
        Ok(())
    }

    fn workspace_hash_for(canonical_dir: &Path) -> u64 {
        ArtifactId::hash_bytes(canonical_dir.to_string_lossy().as_bytes())
            .0
            .0
    }

    fn shm_path_for(icx_dir: &Path) -> std::path::PathBuf {
        std::env::var("ICX_SHM_PATH")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map_or_else(
                || icx_dir.join("icx-fabric-v1.shm"),
                std::path::PathBuf::from,
            )
    }

    /// Upper bound on remembered emitted-block digests (oldest-first eviction is not needed in
    /// practice: one digest per emitted stub, 32 bytes each).
    pub const MAX_EMITTED_BLOCKS: usize = 1_000_000;

    /// Records that `text` (an icx-formatted block) was handed to an agent, so it can later be
    /// recognized as icx's own output when the agent sends it back in history. Content that
    /// merely *looks* like icx output (starts with `⟦` / `⟪ICX:`) but was never emitted is
    /// treated as untrusted (ICX-AUD-011).
    pub fn record_emitted(&mut self, text: &str) {
        if self.emitted_blocks.len() < Self::MAX_EMITTED_BLOCKS {
            self.emitted_blocks
                .insert(*blake3::hash(text.as_bytes()).as_bytes());
        }
    }

    /// Forgets every emitted-block digest. Used when adopting a store icx did not write: the set
    /// marks text as icx's own output, so it must never be inherited (PR #20 review).
    pub fn forget_emitted_blocks(&mut self) {
        self.emitted_blocks.clear();
    }

    /// Returns `true` only if `text` is byte-identical to a block this MMU emitted.
    pub fn is_emitted(&self, text: &str) -> bool {
        self.emitted_blocks
            .contains(blake3::hash(text.as_bytes()).as_bytes())
    }

    /// Tier-0 escape of icx structural delimiters for untrusted text (`⟪⟫⟦⟧`).
    pub fn escape_untrusted_delimiters(text: &str) -> String {
        SecretRedactor::escape_ahtl_delimiters(text)
    }

    /// Returns `true` if `text` contains icx structural delimiters anywhere but is *not* an
    /// icx-emitted block;
    /// such content must be neutralized (Tier-0 escaped) before it reaches a model.
    pub fn is_forged_icx_block(&self, text: &str) -> bool {
        // Position-independent (security review): any delimiter anywhere counts.
        text.contains(['⟦', '⟧', '⟪', '⟫']) && !self.is_emitted(text)
    }

    /// Uses real wall-clock time for bi-temporal event timestamps (ICX-AUD-048). Off by default so
    /// library use and tests stay deterministic (the core must not read the wall clock unless
    /// asked); the CLI turns it on for real workspaces.
    pub fn use_wall_clock(&mut self, enabled: bool) {
        self.wall_clock = enabled;
    }

    /// Next event timestamp: wall clock (strictly increasing) or logical `+10` counter.
    fn next_clock_ms(&self) -> u64 {
        if self.wall_clock {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
            now.max(self.clock_ms.saturating_add(1))
        } else {
            self.clock_ms.saturating_add(10)
        }
    }

    /// Sets the key used to MAC-stamp and verify the SQLite store (see
    /// `workspace::store_seal_key`). `None` disables store-seal verification (library use).
    pub fn set_store_seal_key(&mut self, key: Option<[u8; 32]>) {
        self.store_seal_key = key;
        // The next load must re-verify under the new key; the mirrored generation stays known so
        // a save can still detect a concurrent writer.
        self.reload_required = true;
    }

    /// The store generation this MMU mirrors (last loaded or saved), if any.
    pub fn mirrored_generation(&self) -> Option<u64> {
        self.loaded_generation.as_ref().map(|(_, g)| *g)
    }

    /// Returns the SQLite store generation of `icx_dir` (0 if never saved).
    pub fn workspace_generation(icx_dir: &Path) -> Result<u64, IcxError> {
        if !crate::store::exists(icx_dir) {
            return Ok(0);
        }
        crate::store::generation(&crate::store::open(icx_dir)?)
    }

    fn load_legacy_json_unlocked(&mut self, icx_dir: &Path) -> Result<(), IcxError> {
        let cas_path = icx_dir.join("cas_store.json");
        let new_cas = if cas_path.exists() {
            CasStore::load_from_dir(icx_dir)?
        } else {
            CasStore::new()
        };
        let graph_path = icx_dir.join("bitemporal_graph.json");
        let new_graph = if graph_path.exists() {
            BiTemporalGraph::load_from_dir(icx_dir)?
        } else {
            BiTemporalGraph::new()
        };
        let state_path = icx_dir.join("mmu_state.json");
        let persisted_state: Option<PersistedMmuState> = if state_path.exists() {
            let bytes = std::fs::read(&state_path).map_err(Self::map_io_err)?;
            Some(serde_json::from_slice(&bytes).map_err(Self::map_io_err)?)
        } else {
            None
        };
        self.apply_loaded(icx_dir, new_cas, new_graph, persisted_state, None)?;
        self.persisted_ids = crate::store::PersistedIds::default();
        self.loaded_generation = None;
        Ok(())
    }

    /// Rebuilds the HDC index by re-encoding each recorded event from CAS. Only used when no
    /// consistent persisted snapshot exists (legacy v1.0.x workspaces, first migration).
    fn replay_hdc_events(&mut self, events: Vec<HdcEventRecord>) {
        for (aid, subj, exit_code, force_anomaly) in events {
            if let Ok(bytes) = self.cas.deref_bytes(aid) {
                let content = String::from_utf8_lossy(&bytes);
                let _ = self.hdc_index.observe_event_with_exit_code(
                    aid,
                    &subj,
                    &content,
                    exit_code,
                    force_anomaly,
                );
                self.hdc_events.push((aid, subj, exit_code, force_anomaly));
            }
        }
    }

    fn apply_loaded(
        &mut self,
        icx_dir: &Path,
        new_cas: CasStore,
        new_graph: BiTemporalGraph,
        persisted_state: Option<PersistedMmuState>,
        hdc_snapshot: Option<(HdcAssociativeIndex, u64)>,
    ) -> Result<(), IcxError> {
        let shm_path = Self::shm_path_for(icx_dir);
        let new_bus = if shm_path.exists() {
            SeqlockTelemetryBus::open_shared_file(&shm_path).map_err(Self::map_io_err)?
        } else {
            SeqlockTelemetryBus::new_in_memory()
        };

        let canonical_ws = std::fs::canonicalize(icx_dir).unwrap_or_else(|_| icx_dir.to_path_buf());
        self.workspace_hash = Self::workspace_hash_for(&canonical_ws);
        self.cas = new_cas;
        self.graph = new_graph;
        self.bus = new_bus;
        self.agent_read_cursors.clear();
        // Trust in emitted blocks belongs to the state being replaced (PR #20 review); loaded
        // digests are added back by the caller or below.
        self.emitted_blocks.clear();
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
        self.hdc_index = HdcAssociativeIndex::new();
        self.hdc_events.clear();
        self.indexed_occurrences.clear();
        self.reordered_occurrences.clear();
        self.agent_occurrences.clear();
        self.agent_reordered_occurrences.clear();
        if let Some(st) = persisted_state {
            self.clock_ms = 1_794_880_800_000_u64.max(st.clock_ms).max(max_graph_ms);
            self.dag = st.dag;
            self.dag.rebuild_turn_index();
            self.epochs = st.epochs.unwrap_or_else(|| {
                CacheEpochController::new(ICX_AHTL_LEGEND, EpochControllerConfig::default())
            });
            match hdc_snapshot {
                Some((index, n))
                    if index.has_exact_lexical_index()
                        && usize::try_from(n).ok() == Some(st.hdc_events.len()) =>
                {
                    // Fast path (ICX-AUD-001): restore the persisted index instead of
                    // re-encoding every stored event.
                    self.hdc_index = index;
                    self.hdc_events = st.hdc_events;
                }
                _ => self.replay_hdc_events(st.hdc_events),
            }
            self.emitted_blocks = st.emitted_blocks;
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
            let bus_seq = self.bus.write_sequence();
            self.agent_read_cursors = st
                .agent_read_cursors
                .into_iter()
                .map(|(k, v)| (k, v.min(bus_seq)))
                .collect();
            self.indexed_occurrences = st
                .indexed_occurrences
                .into_iter()
                .filter(|(_, (_, _, aid, _, _, _))| self.cas.get_meta(*aid).is_some())
                .collect();
            self.reordered_occurrences = st
                .reordered_occurrences
                .into_iter()
                .filter(|(_, (_, _, aid, _, _, _))| self.cas.get_meta(*aid).is_some())
                .collect();
            self.agent_occurrences = st
                .agent_occurrences
                .into_iter()
                .map(|(agent_id, occs)| {
                    let valid = occs
                        .into_iter()
                        .filter(|(_, (_, _, aid, _, _, _))| self.cas.get_meta(*aid).is_some())
                        .collect();
                    (agent_id, valid)
                })
                .collect();
            self.agent_reordered_occurrences = st
                .agent_reordered_occurrences
                .into_iter()
                .map(|(agent_id, occs)| {
                    let valid = occs
                        .into_iter()
                        .filter(|(_, (_, _, aid, _, _, _))| self.cas.get_meta(*aid).is_some())
                        .collect();
                    (agent_id, valid)
                })
                .collect();
        } else {
            self.clock_ms = 1_794_880_800_000_u64.max(max_graph_ms);
            self.dag = MerkleContextDag::new();
            self.epochs =
                CacheEpochController::new(ICX_AHTL_LEGEND, EpochControllerConfig::default());
            self.subagent_branches.clear();
        }
        self.sync_global_dag_root();
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

    /// Atomically persists `CasStore`, `BiTemporalGraph`, `MerkleContextDag`,
    /// `CacheEpochController`, `HdcAssociativeIndex` events, `redacted_artifacts`,
    /// `subagent_branches`, `agent_read_cursors`, `indexed_occurrences`, and logical `clock_ms`
    /// state into `icx_dir` under an exclusive workspace lock.
    pub fn save_workspace_dir(&mut self, icx_dir: &Path) -> Result<(), IcxError> {
        let _lock = Self::acquire_workspace_lock(icx_dir)?;
        self.save_workspace_dir_unlocked(icx_dir)
    }

    /// Atomically persists workspace state into `icx_dir` assuming the caller already holds
    /// `Self::acquire_workspace_lock(icx_dir)`.
    pub fn save_workspace_dir_unlocked(&mut self, icx_dir: &Path) -> Result<(), IcxError> {
        let state_bytes =
            serde_json::to_vec(&self.persisted_state(false)).map_err(Self::map_io_err)?;
        let graph_bytes = self.graph.to_json_bytes()?;
        let hdc_bytes = self.hdc_index.encode_snapshot();
        let mut conn = crate::store::open(icx_dir)?;
        let canonical_for_seal =
            std::fs::canonicalize(icx_dir).unwrap_or_else(|_| icx_dir.to_path_buf());
        let derived = crate::store::DerivedState {
            chunk_config: self.cas.chunk_config(),
            graph_json: &graph_bytes,
            mmu_state_json: &state_bytes,
            hdc_snapshot: &hdc_bytes,
            hdc_snapshot_events: self.hdc_events.len() as u64,
            emitted: &self.emitted_blocks,
            seal: self
                .store_seal_key
                .as_ref()
                .map(|k| (k, canonical_for_seal.as_path())),
        };
        // The persisted-id cache describes the store it was loaded from or last saved to. A
        // different destination has none of those rows, so write everything (PR #20 review).
        if self
            .loaded_generation
            .as_ref()
            .is_none_or(|(path, _)| *path != canonical_for_seal)
        {
            self.persisted_ids = crate::store::PersistedIds::default();
        }
        // The generation this state was loaded from or last saved as; 0 (a new store) when it
        // mirrors no store at this path.
        let expected_generation = self
            .loaded_generation
            .as_ref()
            .filter(|(path, _)| *path == canonical_for_seal)
            .map_or(0, |(_, g)| *g);
        let next_gen = crate::store::save(
            &mut conn,
            self.cas.chunk_entries(),
            self.cas.artifact_entries(),
            &mut self.persisted_ids,
            &derived,
            expected_generation,
        );
        match next_gen {
            Ok(g) => {
                let canonical =
                    std::fs::canonicalize(icx_dir).unwrap_or_else(|_| icx_dir.to_path_buf());
                self.loaded_generation = Some((canonical, g));
                Ok(())
            }
            // The transaction rolled back, so the store still holds the generation this MMU last
            // loaded or saved. Keeping that binding lets the next load take the fast path and keep
            // the unsaved in-memory state for a retry; a load still reloads if another writer has
            // advanced the store (PR #20 review).
            Err(e) => Err(e),
        }
    }

    /// `include_emitted` is `false` for the SQLite store, which persists emitted-block digests in
    /// its own table.
    fn persisted_state(&self, include_emitted: bool) -> PersistedMmuState {
        PersistedMmuState {
            clock_ms: self.clock_ms,
            redacted_ids: self.redacted_artifacts.iter().map(|id| id.0).collect(),
            subagent_branches: self.subagent_branches.clone(),
            dag: self.dag.clone(),
            epochs: Some(self.epochs.clone()),
            hdc_events: self.hdc_events.clone(),
            agent_read_cursors: self.agent_read_cursors.clone(),
            indexed_occurrences: self.indexed_occurrences.clone(),
            reordered_occurrences: self.reordered_occurrences.clone(),
            agent_occurrences: self.agent_occurrences.clone(),
            agent_reordered_occurrences: self.agent_reordered_occurrences.clone(),
            emitted_blocks: if include_emitted {
                self.emitted_blocks.clone()
            } else {
                BTreeSet::new()
            },
        }
    }

    /// Writes the v1.0.x three-file JSON workspace format. Retained only to exercise the
    /// migration path in tests; production saves use the SQLite store.
    #[doc(hidden)]
    pub fn save_workspace_dir_legacy_json(&self, icx_dir: &Path) -> Result<(), IcxError> {
        let _lock = Self::acquire_workspace_lock(icx_dir)?;
        self.cas.save_to_dir(icx_dir)?;
        self.graph.save_to_dir(icx_dir)?;
        let state_bytes =
            serde_json::to_vec(&self.persisted_state(true)).map_err(Self::map_io_err)?;
        Self::write_atomic_file(icx_dir, "mmu_state.json", &state_bytes)
    }

    /// Executes an atomic read-modify-write transaction against `icx_dir` while holding a single
    /// exclusive workspace lock across `load_workspace_dir_unlocked`, `op(self)`, and
    /// `save_workspace_dir_unlocked`.
    pub fn with_locked_workspace<T, E, F>(&mut self, icx_dir: &Path, op: F) -> Result<T, E>
    where
        E: From<IcxError>,
        F: FnOnce(&mut Self) -> Result<T, E>,
    {
        let _lock = Self::acquire_workspace_lock(icx_dir).map_err(E::from)?;
        self.load_workspace_dir_unlocked(icx_dir).map_err(E::from)?;
        let out = op(self)?;
        self.save_workspace_dir_unlocked(icx_dir).map_err(E::from)?;
        Ok(out)
    }
}

#[cfg(test)]
mod recall_floor_tests {
    use super::*;
    use icx_core::ArtifactId;

    fn hit(sim: f32, suffix: u32, combined: f32) -> AssociativeHit {
        AssociativeHit {
            artifact_id: ArtifactId(0x1234),
            subject: "Bash:seq".into(),
            hdc_similarity: sim,
            shared_fractal_suffix: suffix,
            combined_score: combined,
        }
    }

    fn shown(h: &AssociativeHit) -> bool {
        let (mut lines, mut used) = (Vec::new(), 0usize);
        InfinityContextMmu::append_tail_recall(&mut lines, &mut used, 4096, h);
        !lines.is_empty()
    }

    #[test]
    fn weak_recall_is_dropped_and_strong_or_lexical_recall_is_kept() {
        assert!(!shown(&hit(0.02, 0, 0.02)));
        assert!(shown(&hit(0.45, 0, 0.45)));
        // An exact-identifier hit can have low HDC similarity but a high combined score.
        assert!(shown(&hit(0.03, 0, 0.9)));
        // A shared fractal suffix is a signal of its own.
        assert!(shown(&hit(0.03, 2, 0.03)));
    }
}
