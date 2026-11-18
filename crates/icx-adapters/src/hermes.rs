//! Hermes Agent `ContextEngine` (`select_context`, `prune_tool_results_only`, `compress`,
//! `on_turn_complete`) and `MemoryProvider` (`on_pre_compress`) adapter (`HermesAdapter`).

use crate::mmu::InfinityContextMmu;
use crate::{AdapterError, AgentHookAdapter};
use icx_core::AgentKind;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Fail-closed pre-compression evidence handoff checkpoint contract version
/// (`ref-docs/agent-internals/rea-binary-and-sqlite-forensics.md` §2.4).
pub const PRE_COMPRESS_CHECKPOINT_API_VERSION: u32 = 2;

fn deserialize_hermes_content<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let val = Option::<Value>::deserialize(deserializer)?.unwrap_or(Value::Null);
    match val {
        Value::Null => Ok(String::new()),
        Value::String(s) => Ok(s),
        Value::Array(parts) => {
            let mut lines = Vec::with_capacity(parts.len());
            for part in parts {
                match part {
                    Value::String(s) => lines.push(s),
                    Value::Object(ref obj) => {
                        if let Some(txt) = ["text", "content", "output"]
                            .into_iter()
                            .find_map(|k| obj.get(k).and_then(Value::as_str))
                        {
                            lines.push(txt.to_owned());
                        }
                    }
                    Value::Number(_) | Value::Bool(_) => lines.push(part.to_string()),
                    _ => {}
                }
            }
            Ok(lines.join("\n"))
        }
        other => Ok(other.to_string()),
    }
}

/// OpenAI-compatible message entry passed by Hermes's `ContextEngine.select_context()`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HermesMessage {
    pub role: String,
    #[serde(default, deserialize_with = "deserialize_hermes_content")]
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Adapter implementing Hermes's `ContextEngine` and `MemoryProvider` transformations without
/// mutating `state.db`.
#[derive(Clone, Debug, Default)]
pub struct HermesAdapter;

impl AgentHookAdapter for HermesAdapter {
    fn agent_kind(&self) -> AgentKind {
        AgentKind::Hermes
    }

    fn handle_hook_event(
        &self,
        event_name: &str,
        stdin_json: &str,
        mmu: &mut InfinityContextMmu,
    ) -> Result<String, AdapterError> {
        Ok(Self::handle_hook(event_name, stdin_json, mmu)?)
    }
}

impl HermesAdapter {
    fn is_synthetic_tail_slot(msg: &HermesMessage) -> bool {
        msg.name.as_deref() == Some("icx_tail_slot") || msg.content.starts_with("⟪ICX:TAIL_SLOT│")
    }

    fn is_synthetic_cache_epoch(msg: &HermesMessage) -> bool {
        msg.name.as_deref() == Some("icx_cache_epoch")
            || msg.content.starts_with("⟪ICX:HERMES_EPOCH│")
            || msg.content.starts_with("⟪ICX:HERMES_COMPACT│")
    }

    fn is_stubbed_message(msg: &HermesMessage) -> bool {
        msg.content.starts_with("⟪ICX:STUB│")
    }

    fn sanitize_embedded_ahtl_line(trimmed: &str) -> String {
        let bounded = if trimmed.len() > icx_core::ICX_MAX_INGEST_BYTES {
            let mut b = icx_core::ICX_MAX_INGEST_BYTES;
            while !trimmed.is_char_boundary(b) {
                b -= 1;
            }
            &trimmed[..b]
        } else {
            trimmed
        };
        if let Some(inner) = bounded.strip_prefix('⟦').and_then(|s| s.strip_suffix('⟧')) {
            let mut out = String::with_capacity(bounded.len());
            out.push('⟦');
            let mut seg_start = 0usize;
            for (idx, ch) in inner.char_indices() {
                if matches!(ch, '⊢' | '⋮' | '↦' | '│') {
                    out.push_str(&icx_codec::SecretRedactor::sanitize(&inner[seg_start..idx]));
                    out.push(ch);
                    seg_start = idx + ch.len_utf8();
                }
            }
            out.push_str(&icx_codec::SecretRedactor::sanitize(&inner[seg_start..]));
            out.push('⟧');
            out
        } else {
            InfinityContextMmu::bound_and_sanitize_raw(bounded, icx_core::ICX_MAX_INGEST_BYTES).0
        }
    }

    fn extract_embedded_ahtl_lines(
        msg: &HermesMessage,
        out: &mut Vec<String>,
        occurrence_idx: &mut usize,
        mmu: &mut InfinityContextMmu,
    ) {
        for line in msg.content.lines().skip(1) {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                let sanitized_line = Self::sanitize_embedded_ahtl_line(trimmed);
                if !mmu.has_indexed_occurrence(*occurrence_idx) {
                    let (aid, _) = mmu.cas.put_bytes(sanitized_line.as_bytes());
                    if mmu
                        .dag
                        .find_turn(AgentKind::Hermes, "carried_epoch", aid)
                        .is_none()
                    {
                        mmu.epochs.record_turn(
                            &mut mmu.dag,
                            AgentKind::Hermes,
                            "carried_epoch",
                            aid,
                            sanitized_line.clone(),
                            0.5,
                            (sanitized_line.len() / 4).max(1),
                        );
                    }
                    mmu.record_carried_occurrence(
                        *occurrence_idx,
                        AgentKind::Hermes,
                        "carried_epoch".to_owned(),
                        aid,
                        crate::mmu::IngestResult {
                            artifact_id: aid,
                            raw_bytes: sanitized_line.len(),
                            compressed_ahtl: sanitized_line.clone(),
                            surprise: 0.5,
                            was_virtualized: sanitized_line.len() >= mmu.tool_virtualize_threshold,
                        },
                    );
                }
                *occurrence_idx += 1;
                out.push(sanitized_line);
            }
        }
    }

    fn count_occurrence_advance(msg: &HermesMessage) -> usize {
        if Self::is_synthetic_tail_slot(msg) || msg.content.trim().is_empty() {
            0
        } else if Self::is_synthetic_cache_epoch(msg) || Self::is_stubbed_message(msg) {
            msg.content
                .lines()
                .skip(1)
                .filter(|l| !l.trim().is_empty())
                .count()
        } else {
            usize::from(!InfinityContextMmu::is_icx_pager_tool(
                Self::message_subject(msg),
            ))
        }
    }

    fn message_subject(msg: &HermesMessage) -> &str {
        msg.name
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(&msg.role)
    }

    /// Implements `ContextEngine.select_context(request_messages, protect_first_n, protect_last_n)`:
    /// 1. Strips any prior synthetic `icx_tail_slot` message so repeated calls remain idempotent.
    /// 2. Keeps the first `protect_first_n` head messages (system prompt + initial goal) verbatim.
    /// 3. Compresses middle messages into sealed AHTL Epoch frames (reusing existing DAG turns on
    ///    subsequent turns without duplicating DAG nodes or `/dev/shm` broadcasts, and preserving
    ///    `icx` MCP pager outputs verbatim).
    /// 4. Keeps the last `protect_last_n` messages verbatim.
    /// 5. Appends the Ephemeral Tail Slot containing HDC/PPR/Fractal recall and `/dev/shm` telepathy.
    pub fn select_context(
        request_messages: &[HermesMessage],
        protect_first_n: usize,
        protect_last_n: usize,
        mmu: &mut InfinityContextMmu,
    ) -> Vec<HermesMessage> {
        let cleaned: Vec<&HermesMessage> = request_messages
            .iter()
            .filter(|m| !Self::is_synthetic_tail_slot(m))
            .collect();
        let total = cleaned.len();
        if total <= protect_first_n.saturating_add(protect_last_n) {
            let mut out: Vec<HermesMessage> = cleaned.into_iter().cloned().collect();
            let last_query = out
                .last()
                .map(|m| m.content.as_str())
                .unwrap_or("hermes_turn");
            let tail = mmu.build_ephemeral_tail_slot(AgentKind::Hermes, last_query, 3);
            out.push(HermesMessage {
                role: "system".into(),
                content: tail,
                tool_call_id: None,
                name: Some("icx_tail_slot".into()),
            });
            return out;
        }

        let head_end = protect_first_n;
        let tail_start = total - protect_last_n;

        let mut selected = Vec::with_capacity(
            protect_first_n
                .saturating_add(protect_last_n)
                .saturating_add(2),
        );
        let mut occurrence_idx = 0usize;
        for &msg in &cleaned[..head_end] {
            occurrence_idx += Self::count_occurrence_advance(msg);
            selected.push(msg.clone());
        }

        let mut middle_ahtl = Vec::new();
        let mut preserved_pager_msgs = Vec::new();
        for &msg in &cleaned[head_end..tail_start] {
            if Self::is_synthetic_cache_epoch(msg) || Self::is_stubbed_message(msg) {
                Self::extract_embedded_ahtl_lines(msg, &mut middle_ahtl, &mut occurrence_idx, mmu);
                continue;
            }
            let subj = Self::message_subject(msg);
            if InfinityContextMmu::is_icx_pager_tool(subj) {
                preserved_pager_msgs.push(msg.clone());
                continue;
            }
            if msg.content.trim().is_empty() {
                continue;
            }
            let (res, _) = mmu.ingest_or_reuse_at_index(
                occurrence_idx,
                AgentKind::Hermes,
                subj,
                &msg.content,
                None,
                false,
            );
            occurrence_idx += 1;
            middle_ahtl.push(res.compressed_ahtl);
        }

        if !middle_ahtl.is_empty() {
            selected.push(HermesMessage {
                role: "system".into(),
                content: format!(
                    "⟪ICX:HERMES_EPOCH│folded_msgs={}⟫\n{}",
                    middle_ahtl.len(),
                    middle_ahtl.join("\n")
                ),
                tool_call_id: None,
                name: Some("icx_cache_epoch".into()),
            });
        }
        selected.extend(preserved_pager_msgs);

        for &msg in &cleaned[tail_start..] {
            selected.push(msg.clone());
        }

        let last_query = cleaned
            .last()
            .map(|m| m.content.as_str())
            .unwrap_or("hermes_turn");
        let tail_slot = mmu.build_ephemeral_tail_slot(AgentKind::Hermes, last_query, 3);
        selected.push(HermesMessage {
            role: "system".into(),
            content: tail_slot,
            tool_call_id: None,
            name: Some("icx_tail_slot".into()),
        });

        selected
    }

    /// Implements `ContextEngine.prune_tool_results_only(messages, protect_last_n)`:
    /// Deterministically trims oversized tool-result messages outside the last `protect_last_n`
    /// messages into `#icx:<16-hex>` AHTL Holographic Stubs while leaving `icx` MCP pager outputs,
    /// already-stubbed tool results, and non-tool messages untouched. Returns
    /// `(messages, n_pruned)`.
    pub fn prune_tool_results_only(
        messages: &[HermesMessage],
        protect_last_n: usize,
        mmu: &mut InfinityContextMmu,
    ) -> (Vec<HermesMessage>, usize) {
        let total = messages.len();
        let cutoff = total.saturating_sub(protect_last_n);
        let mut out = Vec::with_capacity(total);
        let mut n_pruned = 0usize;

        let mut occurrence_idx = 0usize;
        for (idx, msg) in messages.iter().enumerate() {
            let is_tool = msg.role == "tool" || msg.tool_call_id.is_some();
            let subj = Self::message_subject(msg);
            if idx < cutoff
                && is_tool
                && !InfinityContextMmu::is_icx_pager_tool(subj)
                && !Self::is_stubbed_message(msg)
                && msg.content.len() >= mmu.tool_virtualize_threshold
            {
                let (res, _) = mmu.ingest_or_reuse_at_index(
                    occurrence_idx,
                    AgentKind::Hermes,
                    subj,
                    &msg.content,
                    None,
                    false,
                );
                occurrence_idx += 1;
                let mut stubbed = msg.clone();
                stubbed.content = format!(
                    "⟪ICX:STUB│{}│bytes={}│Use `icx` MCP tool (op=deref/slice/grep) to inspect full output⟫\n{}",
                    res.artifact_id, res.raw_bytes, res.compressed_ahtl
                );
                out.push(stubbed);
                n_pruned += 1;
            } else {
                occurrence_idx += Self::count_occurrence_advance(msg);
                out.push(msg.clone());
            }
        }
        (out, n_pruned)
    }

    /// Implements `ContextEngine.compress(messages, focus_topic)`:
    /// Preserves leading system messages, carries forward any existing `icx_cache_epoch` or
    /// stubbed AHTL frames, commits remaining messages into `mmu`, seals the active
    /// `CacheEpochNode` in `MerkleContextDag`, and emits a compacted `icx_cache_epoch` message
    /// followed by any preserved `icx` pager messages and the dynamic `icx_tail_slot`.
    pub fn compress(
        messages: &[HermesMessage],
        focus_topic: Option<&str>,
        mmu: &mut InfinityContextMmu,
    ) -> Vec<HermesMessage> {
        let mut head = Vec::new();
        let mut body_msgs = Vec::new();
        let mut in_head = true;

        for msg in messages {
            if Self::is_synthetic_tail_slot(msg) {
                continue;
            }
            if in_head && msg.role == "system" && !Self::is_synthetic_cache_epoch(msg) {
                head.push(msg.clone());
            } else {
                in_head = false;
                body_msgs.push(msg);
            }
        }

        let mut occurrence_idx: usize = head.iter().map(Self::count_occurrence_advance).sum();
        let mut ahtl_frames = Vec::new();
        let mut preserved_pager_msgs = Vec::new();
        for msg in &body_msgs {
            if Self::is_synthetic_cache_epoch(msg) || Self::is_stubbed_message(msg) {
                Self::extract_embedded_ahtl_lines(msg, &mut ahtl_frames, &mut occurrence_idx, mmu);
                continue;
            }
            let subj = Self::message_subject(msg);
            if InfinityContextMmu::is_icx_pager_tool(subj) {
                preserved_pager_msgs.push((*msg).clone());
                continue;
            }
            if msg.content.trim().is_empty() {
                continue;
            }
            let (res, _) = mmu.ingest_or_reuse_at_index(
                occurrence_idx,
                AgentKind::Hermes,
                subj,
                &msg.content,
                None,
                false,
            );
            occurrence_idx += 1;
            ahtl_frames.push(res.compressed_ahtl);
        }

        let ep_id = mmu.force_rollover_and_sync_dag_root();
        let ep_handle = ep_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "#icx:0000000000000000".to_owned());
        let topic_label = focus_topic
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|t| InfinityContextMmu::sanitize_hook_label(t, "general"))
            .unwrap_or_else(|| "general".to_owned());

        let mut out = head;
        out.push(HermesMessage {
            role: "system".into(),
            content: format!(
                "⟪ICX:HERMES_COMPACT│epoch={ep_handle}│topic={topic_label}│folded_msgs={}⟫\n{}",
                ahtl_frames.len(),
                ahtl_frames.join("\n")
            ),
            tool_call_id: None,
            name: Some("icx_cache_epoch".into()),
        });
        out.extend(preserved_pager_msgs);

        let query = focus_topic
            .filter(|s| !s.trim().is_empty())
            .or_else(|| body_msgs.last().map(|m| m.content.as_str()))
            .unwrap_or("hermes_compact");
        let tail = mmu.build_ephemeral_tail_slot(AgentKind::Hermes, query, 3);
        out.push(HermesMessage {
            role: "system".into(),
            content: tail,
            tool_call_id: None,
            name: Some("icx_tail_slot".into()),
        });

        out
    }

    /// Implements `ContextEngine.on_turn_complete(messages)`:
    /// Indexes any non-synthetic, non-stubbed, non-pager messages from `messages` that have not
    /// yet been recorded at their conversation position in `mmu`, returning the count of newly
    /// ingested messages.
    pub fn on_turn_complete(messages: &[HermesMessage], mmu: &mut InfinityContextMmu) -> usize {
        let mut newly_ingested = 0usize;
        let mut occurrence_idx = 0usize;
        for msg in messages {
            if Self::is_synthetic_tail_slot(msg) || msg.content.trim().is_empty() {
                continue;
            }
            if Self::is_synthetic_cache_epoch(msg) || Self::is_stubbed_message(msg) {
                let mut sink = Vec::new();
                Self::extract_embedded_ahtl_lines(msg, &mut sink, &mut occurrence_idx, mmu);
                continue;
            }
            let subj = Self::message_subject(msg);
            if InfinityContextMmu::is_icx_pager_tool(subj) {
                continue;
            }
            let (_, was_new) = mmu.ingest_or_reuse_at_index(
                occurrence_idx,
                AgentKind::Hermes,
                subj,
                &msg.content,
                None,
                false,
            );
            occurrence_idx += 1;
            if was_new {
                newly_ingested += 1;
            }
        }
        newly_ingested
    }

    /// Implements `MemoryProvider.on_pre_compress(messages) -> String`
    /// (`PRE_COMPRESS_CHECKPOINT_API_VERSION = 2` fail-closed pre-compression evidence handoff).
    pub fn on_pre_compress(messages: &[HermesMessage], mmu: &mut InfinityContextMmu) -> String {
        let mut ahtl_frames = Vec::new();
        let mut occurrence_idx = 0usize;
        for msg in messages {
            if Self::is_synthetic_tail_slot(msg) || msg.content.trim().is_empty() {
                continue;
            }
            if Self::is_synthetic_cache_epoch(msg) || Self::is_stubbed_message(msg) {
                Self::extract_embedded_ahtl_lines(msg, &mut ahtl_frames, &mut occurrence_idx, mmu);
                continue;
            }
            let subj = Self::message_subject(msg);
            if InfinityContextMmu::is_icx_pager_tool(subj) {
                continue;
            }
            let (res, _) = mmu.ingest_or_reuse_at_index(
                occurrence_idx,
                AgentKind::Hermes,
                subj,
                &msg.content,
                None,
                false,
            );
            occurrence_idx += 1;
            ahtl_frames.push(res.compressed_ahtl);
        }
        let ep_id = mmu.force_rollover_and_sync_dag_root();
        let ep_handle = ep_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "#icx:0000000000000000".to_owned());
        format!(
            "⟪ICX:PRE_COMPRESS_CHECKPOINT│v={}│epoch={ep_handle}│msgs={}│cas_artifacts={}⟫\n{}",
            PRE_COMPRESS_CHECKPOINT_API_VERSION,
            ahtl_frames.len(),
            mmu.cas.artifact_count(),
            ahtl_frames.join("\n")
        )
    }

    fn has_structured_non_text_content(raw_msg: &Value) -> bool {
        match raw_msg.get("content") {
            Some(Value::Array(parts)) => parts.iter().any(|p| {
                p.as_object().is_some_and(|obj| {
                    ["text", "content", "output"]
                        .into_iter()
                        .all(|k| !obj.get(k).is_some_and(Value::is_string))
                })
            }),
            _ => false,
        }
    }

    fn parse_messages_with_raw(
        payload: &Value,
    ) -> Result<(Vec<HermesMessage>, Vec<Value>), serde_json::Error> {
        for key in ["messages", "request_messages", "requestMessages"] {
            if let Some(val) = payload.get(key).filter(|v| !v.is_null()) {
                let raw_msgs = serde_json::from_value::<Vec<Value>>(val.clone())?;
                let msgs = serde_json::from_value::<Vec<HermesMessage>>(val.clone())?;
                return Ok((msgs, raw_msgs));
            }
        }
        Ok((Vec::new(), Vec::new()))
    }

    fn select_context_json(
        msgs: &[HermesMessage],
        raw_msgs: &[Value],
        protect_first_n: usize,
        protect_last_n: usize,
        mmu: &mut InfinityContextMmu,
    ) -> Vec<Value> {
        let cleaned: Vec<(&HermesMessage, &Value)> = msgs
            .iter()
            .zip(raw_msgs.iter())
            .filter(|(m, _)| !Self::is_synthetic_tail_slot(m))
            .collect();
        let total = cleaned.len();
        if total <= protect_first_n.saturating_add(protect_last_n) {
            let mut out: Vec<Value> = cleaned.iter().map(|(_, r)| (*r).clone()).collect();
            let last_query = cleaned
                .last()
                .map(|(m, _)| m.content.as_str())
                .unwrap_or("hermes_turn");
            let tail = mmu.build_ephemeral_tail_slot(AgentKind::Hermes, last_query, 3);
            out.push(json!({
                "role": "system",
                "content": tail,
                "name": "icx_tail_slot"
            }));
            return out;
        }

        let head_end = protect_first_n;
        let tail_start = total - protect_last_n;
        let mut selected = Vec::with_capacity(
            protect_first_n
                .saturating_add(protect_last_n)
                .saturating_add(2),
        );
        let mut occurrence_idx = 0usize;
        for &(msg, raw_val) in &cleaned[..head_end] {
            occurrence_idx += Self::count_occurrence_advance(msg);
            selected.push(raw_val.clone());
        }

        let mut middle_ahtl = Vec::new();
        let mut preserved_mid_vals = Vec::new();
        for &(msg, raw_val) in &cleaned[head_end..tail_start] {
            if Self::is_synthetic_cache_epoch(msg) || Self::is_stubbed_message(msg) {
                Self::extract_embedded_ahtl_lines(msg, &mut middle_ahtl, &mut occurrence_idx, mmu);
                continue;
            }
            let subj = Self::message_subject(msg);
            if InfinityContextMmu::is_icx_pager_tool(subj) {
                preserved_mid_vals.push(raw_val.clone());
                continue;
            }
            if Self::has_structured_non_text_content(raw_val) {
                preserved_mid_vals.push(raw_val.clone());
            }
            if msg.content.trim().is_empty() {
                continue;
            }
            let (res, _) = mmu.ingest_or_reuse_at_index(
                occurrence_idx,
                AgentKind::Hermes,
                subj,
                &msg.content,
                None,
                false,
            );
            occurrence_idx += 1;
            middle_ahtl.push(res.compressed_ahtl);
        }

        if !middle_ahtl.is_empty() {
            selected.push(json!({
                "role": "system",
                "content": format!(
                    "⟪ICX:HERMES_EPOCH│folded_msgs={}⟫\n{}",
                    middle_ahtl.len(),
                    middle_ahtl.join("\n")
                ),
                "name": "icx_cache_epoch"
            }));
        }
        selected.extend(preserved_mid_vals);

        for &(_, raw_val) in &cleaned[tail_start..] {
            selected.push(raw_val.clone());
        }

        let last_query = cleaned
            .last()
            .map(|(m, _)| m.content.as_str())
            .unwrap_or("hermes_turn");
        let tail_slot = mmu.build_ephemeral_tail_slot(AgentKind::Hermes, last_query, 3);
        selected.push(json!({
            "role": "system",
            "content": tail_slot,
            "name": "icx_tail_slot"
        }));
        selected
    }

    fn compress_json(
        msgs: &[HermesMessage],
        raw_msgs: &[Value],
        focus_topic: Option<&str>,
        mmu: &mut InfinityContextMmu,
    ) -> Vec<Value> {
        let mut head_vals = Vec::new();
        let mut head_msgs = Vec::new();
        let mut body_pairs = Vec::new();
        let mut in_head = true;

        for (msg, raw_val) in msgs.iter().zip(raw_msgs.iter()) {
            if Self::is_synthetic_tail_slot(msg) {
                continue;
            }
            if in_head && msg.role == "system" && !Self::is_synthetic_cache_epoch(msg) {
                head_msgs.push(msg);
                head_vals.push(raw_val.clone());
            } else {
                in_head = false;
                body_pairs.push((msg, raw_val));
            }
        }

        let mut occurrence_idx: usize = head_msgs
            .iter()
            .map(|m| Self::count_occurrence_advance(m))
            .sum();
        let mut ahtl_frames = Vec::new();
        let mut preserved_vals = Vec::new();
        for &(msg, raw_val) in &body_pairs {
            if Self::is_synthetic_cache_epoch(msg) || Self::is_stubbed_message(msg) {
                Self::extract_embedded_ahtl_lines(msg, &mut ahtl_frames, &mut occurrence_idx, mmu);
                continue;
            }
            let subj = Self::message_subject(msg);
            if InfinityContextMmu::is_icx_pager_tool(subj) {
                preserved_vals.push(raw_val.clone());
                continue;
            }
            if Self::has_structured_non_text_content(raw_val) {
                preserved_vals.push(raw_val.clone());
            }
            if msg.content.trim().is_empty() {
                continue;
            }
            let (res, _) = mmu.ingest_or_reuse_at_index(
                occurrence_idx,
                AgentKind::Hermes,
                subj,
                &msg.content,
                None,
                false,
            );
            occurrence_idx += 1;
            ahtl_frames.push(res.compressed_ahtl);
        }

        let ep_id = mmu.force_rollover_and_sync_dag_root();
        let ep_handle = ep_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "#icx:0000000000000000".to_owned());
        let topic_label = focus_topic
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|t| InfinityContextMmu::sanitize_hook_label(t, "general"))
            .unwrap_or_else(|| "general".to_owned());

        let mut out = head_vals;
        out.push(json!({
            "role": "system",
            "content": format!(
                "⟪ICX:HERMES_COMPACT│epoch={ep_handle}│topic={topic_label}│folded_msgs={}⟫\n{}",
                ahtl_frames.len(),
                ahtl_frames.join("\n")
            ),
            "name": "icx_cache_epoch"
        }));
        out.extend(preserved_vals);

        let query = focus_topic
            .filter(|s| !s.trim().is_empty())
            .or_else(|| body_pairs.last().map(|(m, _)| m.content.as_str()))
            .unwrap_or("hermes_compact");
        let tail = mmu.build_ephemeral_tail_slot(AgentKind::Hermes, query, 3);
        out.push(json!({
            "role": "system",
            "content": tail,
            "name": "icx_tail_slot"
        }));
        out
    }

    /// Handles a Hermes hook or `ContextEngine` / `MemoryProvider` JSON RPC payload.
    pub fn handle_hook(
        event_name: &str,
        stdin_json: &str,
        mmu: &mut InfinityContextMmu,
    ) -> Result<String, serde_json::Error> {
        let payload: Value = if stdin_json.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(stdin_json)?
        };

        let response = match event_name {
            "select_context" | "select-context" => {
                let (msgs, raw_msgs) = Self::parse_messages_with_raw(&payload)?;
                let first_n = payload
                    .get("protect_first_n")
                    .or_else(|| payload.get("protectFirstN"))
                    .and_then(Value::as_u64)
                    .map(|n| n as usize)
                    .unwrap_or(1);
                let last_n = payload
                    .get("protect_last_n")
                    .or_else(|| payload.get("protectLastN"))
                    .and_then(Value::as_u64)
                    .map(|n| n as usize)
                    .unwrap_or(2);
                let selected = Self::select_context_json(&msgs, &raw_msgs, first_n, last_n, mmu);
                json!({
                    "messages": selected
                })
            }
            "prune_tool_results_only" | "prune-tool-results-only" => {
                let (msgs, raw_msgs) = Self::parse_messages_with_raw(&payload)?;
                let last_n = payload
                    .get("protect_last_n")
                    .or_else(|| payload.get("protectLastN"))
                    .and_then(Value::as_u64)
                    .map(|n| n as usize)
                    .unwrap_or(2);
                let (pruned, n_pruned) = Self::prune_tool_results_only(&msgs, last_n, mmu);
                let out_vals: Vec<Value> = msgs
                    .iter()
                    .zip(pruned)
                    .zip(raw_msgs)
                    .map(|((orig, pr), mut raw_val)| {
                        if pr.content != orig.content
                            && let Some(obj) = raw_val.as_object_mut()
                        {
                            obj.insert("content".into(), Value::String(pr.content));
                        }
                        raw_val
                    })
                    .collect();
                json!({
                    "messages": out_vals,
                    "pruned_count": n_pruned
                })
            }
            "compress" | "PreCompact" | "pre-compact" => {
                let (msgs, raw_msgs) = Self::parse_messages_with_raw(&payload)?;
                let focus_topic = payload
                    .get("focus_topic")
                    .or_else(|| payload.get("focusTopic"))
                    .and_then(Value::as_str);
                let compressed = Self::compress_json(&msgs, &raw_msgs, focus_topic, mmu);
                json!({
                    "messages": compressed
                })
            }
            "on_turn_complete" | "on-turn-complete" | "PostToolUse" | "post-tool-use" => {
                let (msgs, _) = Self::parse_messages_with_raw(&payload)?;
                let count = Self::on_turn_complete(&msgs, mmu);
                json!({
                    "ingested_count": count
                })
            }
            "on_pre_compress" | "on-pre-compress" => {
                let (msgs, _) = Self::parse_messages_with_raw(&payload)?;
                let checkpoint = Self::on_pre_compress(&msgs, mmu);
                json!({
                    "checkpoint_version": PRE_COMPRESS_CHECKPOINT_API_VERSION,
                    "checkpoint": checkpoint
                })
            }
            "SessionStart" | "session-start" => {
                let tail = mmu.build_ephemeral_tail_slot(AgentKind::Hermes, "session_init", 3);
                json!({
                    "systemMessage": tail
                })
            }
            _ => json!({}),
        };

        serde_json::to_string(&response)
    }
}
