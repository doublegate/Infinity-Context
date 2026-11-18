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
        if trimmed.len() <= icx_core::ICX_MAX_INGEST_BYTES
            && let Some(inner) = trimmed.strip_prefix('⟦').and_then(|s| s.strip_suffix('⟧'))
        {
            let mut out = String::with_capacity(trimmed.len());
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
            InfinityContextMmu::bound_and_sanitize_raw(trimmed, icx_core::ICX_MAX_INGEST_BYTES).0
        }
    }

    fn extract_stub_artifact_id(msg: &HermesMessage) -> Option<icx_core::ArtifactId> {
        let first_line = msg.content.lines().next()?;
        let after = first_line.strip_prefix("⟪ICX:STUB│")?;
        let handle_str = after.split('│').next()?;
        handle_str.parse::<icx_core::ArtifactId>().ok()
    }

    fn extract_embedded_ahtl_lines(
        msg: &HermesMessage,
        out: &mut Vec<String>,
        occurrence_idx: &mut usize,
        mmu: &mut InfinityContextMmu,
    ) {
        let stub_aid = if Self::is_stubbed_message(msg) {
            Self::extract_stub_artifact_id(msg).filter(|&aid| mmu.cas.get_meta(aid).is_some())
        } else {
            None
        };
        let role_label =
            InfinityContextMmu::sanitize_hook_label(Self::message_subject(msg), "carried_epoch");
        for line in msg.content.lines().skip(1) {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                let sanitized_line = Self::sanitize_embedded_ahtl_line(trimmed);
                if !mmu.has_indexed_occurrence(*occurrence_idx) {
                    let aid =
                        stub_aid.unwrap_or_else(|| mmu.cas.put_bytes(sanitized_line.as_bytes()).0);
                    let subj = if stub_aid.is_some() {
                        role_label.as_str()
                    } else {
                        "carried_epoch"
                    };
                    if mmu.dag.find_turn(AgentKind::Hermes, subj, aid).is_none() {
                        mmu.epochs.record_turn(
                            &mut mmu.dag,
                            AgentKind::Hermes,
                            subj,
                            aid,
                            sanitized_line.clone(),
                            0.5,
                            (sanitized_line.len() / 4).max(1),
                        );
                    }
                    mmu.record_carried_occurrence(
                        *occurrence_idx,
                        AgentKind::Hermes,
                        subj.to_owned(),
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

    /// Computes a boolean mask of middle-slice messages that must be preserved verbatim:
    /// - Any `icx` MCP pager tool response,
    /// - Any message with structured non-text multimodal parts (`image_url`, etc.), and
    /// - Any `assistant` `tool_calls` turn group (`assistant` + its contiguous `role: "tool"`
    ///   responses) whenever at least one message in that group is preserved or the group
    ///   straddles the boundary into `tail_slice`.
    fn build_preserve_mask(
        mid_slice: &[(&HermesMessage, Option<&Value>)],
        tail_starts_with_tool: bool,
    ) -> Vec<bool> {
        let n = mid_slice.len();
        let mut preserve = vec![false; n];
        for (i, &(msg, raw_opt)) in mid_slice.iter().enumerate() {
            let subj = Self::message_subject(msg);
            if InfinityContextMmu::is_icx_pager_tool(subj)
                || raw_opt.is_some_and(Self::has_structured_non_text_content)
            {
                preserve[i] = true;
            }
        }
        let mut idx = 0usize;
        while idx < n {
            let (msg, raw_opt) = mid_slice[idx];
            let is_assistant_tool_call = msg.role == "assistant"
                && raw_opt
                    .and_then(|r| r.get("tool_calls"))
                    .is_some_and(Value::is_array);
            if is_assistant_tool_call {
                let mut group_end = idx + 1;
                while group_end < n
                    && (mid_slice[group_end].0.role == "tool"
                        || mid_slice[group_end].0.tool_call_id.is_some())
                {
                    group_end += 1;
                }
                let group_straddles_tail = group_end == n && tail_starts_with_tool;
                if group_straddles_tail || preserve[idx..group_end].iter().any(|&p| p) {
                    for p in &mut preserve[idx..group_end] {
                        *p = true;
                    }
                }
                idx = group_end;
            } else {
                idx += 1;
            }
        }
        preserve
    }

    fn select_context_core(
        request_messages: &[HermesMessage],
        raw_messages: Option<&[Value]>,
        protect_first_n: usize,
        protect_last_n: usize,
        mmu: &mut InfinityContextMmu,
    ) -> (Vec<HermesMessage>, Vec<Value>) {
        let cleaned: Vec<(&HermesMessage, Option<&Value>)> = request_messages
            .iter()
            .enumerate()
            .filter(|(_, m)| !Self::is_synthetic_tail_slot(m))
            .map(|(i, m)| (m, raw_messages.and_then(|r| r.get(i))))
            .collect();
        let total = cleaned.len();
        if total <= protect_first_n.saturating_add(protect_last_n) {
            let mut out_msgs = Vec::with_capacity(total + 1);
            let mut out_vals =
                Vec::with_capacity(if raw_messages.is_some() { total + 1 } else { 0 });
            for &(m, r_opt) in &cleaned {
                out_msgs.push(m.clone());
                if let Some(r) = r_opt {
                    out_vals.push(r.clone());
                }
            }
            let last_query = out_msgs
                .last()
                .map(|m| m.content.as_str())
                .unwrap_or("hermes_turn");
            let tail = mmu.build_ephemeral_tail_slot(AgentKind::Hermes, last_query, 3);
            let tail_msg = HermesMessage {
                role: "system".into(),
                content: tail,
                tool_call_id: None,
                name: Some("icx_tail_slot".into()),
            };
            if raw_messages.is_some() {
                out_vals.push(json!({
                    "role": "system",
                    "content": tail_msg.content,
                    "name": "icx_tail_slot"
                }));
            }
            out_msgs.push(tail_msg);
            return (out_msgs, out_vals);
        }

        let head_end = protect_first_n;
        let tail_start = total - protect_last_n;
        let cap = protect_first_n
            .saturating_add(protect_last_n)
            .saturating_add(2);
        let mut selected_msgs = Vec::with_capacity(cap);
        let mut selected_vals = Vec::with_capacity(if raw_messages.is_some() { cap } else { 0 });

        let mut occurrence_idx = 0usize;
        for &(msg, raw_opt) in &cleaned[..head_end] {
            occurrence_idx += Self::count_occurrence_advance(msg);
            selected_msgs.push(msg.clone());
            if let Some(r) = raw_opt {
                selected_vals.push(r.clone());
            }
        }

        let mid_slice = &cleaned[head_end..tail_start];
        let tail_starts_with_tool = cleaned[tail_start..]
            .first()
            .is_some_and(|(m, _)| m.role == "tool" || m.tool_call_id.is_some());
        let preserve_mask = Self::build_preserve_mask(mid_slice, tail_starts_with_tool);

        let mut middle_ahtl = Vec::new();
        let mut preserved_msgs = Vec::new();
        let mut preserved_vals = Vec::new();
        for (&(msg, raw_opt), &keep_verbatim) in mid_slice.iter().zip(preserve_mask.iter()) {
            if Self::is_synthetic_cache_epoch(msg) || Self::is_stubbed_message(msg) {
                Self::extract_embedded_ahtl_lines(msg, &mut middle_ahtl, &mut occurrence_idx, mmu);
                continue;
            }
            if keep_verbatim {
                preserved_msgs.push(msg.clone());
                if let Some(r) = raw_opt {
                    preserved_vals.push(r.clone());
                }
                continue;
            }
            if msg.content.trim().is_empty() {
                continue;
            }
            let subj = Self::message_subject(msg);
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
            let epoch_content = format!(
                "⟪ICX:HERMES_EPOCH│folded_msgs={}⟫\n{}",
                middle_ahtl.len(),
                middle_ahtl.join("\n")
            );
            if raw_messages.is_some() {
                selected_vals.push(json!({
                    "role": "system",
                    "content": epoch_content,
                    "name": "icx_cache_epoch"
                }));
            }
            selected_msgs.push(HermesMessage {
                role: "system".into(),
                content: epoch_content,
                tool_call_id: None,
                name: Some("icx_cache_epoch".into()),
            });
        }
        selected_msgs.extend(preserved_msgs);
        selected_vals.extend(preserved_vals);

        for &(msg, raw_opt) in &cleaned[tail_start..] {
            selected_msgs.push(msg.clone());
            if let Some(r) = raw_opt {
                selected_vals.push(r.clone());
            }
        }

        let last_query = cleaned
            .last()
            .map(|(m, _)| m.content.as_str())
            .unwrap_or("hermes_turn");
        let tail_slot = mmu.build_ephemeral_tail_slot(AgentKind::Hermes, last_query, 3);
        if raw_messages.is_some() {
            selected_vals.push(json!({
                "role": "system",
                "content": tail_slot,
                "name": "icx_tail_slot"
            }));
        }
        selected_msgs.push(HermesMessage {
            role: "system".into(),
            content: tail_slot,
            tool_call_id: None,
            name: Some("icx_tail_slot".into()),
        });

        (selected_msgs, selected_vals)
    }

    /// Implements `ContextEngine.select_context(request_messages, protect_first_n, protect_last_n)`.
    pub fn select_context(
        request_messages: &[HermesMessage],
        protect_first_n: usize,
        protect_last_n: usize,
        mmu: &mut InfinityContextMmu,
    ) -> Vec<HermesMessage> {
        Self::select_context_core(request_messages, None, protect_first_n, protect_last_n, mmu).0
    }

    fn prune_tool_results_only_core(
        messages: &[HermesMessage],
        raw_messages: Option<&[Value]>,
        protect_last_n: usize,
        mmu: &mut InfinityContextMmu,
    ) -> (Vec<HermesMessage>, Vec<Value>, usize) {
        let total = messages.len();
        let cutoff = total.saturating_sub(protect_last_n);
        let mut out = Vec::with_capacity(total);
        let mut out_vals = Vec::with_capacity(if raw_messages.is_some() { total } else { 0 });
        let mut n_pruned = 0usize;

        let mut occurrence_idx = 0usize;
        for (idx, msg) in messages.iter().enumerate() {
            let raw_opt = raw_messages.and_then(|r| r.get(idx));
            let is_tool = msg.role == "tool" || msg.tool_call_id.is_some();
            let subj = Self::message_subject(msg);
            if idx < cutoff
                && is_tool
                && !InfinityContextMmu::is_icx_pager_tool(subj)
                && !raw_opt.is_some_and(Self::has_structured_non_text_content)
                && !Self::is_stubbed_message(msg)
                && !msg.content.trim().is_empty()
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
                if let Some(r) = raw_opt {
                    let mut rv = r.clone();
                    if let Some(obj) = rv.as_object_mut() {
                        obj.insert("content".into(), Value::String(stubbed.content.clone()));
                    }
                    out_vals.push(rv);
                }
                out.push(stubbed);
                n_pruned += 1;
            } else {
                if !raw_opt.is_some_and(Self::has_structured_non_text_content) {
                    occurrence_idx += Self::count_occurrence_advance(msg);
                }
                if let Some(r) = raw_opt {
                    out_vals.push(r.clone());
                }
                out.push(msg.clone());
            }
        }
        (out, out_vals, n_pruned)
    }

    /// Implements `ContextEngine.prune_tool_results_only(messages, protect_last_n)`:
    /// Deterministically trims oversized tool-result messages outside the last `protect_last_n`
    /// messages into `#icx:<16-hex>` AHTL Holographic Stubs while leaving `icx` MCP pager outputs,
    /// multimodal messages with non-text content parts, already-stubbed tool results, and non-tool
    /// messages untouched. Returns `(messages, n_pruned)`.
    pub fn prune_tool_results_only(
        messages: &[HermesMessage],
        protect_last_n: usize,
        mmu: &mut InfinityContextMmu,
    ) -> (Vec<HermesMessage>, usize) {
        let (out, _, n_pruned) =
            Self::prune_tool_results_only_core(messages, None, protect_last_n, mmu);
        (out, n_pruned)
    }

    fn compress_core(
        messages: &[HermesMessage],
        raw_messages: Option<&[Value]>,
        focus_topic: Option<&str>,
        mmu: &mut InfinityContextMmu,
    ) -> (Vec<HermesMessage>, Vec<Value>) {
        let mut head_msgs = Vec::new();
        let mut head_vals = Vec::new();
        let mut body_slice: Vec<(&HermesMessage, Option<&Value>)> = Vec::new();
        let mut in_head = true;

        for (i, msg) in messages.iter().enumerate() {
            if Self::is_synthetic_tail_slot(msg) {
                continue;
            }
            let r_opt = raw_messages.and_then(|r| r.get(i));
            if in_head && msg.role == "system" && !Self::is_synthetic_cache_epoch(msg) {
                head_msgs.push(msg.clone());
                if let Some(r) = r_opt {
                    head_vals.push(r.clone());
                }
            } else {
                in_head = false;
                body_slice.push((msg, r_opt));
            }
        }

        let mut occurrence_idx: usize = head_msgs.iter().map(Self::count_occurrence_advance).sum();
        let preserve_mask = Self::build_preserve_mask(&body_slice, false);
        let mut ahtl_frames = Vec::new();
        let mut preserved_msgs = Vec::new();
        let mut preserved_vals = Vec::new();

        for (&(msg, raw_opt), &keep_verbatim) in body_slice.iter().zip(preserve_mask.iter()) {
            if Self::is_synthetic_cache_epoch(msg) || Self::is_stubbed_message(msg) {
                Self::extract_embedded_ahtl_lines(msg, &mut ahtl_frames, &mut occurrence_idx, mmu);
                continue;
            }
            if keep_verbatim {
                preserved_msgs.push(msg.clone());
                if let Some(r) = raw_opt {
                    preserved_vals.push(r.clone());
                }
                continue;
            }
            if msg.content.trim().is_empty() {
                continue;
            }
            let subj = Self::message_subject(msg);
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
        let compact_content = format!(
            "⟪ICX:HERMES_COMPACT│epoch={ep_handle}│topic={topic_label}│folded_msgs={}⟫\n{}",
            ahtl_frames.len(),
            ahtl_frames.join("\n")
        );

        let mut out_msgs = head_msgs;
        let mut out_vals = head_vals;
        if raw_messages.is_some() {
            out_vals.push(json!({
                "role": "system",
                "content": compact_content,
                "name": "icx_cache_epoch"
            }));
        }
        out_msgs.push(HermesMessage {
            role: "system".into(),
            content: compact_content,
            tool_call_id: None,
            name: Some("icx_cache_epoch".into()),
        });
        out_msgs.extend(preserved_msgs);
        out_vals.extend(preserved_vals);

        let query = focus_topic
            .filter(|s| !s.trim().is_empty())
            .or_else(|| body_slice.last().map(|(m, _)| m.content.as_str()))
            .unwrap_or("hermes_compact");
        let tail = mmu.build_ephemeral_tail_slot(AgentKind::Hermes, query, 3);
        if raw_messages.is_some() {
            out_vals.push(json!({
                "role": "system",
                "content": tail,
                "name": "icx_tail_slot"
            }));
        }
        out_msgs.push(HermesMessage {
            role: "system".into(),
            content: tail,
            tool_call_id: None,
            name: Some("icx_tail_slot".into()),
        });

        (out_msgs, out_vals)
    }

    /// Implements `ContextEngine.compress(messages, focus_topic)`.
    pub fn compress(
        messages: &[HermesMessage],
        focus_topic: Option<&str>,
        mmu: &mut InfinityContextMmu,
    ) -> Vec<HermesMessage> {
        Self::compress_core(messages, None, focus_topic, mmu).0
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
                let (_, selected) =
                    Self::select_context_core(&msgs, Some(&raw_msgs), first_n, last_n, mmu);
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
                let (_, out_vals, n_pruned) =
                    Self::prune_tool_results_only_core(&msgs, Some(&raw_msgs), last_n, mmu);
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
                let (_, compressed) = Self::compress_core(&msgs, Some(&raw_msgs), focus_topic, mmu);
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
