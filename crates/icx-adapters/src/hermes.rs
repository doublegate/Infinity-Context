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

    fn extract_line_deref_id(sanitized_line: &str) -> Option<icx_core::ArtifactId> {
        let (_, after) = sanitized_line.rsplit_once("↦#icx:")?;
        let hex_part = after.strip_suffix('⟧').unwrap_or(after);
        format!("#icx:{hex_part}")
            .parse::<icx_core::ArtifactId>()
            .ok()
    }

    fn extract_embedded_ahtl_lines(
        msg: &HermesMessage,
        out: &mut Vec<String>,
        occurrence_idx: &mut usize,
        carried_slots: &mut Vec<(usize, Option<icx_core::ArtifactId>)>,
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
                let parsed_aid = stub_aid
                    .or_else(|| Self::extract_line_deref_id(&sanitized_line))
                    .filter(|&aid| mmu.cas.get_meta(aid).is_some());
                let slot_aid = if mmu.has_indexed_occurrence(*occurrence_idx) {
                    parsed_aid.or_else(|| {
                        Some(icx_core::ArtifactId::hash_bytes(sanitized_line.as_bytes()).0)
                    })
                } else {
                    let line_aid = parsed_aid
                        .unwrap_or_else(|| mmu.cas.put_bytes(sanitized_line.as_bytes()).0);
                    let subj = if stub_aid.is_some() {
                        role_label.as_str()
                    } else {
                        "carried_epoch"
                    };
                    if mmu
                        .dag
                        .find_turn(AgentKind::Hermes, subj, line_aid)
                        .is_none()
                    {
                        mmu.epochs.record_turn(
                            &mut mmu.dag,
                            AgentKind::Hermes,
                            subj,
                            line_aid,
                            sanitized_line.clone(),
                            0.5,
                            (sanitized_line.len() / 4).max(1),
                        );
                    }
                    mmu.record_carried_occurrence(
                        *occurrence_idx,
                        AgentKind::Hermes,
                        subj.to_owned(),
                        line_aid,
                        crate::mmu::IngestResult {
                            artifact_id: line_aid,
                            raw_bytes: sanitized_line.len(),
                            compressed_ahtl: sanitized_line.clone(),
                            surprise: 0.5,
                            was_virtualized: sanitized_line.len() >= mmu.tool_virtualize_threshold,
                        },
                    );
                    Some(line_aid)
                };
                carried_slots.push((*occurrence_idx, slot_aid));
                *occurrence_idx += 1;
                out.push(sanitized_line);
            }
        }
    }

    fn effective_message_text(msg: &HermesMessage, raw_opt: Option<&Value>) -> String {
        let tc_opt = raw_opt
            .and_then(|r| r.get("tool_calls"))
            .filter(|v| v.as_array().is_some_and(|a| !a.is_empty()));
        match (!msg.content.trim().is_empty(), tc_opt) {
            (true, Some(tc)) => format!(
                "tool_calls:{tc} — {}",
                msg.content.lines().collect::<Vec<_>>().join(" ")
            ),
            (true, None) => msg.content.clone(),
            (false, Some(tc)) => format!("tool_calls:{tc}"),
            (false, None) => String::new(),
        }
    }

    /// Indexes or reuses a message in `mmu` to keep `occurrence_idx` aligned across head, middle,
    /// and tail slices without skipping earlier positions.
    fn sync_message_occurrence(
        msg: &HermesMessage,
        raw_opt: Option<&Value>,
        occurrence_idx: &mut usize,
        mmu: &mut InfinityContextMmu,
    ) {
        let eff_text = Self::effective_message_text(msg, raw_opt);
        if Self::is_synthetic_tail_slot(msg) || eff_text.trim().is_empty() {
            return;
        }
        if Self::is_synthetic_cache_epoch(msg) || Self::is_stubbed_message(msg) {
            let mut sink = Vec::new();
            let mut carried_sink = Vec::new();
            Self::extract_embedded_ahtl_lines(
                msg,
                &mut sink,
                occurrence_idx,
                &mut carried_sink,
                mmu,
            );
            return;
        }
        let subj = Self::message_subject(msg);
        if InfinityContextMmu::is_icx_pager_tool(subj) {
            return;
        }
        let _ = mmu.ingest_or_reuse_at_index(
            *occurrence_idx,
            AgentKind::Hermes,
            subj,
            &eff_text,
            None,
            false,
        );
        *occurrence_idx += 1;
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
            let next_is_tool = if idx + 1 < n {
                mid_slice[idx + 1].0.role == "tool" || mid_slice[idx + 1].0.tool_call_id.is_some()
            } else {
                tail_starts_with_tool
            };
            let is_assistant_tool_call = msg.role == "assistant"
                && (raw_opt
                    .and_then(|r| r.get("tool_calls"))
                    .is_some_and(Value::is_array)
                    || next_is_tool);
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

    /// Records `mmu.reordered_occurrences` in `[mid_start_occ, ...)` when folded AHTL lines
    /// and preserved verbatim messages are emitted as `[icx_cache_epoch (folded), preserved_msgs...]`,
    /// resolving each entry from the current input view without mutating the canonical
    /// `mmu.indexed_occurrences` positions.
    fn rebind_reordered_occurrences(
        mmu: &mut InfinityContextMmu,
        mid_start_occ: usize,
        folded_slots: &[(usize, Option<icx_core::ArtifactId>)],
        preserved_slots: &[(usize, Option<icx_core::ArtifactId>)],
    ) {
        if preserved_slots.is_empty() || folded_slots.is_empty() {
            return;
        }
        let resolved: Vec<(usize, crate::mmu::IndexedOccurrenceRecord)> = (mid_start_occ..)
            .zip(folded_slots.iter().chain(preserved_slots.iter()))
            .filter_map(|(new_idx, &(old_idx, aid_opt))| {
                mmu.get_occurrence_record(old_idx, aid_opt)
                    .map(|entry| (new_idx, entry))
            })
            .collect();
        for (new_idx, entry) in resolved {
            mmu.set_reordered_occurrence(new_idx, entry);
        }
    }

    fn process_body_slice<'a>(
        slice: &[(&'a HermesMessage, Option<&'a Value>)],
        preserve_mask: &[bool],
        occurrence_idx: &mut usize,
        mmu: &mut InfinityContextMmu,
    ) -> (Vec<String>, Vec<HermesMessage>, Vec<Value>) {
        let start_occ = *occurrence_idx;
        let mut ahtl_frames = Vec::new();
        let mut folded_slots = Vec::new();
        let mut preserved_slots = Vec::new();
        let mut preserved_msgs = Vec::new();
        let mut preserved_vals = Vec::new();

        for (&(msg, raw_opt), &keep_verbatim) in slice.iter().zip(preserve_mask.iter()) {
            if Self::is_synthetic_cache_epoch(msg)
                || (Self::is_stubbed_message(msg) && !keep_verbatim)
            {
                Self::extract_embedded_ahtl_lines(
                    msg,
                    &mut ahtl_frames,
                    occurrence_idx,
                    &mut folded_slots,
                    mmu,
                );
                continue;
            }
            if keep_verbatim {
                if Self::is_stubbed_message(msg) {
                    let mut stub_sink = Vec::new();
                    Self::extract_embedded_ahtl_lines(
                        msg,
                        &mut stub_sink,
                        occurrence_idx,
                        &mut preserved_slots,
                        mmu,
                    );
                } else {
                    let subj = Self::message_subject(msg);
                    let eff_text = Self::effective_message_text(msg, raw_opt);
                    if !InfinityContextMmu::is_icx_pager_tool(subj) && !eff_text.trim().is_empty() {
                        let (res, _) = mmu.ingest_or_reuse_at_index(
                            *occurrence_idx,
                            AgentKind::Hermes,
                            subj,
                            &eff_text,
                            None,
                            false,
                        );
                        preserved_slots.push((*occurrence_idx, Some(res.artifact_id)));
                        *occurrence_idx += 1;
                    }
                }
                preserved_msgs.push(msg.clone());
                if let Some(r) = raw_opt {
                    preserved_vals.push(r.clone());
                }
                continue;
            }
            let eff_text = Self::effective_message_text(msg, raw_opt);
            if eff_text.trim().is_empty() {
                continue;
            }
            let subj = Self::message_subject(msg);
            let (res, _) = mmu.ingest_or_reuse_at_index(
                *occurrence_idx,
                AgentKind::Hermes,
                subj,
                &eff_text,
                None,
                false,
            );
            folded_slots.push((*occurrence_idx, Some(res.artifact_id)));
            *occurrence_idx += 1;
            ahtl_frames.push(res.compressed_ahtl);
        }
        Self::rebind_reordered_occurrences(mmu, start_occ, &folded_slots, &preserved_slots);
        (ahtl_frames, preserved_msgs, preserved_vals)
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

        let tail_start = total - protect_last_n;
        let head_in_assistant_group = cleaned[..protect_first_n]
            .iter()
            .rev()
            .find(|(m, _)| m.role != "tool" && m.tool_call_id.is_none())
            .is_some_and(|(m, _)| m.role == "assistant");
        let head_end = if head_in_assistant_group {
            protect_first_n
                + cleaned[protect_first_n..tail_start]
                    .iter()
                    .take_while(|(m, _)| m.role == "tool" || m.tool_call_id.is_some())
                    .count()
        } else {
            protect_first_n
        };
        let cap = head_end.saturating_add(protect_last_n).saturating_add(2);
        let mut selected_msgs = Vec::with_capacity(cap);
        let mut selected_vals = Vec::with_capacity(if raw_messages.is_some() { cap } else { 0 });

        let mut occurrence_idx = 0usize;
        for &(msg, raw_opt) in &cleaned[..head_end] {
            Self::sync_message_occurrence(msg, raw_opt, &mut occurrence_idx, mmu);
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
        let (middle_ahtl, preserved_msgs, preserved_vals) =
            Self::process_body_slice(mid_slice, &preserve_mask, &mut occurrence_idx, mmu);

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
            Self::sync_message_occurrence(msg, raw_opt, &mut occurrence_idx, mmu);
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
                    rv["content"] = Value::String(stubbed.content.clone());
                    out_vals.push(rv);
                }
                out.push(stubbed);
                n_pruned += 1;
            } else {
                Self::sync_message_occurrence(msg, raw_opt, &mut occurrence_idx, mmu);
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

        let mut occurrence_idx = 0usize;
        for (i, m) in head_msgs.iter().enumerate() {
            Self::sync_message_occurrence(m, head_vals.get(i), &mut occurrence_idx, mmu);
        }
        let preserve_mask = Self::build_preserve_mask(&body_slice, false);
        let (ahtl_frames, preserved_msgs, preserved_vals) =
            Self::process_body_slice(&body_slice, &preserve_mask, &mut occurrence_idx, mmu);

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

    fn on_turn_complete_core(
        messages: &[HermesMessage],
        raw_messages: Option<&[Value]>,
        mmu: &mut InfinityContextMmu,
    ) -> usize {
        let mut newly_ingested = 0usize;
        let mut occurrence_idx = 0usize;
        let mut carried_sink = Vec::new();
        for (i, msg) in messages.iter().enumerate() {
            let raw_opt = raw_messages.and_then(|r| r.get(i));
            let eff_text = Self::effective_message_text(msg, raw_opt);
            if Self::is_synthetic_tail_slot(msg) || eff_text.trim().is_empty() {
                continue;
            }
            if Self::is_synthetic_cache_epoch(msg) || Self::is_stubbed_message(msg) {
                let mut sink = Vec::new();
                Self::extract_embedded_ahtl_lines(
                    msg,
                    &mut sink,
                    &mut occurrence_idx,
                    &mut carried_sink,
                    mmu,
                );
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
                &eff_text,
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

    /// Implements `ContextEngine.on_turn_complete(messages)`:
    /// Indexes any non-synthetic, non-stubbed, non-pager messages from `messages` that have not
    /// yet been recorded at their conversation position in `mmu`, returning the count of newly
    /// ingested messages.
    pub fn on_turn_complete(messages: &[HermesMessage], mmu: &mut InfinityContextMmu) -> usize {
        Self::on_turn_complete_core(messages, None, mmu)
    }

    fn on_pre_compress_core(
        messages: &[HermesMessage],
        raw_messages: Option<&[Value]>,
        mmu: &mut InfinityContextMmu,
    ) -> String {
        let mut ahtl_frames = Vec::new();
        let mut occurrence_idx = 0usize;
        let mut carried_sink = Vec::new();
        for (i, msg) in messages.iter().enumerate() {
            let raw_opt = raw_messages.and_then(|r| r.get(i));
            let eff_text = Self::effective_message_text(msg, raw_opt);
            if Self::is_synthetic_tail_slot(msg) || eff_text.trim().is_empty() {
                continue;
            }
            if Self::is_synthetic_cache_epoch(msg) || Self::is_stubbed_message(msg) {
                Self::extract_embedded_ahtl_lines(
                    msg,
                    &mut ahtl_frames,
                    &mut occurrence_idx,
                    &mut carried_sink,
                    mmu,
                );
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
                &eff_text,
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

    /// Implements `MemoryProvider.on_pre_compress(messages) -> String`
    /// (`PRE_COMPRESS_CHECKPOINT_API_VERSION = 2` fail-closed pre-compression evidence handoff).
    pub fn on_pre_compress(messages: &[HermesMessage], mmu: &mut InfinityContextMmu) -> String {
        Self::on_pre_compress_core(messages, None, mmu)
    }

    fn parse_messages_with_raw(
        payload: &Value,
    ) -> Result<(Vec<HermesMessage>, Vec<Value>), serde_json::Error> {
        for key in ["messages", "request_messages", "requestMessages"] {
            if let Some(val) = payload.get(key).filter(|v| !v.is_null()) {
                let raw_msgs = serde_json::from_value::<Vec<Value>>(val.clone())?;
                let mut call_id_to_name = std::collections::BTreeMap::new();
                for item in &raw_msgs {
                    if let Some(calls) = item.get("tool_calls").and_then(Value::as_array) {
                        for tc in calls {
                            let id_opt = tc
                                .get("id")
                                .and_then(Value::as_str)
                                .map(str::trim)
                                .filter(|s| !s.is_empty());
                            let name_opt = tc
                                .get("function")
                                .and_then(|f| f.get("name"))
                                .or_else(|| tc.get("name"))
                                .and_then(Value::as_str)
                                .map(str::trim)
                                .filter(|s| !s.is_empty());
                            if let (Some(id), Some(name)) = (id_opt, name_opt) {
                                call_id_to_name.insert(id.to_owned(), name.to_owned());
                            }
                        }
                    }
                }
                let mut msgs = Vec::with_capacity(raw_msgs.len());
                for item in &raw_msgs {
                    let mut m = serde_json::from_value::<HermesMessage>(item.clone())?;
                    if m.name.as_deref().is_none_or(|n| n.trim().is_empty())
                        && let Some(cid) = m.tool_call_id.as_deref()
                        && let Some(resolved_name) = call_id_to_name.get(cid.trim())
                    {
                        m.name = Some(resolved_name.clone());
                    }
                    msgs.push(m);
                }
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
                let (msgs, raw_msgs) = Self::parse_messages_with_raw(&payload)?;
                let count = Self::on_turn_complete_core(&msgs, Some(&raw_msgs), mmu);
                json!({
                    "ingested_count": count
                })
            }
            "on_pre_compress" | "on-pre-compress" => {
                let (msgs, raw_msgs) = Self::parse_messages_with_raw(&payload)?;
                let checkpoint = Self::on_pre_compress_core(&msgs, Some(&raw_msgs), mmu);
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
