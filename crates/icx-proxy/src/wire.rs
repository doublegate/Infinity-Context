//! Wire-level Cache-Epoch request transformer (`WireProxyTransformer`) for Anthropic
//! (`/v1/messages`), OpenAI (`/v1/responses`, `/v1/chat/completions`), and Gemini
//! (`/v1beta/models/*`) payloads, plus inbound SSE inline `⟪!icx:deref(#icx:...)⟫` expansion and
//! local HTTP/1.1 reverse-proxy request handling.

use icx_adapters::InfinityContextMmu;
use icx_core::AgentKind;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

/// Upstream LLM API provider payload format handled by [`WireProxyTransformer`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderFormat {
    Anthropic,
    OpenAI,
    Gemini,
}

impl ProviderFormat {
    /// Parses a provider format from a CLI flag or HTTP route path.
    #[must_use]
    pub fn parse_provider(raw: &str) -> Option<Self> {
        let lower = raw.trim().to_ascii_lowercase();
        if lower == "anthropic" || lower.starts_with("/anthropic") || lower.contains("/v1/messages")
        {
            Some(Self::Anthropic)
        } else if lower == "openai"
            || lower.starts_with("/openai")
            || lower.contains("/v1/responses")
            || lower.contains("/v1/chat/completions")
        {
            Some(Self::OpenAI)
        } else if lower == "gemini"
            || lower.starts_with("/gemini")
            || lower.contains("/v1beta/models")
        {
            Some(Self::Gemini)
        } else {
            None
        }
    }

    /// Returns the default upstream HTTPS base URL for this provider.
    #[must_use]
    pub const fn default_upstream_url(self) -> &'static str {
        match self {
            Self::Anthropic => "https://api.anthropic.com",
            Self::OpenAI => "https://api.openai.com",
            Self::Gemini => "https://generativelanguage.googleapis.com",
        }
    }
}

#[derive(Default, Clone)]
struct SsePendingEntry {
    pending: String,
    template: Option<Value>,
    json_path: Vec<String>,
}

#[derive(Default, Clone)]
struct SsePendingState {
    streams: std::collections::BTreeMap<String, SsePendingEntry>,
}

impl SsePendingState {
    fn is_empty(&self) -> bool {
        self.streams.values().all(|e| e.pending.is_empty())
    }

    fn clear(&mut self) {
        self.streams.clear();
    }
}

/// Transforms outbound LLM API requests to enforce Prefix-Cache-Locked Epochs (`arXiv:2610.28577`)
/// and inject the Ephemeral Tail Slot without invalidating provider KV caches.
#[derive(Clone, Debug, Default)]
pub struct WireProxyTransformer;

impl WireProxyTransformer {
    /// Transforms an Anthropic `/v1/messages` request JSON body:
    /// 1. Pins `cache_control: {"type": "ephemeral"}` on the sealed Cache Epoch block (managing
    ///    the 4-breakpoint budget so middle history uses at most 1 breakpoint).
    /// 2. Compresses oversized middle-history messages (both string `content` and content-block
    ///    arrays containing `text` or `tool_result` blocks) into deterministic AHTL stubs while
    ///    exempting `icx` pager outputs.
    /// 3. Appends the Ephemeral Tail Slot to the final user turn.
    pub fn transform_anthropic_messages(
        body_json: &str,
        mmu: &mut InfinityContextMmu,
    ) -> Result<String, serde_json::Error> {
        let mut root: Value = serde_json::from_str(body_json)?;
        let protect_tail = 2usize;

        let mut external_breakpoints = Self::count_cache_breakpoints(root.get("system"))
            + Self::count_cache_breakpoints(root.get("tools"));

        if let Some(messages) = root.get_mut("messages").and_then(Value::as_array_mut) {
            let pager_tool_use_ids = Self::collect_anthropic_pager_tool_use_ids(messages);
            let total = messages.len();
            if total > protect_tail {
                let fold_end = total - protect_tail;
                for tail_msg in &messages[fold_end..] {
                    external_breakpoints += Self::count_cache_breakpoints(Some(tail_msg));
                }
                let mut occ_idx = 0usize;
                for (idx, msg) in messages[..fold_end].iter_mut().enumerate() {
                    Self::strip_all_cache_control(msg);
                    let role = msg.get("role").and_then(Value::as_str).unwrap_or("");
                    if role != "system" && role != "developer" {
                        let subj = format!("turn_{idx}");
                        Self::virtualize_anthropic_msg_content(
                            msg,
                            &subj,
                            &pager_tool_use_ids,
                            &mut occ_idx,
                            mmu,
                        );
                    }
                    // Pin Anthropic prompt-cache breakpoint on the last sealed epoch message
                    // only if the 4-breakpoint hardware ceiling has room.
                    if idx + 1 == fold_end && external_breakpoints < 4 {
                        msg["cache_control"] = json!({ "type": "ephemeral" });
                    }
                }
            }

            // Append Ephemeral Tail Slot to the final message (stripping any prior tail slot)
            let last_query = messages
                .last()
                .and_then(Self::extract_item_text)
                .map(Self::strip_existing_tail_slot)
                .filter(|s| !s.is_empty())
                .unwrap_or("anthropic_turn")
                .to_owned();
            let tail_slot = mmu.build_ephemeral_tail_slot(AgentKind::ClaudeCode, &last_query, 3);
            if let Some(last_msg) = messages.last_mut() {
                Self::append_tail_to_item(last_msg, &tail_slot, "text");
            }
        }

        serde_json::to_string(&root)
    }

    fn count_cache_breakpoints(val: Option<&Value>) -> usize {
        let Some(v) = val else { return 0 };
        match v {
            Value::Object(map) => {
                let self_bp = usize::from(map.contains_key("cache_control"));
                let child_bp: usize = map
                    .values()
                    .map(|child| Self::count_cache_breakpoints(Some(child)))
                    .sum();
                self_bp + child_bp
            }
            Value::Array(arr) => arr
                .iter()
                .map(|item| Self::count_cache_breakpoints(Some(item)))
                .sum(),
            _ => 0,
        }
    }

    fn strip_all_cache_control(val: &mut Value) {
        match val {
            Value::Object(obj) => {
                obj.remove("cache_control");
                for child in obj.values_mut() {
                    Self::strip_all_cache_control(child);
                }
            }
            Value::Array(arr) => {
                for item in arr {
                    Self::strip_all_cache_control(item);
                }
            }
            _ => {}
        }
    }

    fn collect_anthropic_pager_tool_use_ids(messages: &[Value]) -> BTreeSet<String> {
        let mut ids = BTreeSet::new();
        for msg in messages {
            if let Some(Value::Array(blocks)) = msg.get("content") {
                for block in blocks {
                    let b_type = block.get("type").and_then(Value::as_str).unwrap_or("");
                    let b_name = block
                        .get("name")
                        .or_else(|| block.get("tool_name"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    if b_type == "tool_use"
                        && InfinityContextMmu::is_icx_pager_tool(b_name)
                        && let Some(id) = block.get("id").and_then(Value::as_str)
                    {
                        ids.insert(id.to_owned());
                    }
                }
            }
        }
        ids
    }

    fn virtualize_anthropic_msg_content(
        msg: &mut Value,
        subj: &str,
        pager_tool_use_ids: &BTreeSet<String>,
        occ_idx: &mut usize,
        mmu: &mut InfinityContextMmu,
    ) {
        let threshold = mmu.tool_virtualize_threshold;
        let Some(content_val) = msg.get_mut("content") else {
            return;
        };
        match content_val {
            Value::String(s) => {
                if s.starts_with('⟦') || s.starts_with("⟪ICX:") {
                    *occ_idx += 1;
                } else if s.len() >= threshold {
                    let (res, _) = mmu.ingest_or_reuse_at_index(
                        *occ_idx,
                        AgentKind::ClaudeCode,
                        subj,
                        s,
                        None,
                        false,
                    );
                    *occ_idx += 1;
                    *s = res.compressed_ahtl;
                }
            }
            Value::Array(blocks) => {
                for (b_idx, block) in blocks.iter_mut().enumerate() {
                    let b_name = block
                        .get("name")
                        .or_else(|| block.get("tool_name"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    let is_pager_id = block
                        .get("tool_use_id")
                        .and_then(Value::as_str)
                        .is_some_and(|id| pager_tool_use_ids.contains(id));
                    if InfinityContextMmu::is_icx_pager_tool(b_name) || is_pager_id {
                        continue;
                    }
                    let block_subj = format!("{subj}_b{b_idx}");
                    if let Some(Value::String(t)) = block.get_mut("text") {
                        if t.starts_with('⟦') || t.starts_with("⟪ICX:") {
                            *occ_idx += 1;
                        } else if t.len() >= threshold {
                            let (res, _) = mmu.ingest_or_reuse_at_index(
                                *occ_idx,
                                AgentKind::ClaudeCode,
                                &block_subj,
                                t,
                                None,
                                false,
                            );
                            *occ_idx += 1;
                            *t = res.compressed_ahtl;
                        }
                    } else if let Some(content_field) = block.get_mut("content") {
                        match content_field {
                            Value::String(c) => {
                                if c.starts_with('⟦') || c.starts_with("⟪ICX:") {
                                    *occ_idx += 1;
                                } else if c.len() >= threshold {
                                    let (res, _) = mmu.ingest_or_reuse_at_index(
                                        *occ_idx,
                                        AgentKind::ClaudeCode,
                                        &block_subj,
                                        c,
                                        None,
                                        false,
                                    );
                                    *occ_idx += 1;
                                    *c = res.compressed_ahtl;
                                }
                            }
                            Value::Array(nested_blocks) => {
                                for (n_idx, n_block) in nested_blocks.iter_mut().enumerate() {
                                    if let Some(Value::String(nt)) = n_block.get_mut("text") {
                                        if nt.starts_with('⟦') || nt.starts_with("⟪ICX:") {
                                            *occ_idx += 1;
                                        } else if nt.len() >= threshold {
                                            let n_subj = format!("{block_subj}_n{n_idx}");
                                            let (res, _) = mmu.ingest_or_reuse_at_index(
                                                *occ_idx,
                                                AgentKind::ClaudeCode,
                                                &n_subj,
                                                nt,
                                                None,
                                                false,
                                            );
                                            *occ_idx += 1;
                                            *nt = res.compressed_ahtl;
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn strip_existing_tail_slot(text: &str) -> &str {
        if let Some(idx) = text.find("\n⟪ICX:TAIL_SLOT│") {
            &text[..idx]
        } else if let Some(idx) = text.find("⟪ICX:TAIL_SLOT│") {
            text[..idx].trim_end_matches('\n')
        } else {
            text
        }
    }

    fn join_base_and_tail(base: &str, tail_slot: &str) -> String {
        if base.is_empty() {
            tail_slot.to_owned()
        } else {
            format!("{base}\n{tail_slot}")
        }
    }

    fn parse_deref_range(spec: &str) -> Option<(usize, usize)> {
        let trimmed = spec
            .trim()
            .strip_prefix('L')
            .or_else(|| spec.trim().strip_prefix('l'))
            .unwrap_or_else(|| spec.trim());
        let (s_str, e_str) = trimmed
            .split_once(':')
            .or_else(|| trimmed.split_once('-'))?;
        let s = s_str.trim().parse::<usize>().ok()?;
        let e = e_str.trim().parse::<usize>().ok()?;
        Some((s, e))
    }

    fn attach_deref_provenance(
        text: String,
        aid: icx_core::ArtifactId,
        mmu: &InfinityContextMmu,
    ) -> String {
        if mmu.is_artifact_redacted(aid) {
            format!(
                "⟪ICX:PROVENANCE│id={aid}│redacted=true│exactness=post_sanitization_cas⟫\n{text}"
            )
        } else {
            text
        }
    }

    /// Transforms an OpenAI `/v1/chat/completions` (`messages`) or `/v1/responses` (`input`) JSON
    /// request body, preserving the 1,024-token automatic prefix cache while virtualizing oversized
    /// middle turns and appending the Ephemeral Tail Slot.
    pub fn transform_openai_request(
        body_json: &str,
        mmu: &mut InfinityContextMmu,
    ) -> Result<String, serde_json::Error> {
        let mut root: Value = serde_json::from_str(body_json)?;
        let protect_tail = 2usize;

        if let Some(messages) = root.get_mut("messages").and_then(Value::as_array_mut) {
            Self::transform_openai_items(messages, mmu, protect_tail, "text");
        } else if let Some(input_val) = root.get_mut("input") {
            match input_val {
                Value::Array(items) => {
                    Self::transform_openai_items(items, mmu, protect_tail, "input_text");
                }
                Value::String(prompt_str) => {
                    let base = Self::strip_existing_tail_slot(prompt_str).to_owned();
                    let tail_slot = mmu.build_ephemeral_tail_slot(AgentKind::Codex, &base, 3);
                    *prompt_str = Self::join_base_and_tail(&base, &tail_slot);
                }
                _ => {}
            }
        }

        serde_json::to_string(&root)
    }

    /// Transforms a Google Gemini `/v1beta/models/*` request JSON body (`contents[].parts[].text`),
    /// virtualizing oversized middle turns into AHTL stubs and appending the Ephemeral Tail Slot
    /// to the final turn.
    pub fn transform_gemini_request(
        body_json: &str,
        mmu: &mut InfinityContextMmu,
    ) -> Result<String, serde_json::Error> {
        let mut root: Value = serde_json::from_str(body_json)?;
        let protect_tail = 2usize;
        let threshold = mmu.tool_virtualize_threshold;

        if let Some(contents) = root.get_mut("contents").and_then(Value::as_array_mut) {
            let total = contents.len();
            if total > protect_tail {
                let fold_end = total - protect_tail;
                let mut occ_idx = 0usize;
                for (idx, content_item) in contents[..fold_end].iter_mut().enumerate() {
                    let role = content_item
                        .get("role")
                        .and_then(Value::as_str)
                        .unwrap_or("user");
                    if role == "system" || role == "developer" {
                        continue;
                    }
                    let item_name = content_item
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    if InfinityContextMmu::is_icx_pager_tool(item_name) {
                        continue;
                    }
                    if let Some(parts) = content_item.get_mut("parts").and_then(Value::as_array_mut)
                    {
                        for (p_idx, part) in parts.iter_mut().enumerate() {
                            let part_name = part
                                .get("functionResponse")
                                .and_then(|fr| fr.get("name"))
                                .or_else(|| part.get("name"))
                                .and_then(Value::as_str)
                                .unwrap_or("");
                            if InfinityContextMmu::is_icx_pager_tool(part_name) {
                                continue;
                            }
                            if let Some(Value::String(t)) = part.get_mut("text") {
                                if t.starts_with('⟦') || t.starts_with("⟪ICX:") {
                                    occ_idx += 1;
                                } else if t.len() >= threshold {
                                    let subj = format!("gemini_turn_{idx}_p{p_idx}");
                                    let (res, _) = mmu.ingest_or_reuse_at_index(
                                        occ_idx,
                                        AgentKind::AntigravityCli,
                                        &subj,
                                        t,
                                        None,
                                        false,
                                    );
                                    occ_idx += 1;
                                    *t = res.compressed_ahtl;
                                }
                            }
                        }
                    }
                }
            }

            let last_query = contents
                .last()
                .and_then(|c| c.get("parts"))
                .and_then(Value::as_array)
                .and_then(|parts| {
                    parts
                        .iter()
                        .rev()
                        .find_map(|p| p.get("text").and_then(Value::as_str))
                })
                .map(Self::strip_existing_tail_slot)
                .filter(|s| !s.is_empty())
                .unwrap_or("gemini_turn")
                .to_owned();
            let tail_slot =
                mmu.build_ephemeral_tail_slot(AgentKind::AntigravityCli, &last_query, 3);
            if let Some(last_content) = contents.last_mut()
                && let Some(parts) = last_content.get_mut("parts").and_then(Value::as_array_mut)
            {
                if let Some(last_text_part) = parts
                    .iter_mut()
                    .rev()
                    .find(|p| p.get("text").is_some_and(Value::is_string))
                    && let Some(Value::String(t)) = last_text_part.get_mut("text")
                {
                    let base = Self::strip_existing_tail_slot(t).to_owned();
                    *t = Self::join_base_and_tail(&base, &tail_slot);
                } else {
                    parts.push(json!({ "text": tail_slot }));
                }
            }
        }

        serde_json::to_string(&root)
    }

    /// Expands any inline `⟪!icx:deref(#icx:<hex>)⟫` or `⟪!icx:deref(#icx:<hex>,start:end)⟫`
    /// directives in `text` against `mmu`.
    #[must_use]
    pub fn expand_inline_deref_directives(text: &str, mmu: &InfinityContextMmu) -> String {
        const PREFIX: &str = "⟪!icx:deref(";
        if !text.contains(PREFIX) {
            return text.to_owned();
        }
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(start_idx) = rest.find(PREFIX) {
            out.push_str(&rest[..start_idx]);
            let after_prefix = &rest[start_idx + PREFIX.len()..];
            if let Some(end_idx) = after_prefix.find(")⟫") {
                let inner = after_prefix[..end_idx].trim();
                let (handle_str, range_opt) = match inner.split_once(',') {
                    Some((h, r)) => (h.trim(), Some(r.trim())),
                    None => (inner, None),
                };
                let expanded = match mmu.resolve_handle(handle_str) {
                    Ok(aid) => match range_opt {
                        Some(r_spec) => match Self::parse_deref_range(r_spec) {
                            Some((s, e)) => mmu
                                .deref_lines(aid, s, e)
                                .map(|t| Self::attach_deref_provenance(t, aid, mmu))
                                .unwrap_or_else(|err| format!("⟪ICX:DEREF_ERR│{err}⟫")),
                            None => format!("⟪ICX:DEREF_ERR│invalid line range `{r_spec}`⟫"),
                        },
                        None => mmu
                            .deref_text(aid)
                            .map(|t| Self::attach_deref_provenance(t, aid, mmu))
                            .unwrap_or_else(|err| format!("⟪ICX:DEREF_ERR│{err}⟫")),
                    },
                    Err(err) => format!("⟪ICX:DEREF_ERR│{err}⟫"),
                };
                out.push_str(&expanded);
                rest = &after_prefix[end_idx + ")⟫".len()..];
            } else {
                out.push_str(&rest[start_idx..]);
                return out;
            }
        }
        out.push_str(rest);
        out
    }

    fn find_incomplete_deref_start(s: &str) -> Option<usize> {
        const PREFIX: &str = "⟪!icx:deref(";
        if let Some(open_idx) = s.rfind(PREFIX) {
            let tail = &s[open_idx..];
            if !tail.contains(")⟫") && tail.len() <= 96 {
                return Some(open_idx);
            }
        }
        for (byte_idx, _) in PREFIX.char_indices().skip(1) {
            let sub = &PREFIX[..byte_idx];
            if s.ends_with(sub) {
                return Some(s.len() - sub.len());
            }
        }
        None
    }

    fn expand_text_with_pending(
        text: &str,
        pending: &mut String,
        mmu: &InfinityContextMmu,
        allow_holdback: bool,
    ) -> String {
        let combined = if pending.is_empty() {
            text.to_owned()
        } else {
            format!("{}{text}", std::mem::take(pending))
        };
        if allow_holdback && let Some(split_idx) = Self::find_incomplete_deref_start(&combined) {
            combined[split_idx..].clone_into(pending);
            Self::expand_inline_deref_directives(&combined[..split_idx], mmu)
        } else {
            Self::expand_inline_deref_directives(&combined, mmu)
        }
    }

    fn extract_stream_index_tag(map: &serde_json::Map<String, Value>) -> String {
        for key in ["index", "output_index", "content_index", "candidateIndex"] {
            if let Some(n) = map.get(key).and_then(Value::as_u64) {
                return format!("[{key}={n}]");
            }
        }
        String::new()
    }

    fn set_json_string_at_path(root: &mut Value, path: &[String], new_val: String) {
        let mut cur = root;
        for seg in path {
            cur = match cur {
                Value::Object(map) => match map.get_mut(seg) {
                    Some(v) => v,
                    None => return,
                },
                Value::Array(arr) => match seg.parse::<usize>().ok().and_then(|i| arr.get_mut(i)) {
                    Some(v) => v,
                    None => return,
                },
                _ => return,
            };
        }
        *cur = Value::String(new_val);
    }

    fn expand_deref_in_json_value_stateful(
        val: &mut Value,
        stream_key_prefix: &str,
        json_path: &mut Vec<String>,
        state: &mut SsePendingState,
        mmu: &InfinityContextMmu,
        allow_holdback: bool,
    ) {
        match val {
            Value::String(s) => {
                let key = if stream_key_prefix.is_empty() {
                    "root".to_owned()
                } else {
                    stream_key_prefix.to_owned()
                };
                let has_pending = state
                    .streams
                    .get(&key)
                    .is_some_and(|e| !e.pending.is_empty());
                if has_pending || s.contains('⟪') {
                    let entry = state.streams.entry(key).or_default();
                    *s = Self::expand_text_with_pending(s, &mut entry.pending, mmu, allow_holdback);
                    entry.template = None;
                    if !entry.pending.is_empty() {
                        entry.json_path.clone_from(json_path);
                    }
                }
            }
            Value::Array(arr) => {
                for (i, item) in arr.iter_mut().enumerate() {
                    let elem_tag = match item.as_object() {
                        Some(m) => {
                            let t = Self::extract_stream_index_tag(m);
                            if t.is_empty() { format!("[{i}]") } else { t }
                        }
                        None => format!("[{i}]"),
                    };
                    let child_key = format!("{stream_key_prefix}{elem_tag}");
                    json_path.push(i.to_string());
                    Self::expand_deref_in_json_value_stateful(
                        item,
                        &child_key,
                        json_path,
                        state,
                        mmu,
                        allow_holdback,
                    );
                    json_path.pop();
                }
            }
            Value::Object(map) => {
                let obj_tag = if stream_key_prefix.is_empty() {
                    Self::extract_stream_index_tag(map)
                } else {
                    String::new()
                };
                for (k, v) in map.iter_mut() {
                    let child_key = format!("{stream_key_prefix}{obj_tag}.{k}");
                    json_path.push(k.clone());
                    if let Value::String(s) = v {
                        let is_text_key =
                            matches!(k.as_str(), "text" | "content" | "delta" | "output");
                        let has_pending = state
                            .streams
                            .get(&child_key)
                            .is_some_and(|e| !e.pending.is_empty());
                        if (is_text_key && has_pending) || s.contains('⟪') {
                            let entry = state.streams.entry(child_key).or_default();
                            *s = Self::expand_text_with_pending(
                                s,
                                &mut entry.pending,
                                mmu,
                                allow_holdback,
                            );
                            entry.template = None;
                            if !entry.pending.is_empty() {
                                entry.json_path.clone_from(json_path);
                            }
                        }
                    } else {
                        Self::expand_deref_in_json_value_stateful(
                            v,
                            &child_key,
                            json_path,
                            state,
                            mmu,
                            allow_holdback,
                        );
                    }
                    json_path.pop();
                }
            }
            _ => {}
        }
    }

    fn transform_sse_line_stateful(
        line: &str,
        state: &mut SsePendingState,
        mmu: &InfinityContextMmu,
        allow_holdback: bool,
    ) -> String {
        if state.is_empty() && !line.contains('⟪') {
            return line.to_owned();
        }
        if let Some(json_part) = line.strip_prefix("data: ") {
            let trimmed = json_part.trim();
            if trimmed == "[DONE]" {
                return line.to_owned();
            }
            if let Ok(mut val) = serde_json::from_str::<Value>(trimmed) {
                let mut json_path = Vec::new();
                Self::expand_deref_in_json_value_stateful(
                    &mut val,
                    "",
                    &mut json_path,
                    state,
                    mmu,
                    allow_holdback,
                );
                for entry in state.streams.values_mut() {
                    if !entry.pending.is_empty() && entry.template.is_none() {
                        entry.template = Some(val.clone());
                    }
                }
                if let Ok(re_serialized) = serde_json::to_string(&val) {
                    return format!("data: {re_serialized}");
                }
            }
            let entry = state.streams.entry("raw_data".to_owned()).or_default();
            let out_text =
                Self::expand_text_with_pending(json_part, &mut entry.pending, mmu, allow_holdback);
            return format!("data: {out_text}");
        }
        if !line.is_empty()
            && !line.starts_with("event:")
            && !line.starts_with("id:")
            && !line.starts_with("retry:")
            && !line.starts_with(':')
        {
            let entry = state.streams.entry("raw_line".to_owned()).or_default();
            return Self::expand_text_with_pending(line, &mut entry.pending, mmu, allow_holdback);
        }
        line.to_owned()
    }

    /// Transforms an inbound Server-Sent Events (`SSE`) chunk, expanding any inline
    /// `⟪!icx:deref(#icx:...)⟫` directives inside `data: {...}` JSON lines or plain text lines
    /// (including directives split across consecutive SSE delta events).
    #[must_use]
    pub fn transform_sse_chunk(sse_chunk: &str, mmu: &InfinityContextMmu) -> String {
        if !sse_chunk.contains('⟪') {
            return sse_chunk.to_owned();
        }
        let lines: Vec<&str> = sse_chunk.split('\n').collect();
        let mut out: Vec<String> = Vec::with_capacity(lines.len());
        let mut state = SsePendingState::default();
        let mut pending_origin_idx: Option<usize> = None;

        for (idx, &line) in lines.iter().enumerate() {
            let was_empty = state.is_empty();
            let transformed = Self::transform_sse_line_stateful(line, &mut state, mmu, true);
            if was_empty && !state.is_empty() {
                pending_origin_idx = Some(idx);
            } else if state.is_empty() {
                pending_origin_idx = None;
            }
            out.push(transformed);
        }

        if !state.is_empty()
            && let Some(origin_idx) = pending_origin_idx
        {
            state.clear();
            for idx in origin_idx..lines.len() {
                out[idx] = Self::transform_sse_line_stateful(lines[idx], &mut state, mmu, false);
            }
        }

        out.join("\n")
    }

    /// Expands any inline `⟪!icx:deref(#icx:...)⟫` directives in a JSON or plain-text response
    /// body, preserving valid JSON string escaping when `body` is a JSON document.
    #[must_use]
    pub fn expand_json_or_text_body(body: &str, mmu: &InfinityContextMmu) -> String {
        if !body.contains("⟪!icx:deref(") {
            return body.to_owned();
        }
        if let Ok(mut val) = serde_json::from_str::<Value>(body) {
            Self::expand_deref_in_json_value(&mut val, mmu);
            if let Ok(re_serialized) = serde_json::to_string(&val) {
                return re_serialized;
            }
        }
        Self::expand_inline_deref_directives(body, mmu)
    }

    fn expand_deref_in_json_value(val: &mut Value, mmu: &InfinityContextMmu) {
        match val {
            Value::String(s) => {
                if s.contains("⟪!icx:deref(") {
                    *s = Self::expand_inline_deref_directives(s, mmu);
                }
            }
            Value::Array(arr) => {
                for item in arr {
                    Self::expand_deref_in_json_value(item, mmu);
                }
            }
            Value::Object(map) => {
                for v in map.values_mut() {
                    Self::expand_deref_in_json_value(v, mmu);
                }
            }
            _ => {}
        }
    }

    /// Resolves the target [`ProviderFormat`] from an explicit override, HTTP route path, or JSON body.
    #[must_use]
    pub fn resolve_provider(
        format: Option<ProviderFormat>,
        route_path: &str,
        body_json: &str,
    ) -> ProviderFormat {
        format
            .or_else(|| ProviderFormat::parse_provider(route_path))
            .unwrap_or_else(|| {
                if let Ok(v) = serde_json::from_str::<Value>(body_json) {
                    if v.get("contents").is_some() {
                        return ProviderFormat::Gemini;
                    }
                    if v.get("input").is_some() {
                        return ProviderFormat::OpenAI;
                    }
                    if let Some(m) = v.get("model").and_then(Value::as_str) {
                        let lower = m.to_ascii_lowercase();
                        if lower.contains("gemini") {
                            return ProviderFormat::Gemini;
                        }
                        if lower.starts_with("gpt-")
                            || lower.starts_with("o1")
                            || lower.starts_with("o3")
                            || lower.starts_with("o4")
                            || lower.contains("codex")
                        {
                            return ProviderFormat::OpenAI;
                        }
                    }
                }
                ProviderFormat::Anthropic
            })
    }

    /// Dispatches a JSON request body according to `format` (or auto-detects from the route/body).
    pub fn transform_for_provider(
        format: Option<ProviderFormat>,
        route_path: &str,
        body_json: &str,
        mmu: &mut InfinityContextMmu,
    ) -> Result<String, serde_json::Error> {
        match Self::resolve_provider(format, route_path, body_json) {
            ProviderFormat::Anthropic => Self::transform_anthropic_messages(body_json, mmu),
            ProviderFormat::OpenAI => Self::transform_openai_request(body_json, mmu),
            ProviderFormat::Gemini => Self::transform_gemini_request(body_json, mmu),
        }
    }

    fn strip_provider_route_prefix(path: &str) -> String {
        let mut out = path;
        for prefix in ["/anthropic/", "/openai/", "/gemini/", "/transform/"] {
            if path.starts_with(prefix) {
                out = &path[prefix.len() - 1..];
                break;
            }
        }
        for exact in ["/anthropic", "/openai", "/gemini", "/transform"] {
            if path == exact {
                return "/".to_owned();
            }
        }
        if out.starts_with('/') {
            out.to_owned()
        } else {
            format!("/{out}")
        }
    }

    fn is_hop_by_hop_or_internal_header(name: &str) -> bool {
        let lower = name.trim().to_ascii_lowercase();
        matches!(
            lower.as_str(),
            "host"
                | "content-length"
                | "connection"
                | "transfer-encoding"
                | "accept-encoding"
                | "x-icx-upstream"
                | "x-icx-transform-only"
        )
    }

    const MAX_REQUEST_BODY_BYTES: usize = 16 * 1024 * 1024;
    const MAX_SSE_LINE_BYTES: usize = 16 * 1024 * 1024;
    const MAX_UPSTREAM_RESPONSE_BYTES: usize = 64 * 1024 * 1024;

    fn read_next_http_chunk<R: BufRead>(
        reader: &mut R,
        max_chunk_bytes: usize,
    ) -> io::Result<Option<Vec<u8>>> {
        let mut size_line = String::new();
        if reader.read_line(&mut size_line)? == 0 {
            return Ok(None);
        }
        let hex_part = size_line.trim().split(';').next().unwrap_or("").trim();
        let chunk_len = usize::from_str_radix(hex_part, 16)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("chunk size: {e}")))?;
        if chunk_len == 0 {
            loop {
                let mut trailer = String::new();
                let n = reader.read_line(&mut trailer)?;
                if n == 0 || trailer == "\r\n" || trailer == "\n" {
                    break;
                }
            }
            return Ok(None);
        }
        if chunk_len > max_chunk_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("HTTP chunk size {chunk_len} exceeds remaining budget {max_chunk_bytes}"),
            ));
        }
        let mut buf = vec![0u8; chunk_len];
        reader.read_exact(&mut buf)?;
        let mut crlf = String::new();
        let _ = reader.read_line(&mut crlf);
        Ok(Some(buf))
    }

    fn is_valid_http_token(s: &str) -> bool {
        !s.is_empty()
            && s.bytes().all(|b| {
                b.is_ascii_alphanumeric()
                    || matches!(
                        b,
                        b'!' | b'#'
                            | b'$'
                            | b'%'
                            | b'&'
                            | b'\''
                            | b'*'
                            | b'+'
                            | b'-'
                            | b'.'
                            | b'^'
                            | b'_'
                            | b'`'
                            | b'|'
                            | b'~'
                    )
            })
    }

    fn flush_sse_pending<W: Write>(
        sse_pending: &mut SsePendingState,
        writer: &mut W,
    ) -> io::Result<()> {
        for entry in sse_pending.streams.values_mut() {
            if !entry.pending.is_empty() {
                let residual = std::mem::take(&mut entry.pending);
                if let Some(mut template) = entry.template.take() {
                    Self::set_json_string_at_path(&mut template, &entry.json_path, residual);
                    if let Ok(serialized) = serde_json::to_string(&template) {
                        write!(writer, "data: {serialized}\n\n")?;
                    }
                } else {
                    write!(writer, "data: {residual}\n\n")?;
                }
                writer.flush()?;
            }
        }
        Ok(())
    }

    fn relay_upstream_response<R: BufRead, W: Write>(
        up_reader: &mut R,
        body_already_dechunked: bool,
        mmu: &InfinityContextMmu,
        writer: &mut W,
    ) -> io::Result<()> {
        let mut status_trimmed;
        let mut resp_headers = Vec::new();
        let mut is_sse;
        let mut is_chunked;

        loop {
            let mut status_line = String::new();
            if up_reader.read_line(&mut status_line)? == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "empty upstream HTTP response",
                ));
            }
            status_trimmed = status_line.trim_end_matches(&['\r', '\n'][..]).to_owned();
            resp_headers.clear();
            is_sse = false;
            is_chunked = false;
            loop {
                let mut hline = String::new();
                let n = up_reader.read_line(&mut hline)?;
                if n == 0 || hline == "\r\n" || hline == "\n" {
                    break;
                }
                if let Some((hk, hv)) = hline.split_once(':') {
                    let hk_trim = hk.trim();
                    let hv_trim = hv.trim();
                    if hk_trim.eq_ignore_ascii_case("content-type")
                        && hv_trim.to_ascii_lowercase().contains("text/event-stream")
                    {
                        is_sse = true;
                    } else if !body_already_dechunked
                        && hk_trim.eq_ignore_ascii_case("transfer-encoding")
                        && hv_trim.to_ascii_lowercase().contains("chunked")
                    {
                        is_chunked = true;
                    }
                    if !Self::is_hop_by_hop_or_internal_header(hk_trim) {
                        resp_headers.push((hk_trim.to_owned(), hv_trim.to_owned()));
                    }
                }
            }
            if !status_trimmed.contains(" 100 ")
                && !status_trimmed
                    .to_ascii_lowercase()
                    .contains("connection established")
            {
                break;
            }
        }

        if is_sse {
            write!(writer, "{status_trimmed}\r\n")?;
            for (hk, hv) in &resp_headers {
                write!(writer, "{hk}: {hv}\r\n")?;
            }
            write!(writer, "Connection: close\r\n\r\n")?;
            writer.flush()?;

            let mut sse_pending = SsePendingState::default();
            if is_chunked {
                let mut pending_bytes: Vec<u8> = Vec::new();
                while let Some(chunk) = Self::read_next_http_chunk(
                    up_reader,
                    Self::MAX_SSE_LINE_BYTES.saturating_sub(pending_bytes.len()),
                )? {
                    pending_bytes.extend_from_slice(&chunk);
                    while let Some(nl_idx) = pending_bytes.iter().position(|&b| b == b'\n') {
                        let line_bytes = &pending_bytes[..nl_idx];
                        let trimmed_bytes = line_bytes.strip_suffix(b"\r").unwrap_or(line_bytes);
                        let raw_line = String::from_utf8_lossy(trimmed_bytes);
                        if raw_line.trim() == "data: [DONE]" {
                            Self::flush_sse_pending(&mut sse_pending, writer)?;
                        }
                        let transformed_line = Self::transform_sse_line_stateful(
                            &raw_line,
                            &mut sse_pending,
                            mmu,
                            true,
                        );
                        writeln!(writer, "{transformed_line}")?;
                        writer.flush()?;
                        pending_bytes.drain(..=nl_idx);
                    }
                }
                if !pending_bytes.is_empty() {
                    let trimmed_bytes = pending_bytes.strip_suffix(b"\r").unwrap_or(&pending_bytes);
                    let raw_line = String::from_utf8_lossy(trimmed_bytes);
                    let transformed_line =
                        Self::transform_sse_line_stateful(&raw_line, &mut sse_pending, mmu, false);
                    write!(writer, "{transformed_line}")?;
                    writer.flush()?;
                }
                Self::flush_sse_pending(&mut sse_pending, writer)?;
            } else {
                let mut line = String::new();
                loop {
                    line.clear();
                    if up_reader
                        .take((Self::MAX_SSE_LINE_BYTES as u64) + 1)
                        .read_line(&mut line)?
                        == 0
                    {
                        break;
                    }
                    if line.len() > Self::MAX_SSE_LINE_BYTES {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "upstream SSE line exceeds 16 MiB limit",
                        ));
                    }
                    let has_nl = line.ends_with('\n');
                    let trimmed = line.trim_end_matches(&['\r', '\n'][..]);
                    if trimmed.trim() == "data: [DONE]" {
                        Self::flush_sse_pending(&mut sse_pending, writer)?;
                    }
                    let transformed_line =
                        Self::transform_sse_line_stateful(trimmed, &mut sse_pending, mmu, has_nl);
                    if has_nl {
                        writeln!(writer, "{transformed_line}")?;
                    } else {
                        write!(writer, "{transformed_line}")?;
                    }
                    writer.flush()?;
                }
                Self::flush_sse_pending(&mut sse_pending, writer)?;
            }
            return Ok(());
        }

        let mut resp_bytes = Vec::new();
        if is_chunked {
            while let Some(chunk) = Self::read_next_http_chunk(
                up_reader,
                Self::MAX_UPSTREAM_RESPONSE_BYTES.saturating_sub(resp_bytes.len()),
            )? {
                resp_bytes.extend_from_slice(&chunk);
            }
        } else {
            up_reader
                .take((Self::MAX_UPSTREAM_RESPONSE_BYTES as u64) + 1)
                .read_to_end(&mut resp_bytes)?;
            if resp_bytes.len() > Self::MAX_UPSTREAM_RESPONSE_BYTES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "upstream HTTP response exceeds 64 MiB limit",
                ));
            }
        }
        let resp_str = String::from_utf8_lossy(&resp_bytes);
        let final_body = Self::expand_json_or_text_body(&resp_str, mmu);

        write!(writer, "{status_trimmed}\r\n")?;
        for (hk, hv) in &resp_headers {
            write!(writer, "{hk}: {hv}\r\n")?;
        }
        write!(
            writer,
            "Content-Length: {}\r\nConnection: close\r\n\r\n{}",
            final_body.len(),
            final_body
        )?;
        writer.flush()
    }

    fn join_upstream_path(base_path: &str, stripped_path: &str) -> String {
        let bp = base_path.trim_end_matches('/');
        if bp.is_empty()
            || stripped_path == bp
            || stripped_path
                .strip_prefix(bp)
                .is_some_and(|rest| rest.starts_with('/'))
        {
            stripped_path.to_owned()
        } else {
            format!("{bp}{stripped_path}")
        }
    }

    fn parse_http_upstream_endpoint(
        upstream_base: &str,
        stripped_path: &str,
    ) -> (String, String, String) {
        let without_scheme = upstream_base
            .trim()
            .strip_prefix("http://")
            .unwrap_or_else(|| upstream_base.trim());
        let (authority, base_path) = match without_scheme.find('/') {
            Some(idx) => (
                &without_scheme[..idx],
                without_scheme[idx..].trim_end_matches('/'),
            ),
            None => (without_scheme, ""),
        };
        let has_port = if authority.starts_with('[') {
            authority.contains("]:")
        } else {
            authority.contains(':')
        };
        let connect_addr = if has_port {
            authority.to_owned()
        } else {
            format!("{authority}:80")
        };
        let target_path = Self::join_upstream_path(base_path, stripped_path);
        (connect_addr, authority.to_owned(), target_path)
    }

    fn join_upstream_base_and_route(upstream_base: &str, stripped_path: &str) -> String {
        let trimmed = upstream_base.trim().trim_end_matches('/');
        let after_scheme_idx = trimmed.find("://").map_or(0, |idx| idx + 3);
        let (origin, base_path) = match trimmed[after_scheme_idx..].find('/') {
            Some(rel_idx) => {
                let split_idx = after_scheme_idx + rel_idx;
                (&trimmed[..split_idx], &trimmed[split_idx..])
            }
            None => (trimmed, ""),
        };
        format!(
            "{origin}{}",
            Self::join_upstream_path(base_path, stripped_path)
        )
    }

    fn forward_http_upstream<W: Write>(
        method: &str,
        upstream_base: &str,
        stripped_path: &str,
        client_headers: &[(String, String)],
        transformed_body: &str,
        mmu: &InfinityContextMmu,
        writer: &mut W,
    ) -> io::Result<()> {
        let (connect_addr, host_header, target_path) =
            Self::parse_http_upstream_endpoint(upstream_base, stripped_path);

        let mut upstream = TcpStream::connect(&connect_addr)?;
        let _ = upstream.set_read_timeout(Some(Duration::from_secs(30)));
        let _ = upstream.set_write_timeout(Some(Duration::from_secs(30)));

        let safe_method = if Self::is_valid_http_token(method) {
            method
        } else {
            "POST"
        };
        write!(
            upstream,
            "{safe_method} {target_path} HTTP/1.1\r\nHost: {host_header}\r\nAccept-Encoding: identity\r\n"
        )?;
        let mut has_content_type = false;
        for (k, v) in client_headers {
            if Self::is_hop_by_hop_or_internal_header(k)
                || !Self::is_valid_http_token(k)
                || v.contains(['\r', '\n', '\0'])
            {
                continue;
            }
            if k.eq_ignore_ascii_case("content-type") {
                has_content_type = true;
            }
            write!(upstream, "{k}: {v}\r\n")?;
        }
        if !has_content_type {
            write!(upstream, "Content-Type: application/json\r\n")?;
        }
        write!(
            upstream,
            "Content-Length: {}\r\nConnection: close\r\n\r\n{}",
            transformed_body.len(),
            transformed_body
        )?;
        upstream.flush()?;

        let mut up_reader = BufReader::new(upstream);
        Self::relay_upstream_response(&mut up_reader, false, mmu, writer)
    }

    fn is_loopback_upstream(url: &str) -> bool {
        let trimmed = url.trim();
        if trimmed.contains('@')
            || trimmed.contains('\\')
            || trimmed.chars().any(|c| c.is_whitespace() || c.is_control())
        {
            return false;
        }
        let without_scheme = trimmed
            .strip_prefix("http://")
            .or_else(|| trimmed.strip_prefix("https://"))
            .unwrap_or(trimmed);
        let host_port = without_scheme.split('/').next().unwrap_or("");
        for (prefix, exact) in [
            ("127.0.0.1:", "127.0.0.1"),
            ("localhost:", "localhost"),
            ("[::1]:", "[::1]"),
        ] {
            if host_port == exact {
                return true;
            }
            if let Some(port_str) = host_port.strip_prefix(prefix) {
                return port_str.parse::<u16>().is_ok();
            }
        }
        false
    }

    fn forward_https_upstream<W: Write>(
        method: &str,
        upstream_base: &str,
        stripped_path: &str,
        client_headers: &[(String, String)],
        transformed_body: &str,
        mmu: &InfinityContextMmu,
        writer: &mut W,
    ) -> io::Result<()> {
        let safe_method = if Self::is_valid_http_token(method) {
            method
        } else {
            "POST"
        };
        let full_url = Self::join_upstream_base_and_route(upstream_base, stripped_path);
        let mut cmd = Command::new("curl");
        cmd.args([
            "-sS",
            "-i",
            "-N",
            "--suppress-connect-headers",
            "--http1.1",
            "--connect-timeout",
            "10",
            "--speed-time",
            "60",
            "--speed-limit",
            "1",
            "-H",
            "Expect:",
            "-H",
            "Accept-Encoding: identity",
            "-X",
            safe_method,
        ]);
        let mut has_content_type = false;
        for (k, v) in client_headers {
            if Self::is_hop_by_hop_or_internal_header(k)
                || !Self::is_valid_http_token(k)
                || v.contains(['\r', '\n', '\0'])
            {
                continue;
            }
            if k.eq_ignore_ascii_case("content-type") {
                has_content_type = true;
            }
            cmd.args(["-H", &format!("{k}: {v}")]);
        }
        if !has_content_type {
            cmd.args(["-H", "Content-Type: application/json"]);
        }
        cmd.args(["--data-binary", "@-", "--", &full_url]);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let mut child = cmd.spawn()?;
        let write_res = if let Some(mut child_in) = child.stdin.take() {
            child_in.write_all(transformed_body.as_bytes())
        } else {
            Ok(())
        };
        let relay_res = match write_res {
            Ok(()) => match child.stdout.take() {
                Some(child_out) => {
                    let mut up_reader = BufReader::new(child_out);
                    Self::relay_upstream_response(&mut up_reader, true, mmu, writer)
                }
                None => Err(io::Error::other("missing curl stdout")),
            },
            Err(e) => Err(e),
        };
        let status = child.wait()?;
        if relay_res.is_err() && !status.success() {
            return Err(io::Error::other(format!("curl exited with {status}")));
        }
        relay_res
    }

    fn write_payload_too_large<W: Write>(writer: &mut W) -> io::Result<()> {
        let err_json = "{\"error\":\"request payload exceeds 16 MiB limit\"}";
        write!(
            writer,
            "HTTP/1.1 413 Payload Too Large\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{err_json}",
            err_json.len(),
        )?;
        writer.flush()
    }

    /// Handles a single HTTP/1.1 request on `reader` and writes an HTTP/1.1 response to `writer`.
    /// Supports `/health` (`GET`), `/sse-expand` (`POST`), `/transform` (`POST` transform-only),
    /// and `/anthropic/v1/messages`, `/openai/v1/responses`, `/openai/v1/chat/completions`, and
    /// `/gemini/v1beta/models/*` (`POST` with upstream forwarding when an upstream is configured).
    pub fn handle_http_request<R: Read, W: Write>(
        reader: R,
        writer: W,
        default_provider: Option<ProviderFormat>,
        mmu: &mut InfinityContextMmu,
    ) -> io::Result<()> {
        Self::handle_http_request_with_config(reader, writer, default_provider, None, None, mmu)
    }

    /// Handles a single HTTP/1.1 request with optional `upstream_override` (`Some("http://...")`,
    /// `Some("https://...")`, or `Some("auto")` to forward to the provider's default upstream)
    /// and optional `workspace_dir` (reloading and saving `.icx/` transactionally under the
    /// workspace lock before any upstream network round-trip).
    pub fn handle_http_request_with_config<R: Read, W: Write>(
        reader: R,
        mut writer: W,
        default_provider: Option<ProviderFormat>,
        upstream_override: Option<&str>,
        workspace_dir: Option<&Path>,
        mmu: &mut InfinityContextMmu,
    ) -> io::Result<()> {
        let mut buf_reader = BufReader::new(reader);
        let mut request_line = String::new();
        if buf_reader.read_line(&mut request_line)? == 0 {
            return Ok(());
        }
        let parts: Vec<&str> = request_line.split_whitespace().collect();
        let method = parts.first().copied().unwrap_or("GET");
        let path = parts.get(1).copied().unwrap_or("/");

        let mut content_length = 0usize;
        let mut payload_too_large = false;
        let mut req_is_chunked = false;
        let mut client_headers: Vec<(String, String)> = Vec::new();
        let mut header_upstream: Option<String> = None;
        let mut transform_only = path.starts_with("/transform");
        loop {
            let mut header_line = String::new();
            let n = buf_reader.read_line(&mut header_line)?;
            if n == 0 || header_line == "\r\n" || header_line == "\n" {
                break;
            }
            if let Some((k, v)) = header_line.split_once(':') {
                let k_trim = k.trim();
                let v_trim = v.trim();
                if k_trim.eq_ignore_ascii_case("content-length") {
                    match v_trim.parse::<usize>() {
                        Ok(len) if len <= Self::MAX_REQUEST_BODY_BYTES => content_length = len,
                        _ => payload_too_large = true,
                    }
                } else if k_trim.eq_ignore_ascii_case("transfer-encoding")
                    && v_trim.to_ascii_lowercase().contains("chunked")
                {
                    req_is_chunked = true;
                } else if k_trim.eq_ignore_ascii_case("x-icx-upstream") && !v_trim.is_empty() {
                    header_upstream = Some(v_trim.to_owned());
                } else if k_trim.eq_ignore_ascii_case("x-icx-transform-only") {
                    transform_only = v_trim == "1" || v_trim.eq_ignore_ascii_case("true");
                }
                client_headers.push((k_trim.to_owned(), v_trim.to_owned()));
            }
        }

        if payload_too_large {
            return Self::write_payload_too_large(&mut writer);
        }

        let mut body_bytes = Vec::new();
        if req_is_chunked {
            loop {
                let remaining = Self::MAX_REQUEST_BODY_BYTES.saturating_sub(body_bytes.len());
                match Self::read_next_http_chunk(&mut buf_reader, remaining) {
                    Ok(Some(chunk)) => body_bytes.extend_from_slice(&chunk),
                    Ok(None) => break,
                    Err(err)
                        if err.kind() == io::ErrorKind::InvalidData
                            && err.to_string().contains("exceeds remaining budget") =>
                    {
                        return Self::write_payload_too_large(&mut writer);
                    }
                    Err(err) => return Err(err),
                }
            }
        } else if content_length > 0 {
            body_bytes.resize(content_length, 0u8);
            buf_reader.read_exact(&mut body_bytes)?;
        }
        let body_str = String::from_utf8_lossy(&body_bytes).into_owned();

        // Execute the local workspace read-modify-write under the exclusive lock and release the
        // lock BEFORE performing any upstream network round-trip.
        let ws_opt = workspace_dir.filter(|p| p.exists());
        let lock_guard = match ws_opt {
            Some(p) => {
                let g = InfinityContextMmu::acquire_workspace_lock(p)
                    .map_err(|e| io::Error::other(e.to_string()))?;
                mmu.load_workspace_dir_unlocked(p)
                    .map_err(|e| io::Error::other(e.to_string()))?;
                Some((p, g))
            }
            None => None,
        };

        if method.eq_ignore_ascii_case("GET") && path.starts_with("/health") {
            let resp_body = format!(
                "{{\"status\":\"ok\",\"cas_artifacts\":{},\"sealed_epochs\":{}}}",
                mmu.cas.artifact_count(),
                mmu.epochs.sealed_epoch_count()
            );
            drop(lock_guard);
            write!(
                writer,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                resp_body.len(),
                resp_body
            )?;
            return writer.flush();
        }

        if path.starts_with("/sse-expand") {
            let expanded = Self::transform_sse_chunk(&body_str, mmu);
            drop(lock_guard);
            write!(
                writer,
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                expanded.len(),
                expanded
            )?;
            return writer.flush();
        }

        let resolved_provider = Self::resolve_provider(default_provider, path, &body_str);
        let transformed_res =
            Self::transform_for_provider(Some(resolved_provider), path, &body_str, mmu);

        if let Some((p, g)) = lock_guard {
            let save_res = mmu.save_workspace_dir_unlocked(p);
            drop(g);
            save_res.map_err(|e| io::Error::other(e.to_string()))?;
        }

        let transformed = match transformed_res {
            Ok(t) => t,
            Err(err) => {
                let err_json = json!({ "error": err.to_string() }).to_string();
                write!(
                    writer,
                    "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    err_json.len(),
                    err_json
                )?;
                return writer.flush();
            }
        };

        let env_upstream = std::env::var("ICX_UPSTREAM_URL")
            .ok()
            .filter(|s| !s.trim().is_empty());
        let allow_header_upstream = upstream_override.is_none();
        let effective_upstream: Option<&str> = if transform_only {
            None
        } else if allow_header_upstream
            && let Some(ref hu) = header_upstream
            && Self::is_loopback_upstream(hu)
        {
            Some(hu.as_str())
        } else if let Some(uo) = upstream_override {
            match uo {
                "none" | "transform-only" => None,
                "auto" => env_upstream
                    .as_deref()
                    .or_else(|| Some(resolved_provider.default_upstream_url())),
                other => Some(other),
            }
        } else {
            env_upstream.as_deref()
        };

        if let Some(upstream_base) = effective_upstream {
            let stripped_path = Self::strip_provider_route_prefix(path);
            let forward_res = if upstream_base.starts_with("https://") {
                Self::forward_https_upstream(
                    method,
                    upstream_base,
                    &stripped_path,
                    &client_headers,
                    &transformed,
                    mmu,
                    &mut writer,
                )
            } else {
                Self::forward_http_upstream(
                    method,
                    upstream_base,
                    &stripped_path,
                    &client_headers,
                    &transformed,
                    mmu,
                    &mut writer,
                )
            };
            if let Err(err) = forward_res {
                let err_json =
                    json!({ "error": format!("upstream gateway error: {err}") }).to_string();
                write!(
                    writer,
                    "HTTP/1.1 502 Bad Gateway\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    err_json.len(),
                    err_json
                )?;
                return writer.flush();
            }
            return Ok(());
        }

        write!(
            writer,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            transformed.len(),
            transformed
        )?;
        writer.flush()
    }

    /// Serves `max_requests` HTTP/1.1 requests on `listener` (or runs indefinitely when
    /// `max_requests` is `None`).
    pub fn serve_http_listener(
        listener: &TcpListener,
        default_provider: Option<ProviderFormat>,
        max_requests: Option<usize>,
        mmu: &mut InfinityContextMmu,
    ) -> io::Result<usize> {
        Self::serve_http_listener_with_config(
            listener,
            default_provider,
            None,
            None,
            max_requests,
            mmu,
        )
    }

    /// Serves HTTP/1.1 requests on `listener` with optional `upstream_override` and transactional
    /// `workspace_dir` persistence on each request.
    pub fn serve_http_listener_with_config(
        listener: &TcpListener,
        default_provider: Option<ProviderFormat>,
        upstream_override: Option<&str>,
        workspace_dir: Option<&Path>,
        max_requests: Option<usize>,
        mmu: &mut InfinityContextMmu,
    ) -> io::Result<usize> {
        if max_requests == Some(0) {
            return Ok(0);
        }
        let mut served = 0usize;
        for stream_res in listener.incoming() {
            let Ok(mut stream) = stream_res else {
                continue;
            };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
            let Ok(reader) = stream.try_clone() else {
                continue;
            };
            let _ = Self::handle_http_request_with_config(
                reader,
                &mut stream,
                default_provider,
                upstream_override,
                workspace_dir,
                mmu,
            );
            served += 1;
            if max_requests.is_some_and(|max| served >= max) {
                break;
            }
        }
        Ok(served)
    }

    fn collect_openai_pager_call_ids(items: &[Value]) -> BTreeSet<String> {
        let mut ids = BTreeSet::new();
        for item in items {
            let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
            let name = item.get("name").and_then(Value::as_str).unwrap_or("");
            if item_type == "function_call" && InfinityContextMmu::is_icx_pager_tool(name) {
                for k in ["call_id", "id"] {
                    if let Some(cid) = item.get(k).and_then(Value::as_str) {
                        ids.insert(cid.to_owned());
                    }
                }
            }
            if let Some(Value::Array(tcs)) = item.get("tool_calls") {
                for tc in tcs {
                    let tc_name = tc
                        .get("function")
                        .and_then(|f| f.get("name"))
                        .or_else(|| tc.get("name"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    if InfinityContextMmu::is_icx_pager_tool(tc_name) {
                        for k in ["id", "call_id"] {
                            if let Some(cid) = tc.get(k).and_then(Value::as_str) {
                                ids.insert(cid.to_owned());
                            }
                        }
                    }
                }
            }
        }
        ids
    }

    fn transform_openai_items(
        items: &mut [Value],
        mmu: &mut InfinityContextMmu,
        protect_tail: usize,
        text_part_type: &str,
    ) {
        let pager_call_ids = Self::collect_openai_pager_call_ids(items);
        let total = items.len();
        if total > protect_tail {
            let fold_end = total - protect_tail;
            let mut occ_idx = 0usize;
            for (idx, item) in items[..fold_end].iter_mut().enumerate() {
                let role = item.get("role").and_then(Value::as_str).unwrap_or("");
                if role == "system" || role == "developer" {
                    continue;
                }
                let name = item.get("name").and_then(Value::as_str).unwrap_or("");
                let is_pager_call = item
                    .get("call_id")
                    .or_else(|| item.get("tool_call_id"))
                    .and_then(Value::as_str)
                    .is_some_and(|cid| pager_call_ids.contains(cid));
                if InfinityContextMmu::is_icx_pager_tool(name) || is_pager_call {
                    continue;
                }
                let subj = format!("openai_turn_{idx}");
                Self::virtualize_item_text(item, &subj, &mut occ_idx, mmu);
            }
        }

        let last_query = items
            .last()
            .and_then(Self::extract_item_text)
            .map(Self::strip_existing_tail_slot)
            .filter(|s| !s.is_empty())
            .unwrap_or("openai_turn")
            .to_owned();
        let tail_slot = mmu.build_ephemeral_tail_slot(AgentKind::Codex, &last_query, 3);
        if let Some(last_item) = items.last_mut() {
            Self::append_tail_to_item(last_item, &tail_slot, text_part_type);
        }
    }

    fn extract_item_text(item: &Value) -> Option<&str> {
        if let Some(s) = item.get("content").and_then(Value::as_str) {
            return Some(s);
        }
        if let Some(arr) = item.get("content").and_then(Value::as_array) {
            for part in arr.iter().rev() {
                if let Some(t) = part
                    .get("text")
                    .or_else(|| part.get("content"))
                    .and_then(Value::as_str)
                {
                    return Some(t);
                }
            }
        }
        item.get("output").and_then(Value::as_str)
    }

    fn virtualize_item_text(
        item: &mut Value,
        subj: &str,
        occ_idx: &mut usize,
        mmu: &mut InfinityContextMmu,
    ) {
        let threshold = mmu.tool_virtualize_threshold;
        if let Some(content_val) = item.get_mut("content") {
            match content_val {
                Value::String(s) => {
                    if s.starts_with('⟦') || s.starts_with("⟪ICX:") {
                        *occ_idx += 1;
                        return;
                    }
                    if s.len() >= threshold {
                        let (res, _) = mmu.ingest_or_reuse_at_index(
                            *occ_idx,
                            AgentKind::Codex,
                            subj,
                            s,
                            None,
                            false,
                        );
                        *occ_idx += 1;
                        *s = res.compressed_ahtl;
                        return;
                    }
                }
                Value::Array(parts) => {
                    for (p_idx, part) in parts.iter_mut().enumerate() {
                        if let Some(Value::String(t)) = part.get_mut("text") {
                            if t.starts_with('⟦') || t.starts_with("⟪ICX:") {
                                *occ_idx += 1;
                            } else if t.len() >= threshold {
                                let part_subj = format!("{subj}_p{p_idx}");
                                let (res, _) = mmu.ingest_or_reuse_at_index(
                                    *occ_idx,
                                    AgentKind::Codex,
                                    &part_subj,
                                    t,
                                    None,
                                    false,
                                );
                                *occ_idx += 1;
                                *t = res.compressed_ahtl;
                            }
                        }
                    }
                    return;
                }
                _ => {}
            }
        }
        if let Some(Value::String(out_s)) = item.get_mut("output") {
            if out_s.starts_with('⟦') || out_s.starts_with("⟪ICX:") {
                *occ_idx += 1;
            } else if out_s.len() >= threshold {
                let (res, _) = mmu.ingest_or_reuse_at_index(
                    *occ_idx,
                    AgentKind::Codex,
                    subj,
                    out_s,
                    None,
                    false,
                );
                *occ_idx += 1;
                *out_s = res.compressed_ahtl;
            }
        }
    }

    fn append_tail_to_item(item: &mut Value, tail_slot: &str, text_part_type: &str) {
        if let Some(content_val) = item.get_mut("content") {
            match content_val {
                Value::String(s) => {
                    let base = Self::strip_existing_tail_slot(s).to_owned();
                    *s = Self::join_base_and_tail(&base, tail_slot);
                    return;
                }
                Value::Array(parts) => {
                    if let Some(last_part) = parts.last_mut() {
                        if let Some(Value::String(t)) = last_part.get_mut("text") {
                            let base = Self::strip_existing_tail_slot(t).to_owned();
                            *t = Self::join_base_and_tail(&base, tail_slot);
                            return;
                        }
                    }
                    parts.push(json!({ "type": text_part_type, "text": tail_slot }));
                    return;
                }
                _ => {}
            }
        }
        if let Some(Value::String(out_s)) = item.get_mut("output") {
            let base = Self::strip_existing_tail_slot(out_s).to_owned();
            *out_s = Self::join_base_and_tail(&base, tail_slot);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anthropic_and_openai_wire_transformers_inject_cache_control_and_virtualize_middle_history() {
        let mut mmu = InfinityContextMmu::default();
        let big_middle = "fn bulky_middle() {}\n".repeat(60);
        let req = json!({
            "model": "claude-opus-4-6",
            "messages": [
                { "role": "user", "content": big_middle },
                { "role": "assistant", "content": "ok" },
                { "role": "user", "content": "What was bulky_middle?" }
            ]
        })
        .to_string();

        let transformed =
            WireProxyTransformer::transform_anthropic_messages(&req, &mut mmu).unwrap();
        let val: Value = serde_json::from_str(&transformed).unwrap();
        let msgs = val["messages"].as_array().unwrap();

        assert!(
            msgs[0]["content"]
                .as_str()
                .unwrap()
                .starts_with("⟦Δ:turn_0"),
            "oversized middle turn must be virtualized into AHTL"
        );
        assert_eq!(msgs[0]["cache_control"]["type"], "ephemeral");
        assert!(
            msgs[2]["content"]
                .as_str()
                .unwrap()
                .contains("⟪ICX:TAIL_SLOT"),
            "final turn must receive the Ephemeral Tail Slot"
        );

        let openai_transformed =
            WireProxyTransformer::transform_openai_request(&req, &mut mmu).unwrap();
        let openai_val: Value = serde_json::from_str(&openai_transformed).unwrap();
        let o_msgs = openai_val["messages"].as_array().unwrap();
        assert!(
            o_msgs[0]["content"]
                .as_str()
                .unwrap()
                .starts_with("⟦Δ:openai_turn_0")
        );
        assert!(
            o_msgs[2]["content"]
                .as_str()
                .unwrap()
                .contains("⟪ICX:TAIL_SLOT")
        );

        let def_transformer = WireProxyTransformer;
        assert!(format!("{def_transformer:?}").contains("WireProxyTransformer"));
        let short_req = json!({
            "model": "gpt-5",
            "messages": [
                { "role": "user", "content": "short 1" },
                { "role": "assistant", "content": "short 2" },
                { "role": "user", "content": "short 3" }
            ]
        })
        .to_string();
        let short_anth =
            WireProxyTransformer::transform_anthropic_messages(&short_req, &mut mmu).unwrap();
        assert!(short_anth.contains("ephemeral"));
        let short_oai =
            WireProxyTransformer::transform_openai_request(&short_req, &mut mmu).unwrap();
        assert!(short_oai.contains("short 1"));

        let empty_anth =
            WireProxyTransformer::transform_anthropic_messages("{}", &mut mmu).unwrap();
        assert_eq!(empty_anth, "{}");
        let empty_oai = WireProxyTransformer::transform_openai_request("{}", &mut mmu).unwrap();
        assert_eq!(empty_oai, "{}");

        let two_msg = json!({
            "messages": [
                { "role": "user", "content": "first" },
                { "role": "user", "content": "second" }
            ]
        })
        .to_string();
        assert!(
            WireProxyTransformer::transform_anthropic_messages(&two_msg, &mut mmu)
                .unwrap()
                .contains("⟪ICX:TAIL_SLOT")
        );
        assert!(
            WireProxyTransformer::transform_openai_request(&two_msg, &mut mmu)
                .unwrap()
                .contains("⟪ICX:TAIL_SLOT")
        );

        // Verify Zone 0 `system` / `developer` prompts and already-virtualized AHTL turns are never overwritten
        let big_sys = "You are a systems architect.\n".repeat(40);
        let pinned_req = json!({
            "model": "gpt-5",
            "messages": [
                { "role": "system", "content": big_sys },
                { "role": "developer", "content": big_sys },
                { "role": "user", "content": "⟦Δ:already_virtualized⟧".repeat(30) },
                { "role": "assistant", "content": "ack" },
                { "role": "user", "content": "latest query" }
            ]
        })
        .to_string();
        let pinned_oai: Value = serde_json::from_str(
            &WireProxyTransformer::transform_openai_request(&pinned_req, &mut mmu).unwrap(),
        )
        .unwrap();
        assert_eq!(
            pinned_oai["messages"][0]["content"].as_str().unwrap(),
            big_sys
        );
        assert_eq!(
            pinned_oai["messages"][1]["content"].as_str().unwrap(),
            big_sys
        );
        assert!(
            pinned_oai["messages"][2]["content"]
                .as_str()
                .unwrap()
                .starts_with("⟦Δ:already_virtualized⟧")
        );

        // Verify OpenAI `/v1/responses` `input` string and `input` item array (parts + function_call_output)
        let resp_str_req = json!({
            "model": "gpt-5-codex",
            "input": "Summarize the latest commit"
        })
        .to_string();
        let resp_str_out: Value = serde_json::from_str(
            &WireProxyTransformer::transform_openai_request(&resp_str_req, &mut mmu).unwrap(),
        )
        .unwrap();
        assert!(
            resp_str_out["input"]
                .as_str()
                .unwrap()
                .contains("⟪ICX:TAIL_SLOT")
        );

        let resp_arr_req = json!({
            "model": "gpt-5-codex",
            "input": [
                {
                    "role": "user",
                    "content": [
                        { "type": "input_text", "text": big_middle }
                    ]
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_1",
                    "output": big_middle
                },
                {
                    "role": "assistant",
                    "content": [
                        { "type": "output_text", "text": "done" }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        { "type": "input_text", "text": "What did call_1 return?" }
                    ]
                }
            ]
        })
        .to_string();
        let resp_arr_out: Value = serde_json::from_str(
            &WireProxyTransformer::transform_openai_request(&resp_arr_req, &mut mmu).unwrap(),
        )
        .unwrap();
        let in_items = resp_arr_out["input"].as_array().unwrap();
        assert!(
            in_items[0]["content"][0]["text"]
                .as_str()
                .unwrap()
                .starts_with("⟦Δ:openai_turn_0_p0")
        );
        assert!(
            in_items[1]["output"]
                .as_str()
                .unwrap()
                .starts_with("⟦Δ:openai_turn_1")
        );
        assert!(
            in_items[3]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("⟪ICX:TAIL_SLOT")
        );

        // Verify Anthropic content-block arrays (`tool_result` + `text`, pager exemption, and cache_control stripping)
        let anth_blocks_req = json!({
            "model": "claude-opus-4-6",
            "messages": [
                {
                    "role": "user",
                    "cache_control": { "type": "ephemeral" },
                    "content": [
                        { "type": "text", "text": big_middle, "cache_control": { "type": "ephemeral" } },
                        { "type": "tool_result", "name": "mcp__icx__icx", "content": big_middle },
                        { "type": "tool_result", "name": "Bash", "content": big_middle },
                        { "type": "image", "source": { "type": "base64" } }
                    ]
                },
                { "role": "assistant", "content": "ack" },
                {
                    "role": "user",
                    "content": [
                        { "type": "image", "source": { "type": "base64" } }
                    ]
                }
            ]
        })
        .to_string();
        let anth_blocks_out: Value = serde_json::from_str(
            &WireProxyTransformer::transform_anthropic_messages(&anth_blocks_req, &mut mmu)
                .unwrap(),
        )
        .unwrap();
        let ab_msgs = anth_blocks_out["messages"].as_array().unwrap();
        assert!(
            ab_msgs[0]["content"][0]["text"]
                .as_str()
                .unwrap()
                .starts_with("⟦Δ:turn_0_b0")
        );
        assert!(ab_msgs[0]["content"][0].get("cache_control").is_none());
        // Pager block (`mcp__icx__icx`) is preserved untouched
        assert_eq!(
            ab_msgs[0]["content"][1]["content"].as_str().unwrap(),
            big_middle
        );
        // Non-pager `tool_result` content is virtualized
        assert!(
            ab_msgs[0]["content"][2]["content"]
                .as_str()
                .unwrap()
                .starts_with("⟦Δ:turn_0_b2")
        );
        assert_eq!(ab_msgs[0]["cache_control"]["type"], "ephemeral");
        // Final message without a text block gets a new text block appended with the tail slot
        assert!(
            ab_msgs[2]["content"][1]["text"]
                .as_str()
                .unwrap()
                .contains("⟪ICX:TAIL_SLOT")
        );

        // Verify Gemini `/v1beta/models/*` request transformation (`contents[].parts[].text`)
        let gemini_req = json!({
            "contents": [
                {
                    "role": "user",
                    "parts": [
                        { "text": big_middle }
                    ]
                },
                {
                    "role": "model",
                    "parts": [
                        { "text": "ack" }
                    ]
                },
                {
                    "role": "user",
                    "parts": [
                        { "text": "Summarize bulky_middle\n⟪ICX:TAIL_SLOT│old⟫" }
                    ]
                }
            ]
        })
        .to_string();
        let gemini_out: Value = serde_json::from_str(
            &WireProxyTransformer::transform_gemini_request(&gemini_req, &mut mmu).unwrap(),
        )
        .unwrap();
        let g_contents = gemini_out["contents"].as_array().unwrap();
        assert!(
            g_contents[0]["parts"][0]["text"]
                .as_str()
                .unwrap()
                .starts_with("⟦Δ:gemini_turn_0_p0")
        );
        assert!(
            g_contents[2]["parts"][0]["text"]
                .as_str()
                .unwrap()
                .contains("⟪ICX:TAIL_SLOT│agent=agy")
        );
        let gemini_non_text = json!({
            "contents": [
                {
                    "role": "user",
                    "parts": [
                        { "inlineData": { "mimeType": "image/png", "data": "AAAA" } }
                    ]
                }
            ]
        })
        .to_string();
        let gemini_nt_out: Value = serde_json::from_str(
            &WireProxyTransformer::transform_gemini_request(&gemini_non_text, &mut mmu).unwrap(),
        )
        .unwrap();
        assert!(
            gemini_nt_out["contents"][0]["parts"][1]["text"]
                .as_str()
                .unwrap()
                .contains("⟪ICX:TAIL_SLOT")
        );

        // Verify inline `⟪!icx:deref(#icx:...)⟫` and SSE chunk expansion
        let sample_res = mmu.ingest_payload(
            AgentKind::ClaudeCode,
            "sse_target",
            "line_alpha\nline_beta\nline_gamma\n",
            false,
        );
        let aid = sample_res.artifact_id;
        assert_eq!(
            WireProxyTransformer::expand_inline_deref_directives("plain text", &mmu),
            "plain text"
        );
        let inline_full = WireProxyTransformer::expand_inline_deref_directives(
            &format!("prefix ⟪!icx:deref({aid})⟫ suffix"),
            &mmu,
        );
        assert!(inline_full.contains("line_beta"));
        let inline_slice = WireProxyTransformer::expand_inline_deref_directives(
            &format!("slice: ⟪!icx:deref({aid},2:2)⟫!"),
            &mmu,
        );
        assert_eq!(inline_slice, "slice: line_beta!");
        let inline_bad = WireProxyTransformer::expand_inline_deref_directives(
            "bad: ⟪!icx:deref(#icx:0000000000000000)⟫ / ⟪!icx:deref(unterminated",
            &mmu,
        );
        assert!(inline_bad.contains("⟪ICX:DEREF_ERR│"));
        assert!(inline_bad.contains("⟪!icx:deref(unterminated"));
        let inline_bad_range = WireProxyTransformer::expand_inline_deref_directives(
            &format!("⟪!icx:deref({aid},99:100)⟫"),
            &mmu,
        );
        assert!(inline_bad_range.contains("⟪ICX:DEREF_ERR│"));

        let sse_plain = "event: ping\ndata: {\"type\":\"ping\"}\n\n";
        assert_eq!(
            WireProxyTransformer::transform_sse_chunk(sse_plain, &mmu),
            sse_plain
        );
        let sse_with_deref = format!(
            "event: content_block_delta\ndata: {{\"delta\":{{\"text\":\"Here: ⟪!icx:deref({aid},1:2)⟫\"}},\"n\":1,\"arr\":[\"⟪!icx:deref({aid},3:3)⟫\"]}}\ndata: raw_non_json ⟪!icx:deref({aid},1:1)⟫\n\n"
        );
        let sse_expanded = WireProxyTransformer::transform_sse_chunk(&sse_with_deref, &mmu);
        assert!(sse_expanded.contains("line_alpha\\nline_beta"));
        assert!(sse_expanded.contains("line_gamma"));
        assert!(sse_expanded.contains("raw_non_json line_alpha"));

        // Verify ProviderFormat parsing, auto-detection, and HTTP/1.1 listener handling
        assert_eq!(
            ProviderFormat::parse_provider("anthropic"),
            Some(ProviderFormat::Anthropic)
        );
        assert_eq!(
            ProviderFormat::parse_provider("/openai/v1/responses"),
            Some(ProviderFormat::OpenAI)
        );
        assert_eq!(
            ProviderFormat::parse_provider("/gemini/v1beta/models/gemini-2.5-pro:generateContent"),
            Some(ProviderFormat::Gemini)
        );
        assert_eq!(ProviderFormat::parse_provider("unknown"), None);

        assert!(
            WireProxyTransformer::transform_for_provider(
                None,
                "/",
                r#"{"model":"gemini-2.5-pro","contents":[{"role":"user","parts":[{"text":"hi"}]}]}"#,
                &mut mmu,
            )
            .unwrap()
            .contains("⟪ICX:TAIL_SLOT")
        );
        assert!(
            WireProxyTransformer::transform_for_provider(
                None,
                "/",
                r#"{"model":"gemini-2.5-pro"}"#,
                &mut mmu,
            )
            .is_ok()
        );
        assert!(
            WireProxyTransformer::transform_for_provider(
                None,
                "/",
                r#"{"model":"o3-mini","messages":[{"role":"user","content":"hi"}]}"#,
                &mut mmu,
            )
            .unwrap()
            .contains("⟪ICX:TAIL_SLOT")
        );
        assert!(
            WireProxyTransformer::transform_for_provider(
                None,
                "/",
                r#"{"input":"hello codex"}"#,
                &mut mmu,
            )
            .unwrap()
            .contains("⟪ICX:TAIL_SLOT")
        );

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client_handle = std::thread::spawn(move || {
            // 1. GET /health
            {
                let mut s = std::net::TcpStream::connect(addr).unwrap();
                s.write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n")
                    .unwrap();
                let mut resp = String::new();
                s.read_to_string(&mut resp).unwrap();
                assert!(resp.contains("200 OK"));
                assert!(resp.contains(r#""status":"ok""#));
            }
            // 2. POST /anthropic/v1/messages
            {
                let body = r#"{"model":"claude-opus-4-6","messages":[{"role":"user","content":"http test"}]}"#;
                let req_str = format!(
                    "POST /anthropic/v1/messages HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let mut s = std::net::TcpStream::connect(addr).unwrap();
                s.write_all(req_str.as_bytes()).unwrap();
                let mut resp = String::new();
                s.read_to_string(&mut resp).unwrap();
                assert!(resp.contains("200 OK"));
                assert!(resp.contains("⟪ICX:TAIL_SLOT"));
            }
            // 3. POST /sse-expand
            {
                let body = format!("data: {{\"text\":\"⟪!icx:deref({aid},1:1)⟫\"}}\n\n");
                let req_str = format!(
                    "POST /sse-expand HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let mut s = std::net::TcpStream::connect(addr).unwrap();
                s.write_all(req_str.as_bytes()).unwrap();
                let mut resp = String::new();
                s.read_to_string(&mut resp).unwrap();
                assert!(resp.contains("text/event-stream"));
                assert!(resp.contains("line_alpha"));
            }
            // 4. POST invalid JSON -> 400 Bad Request
            {
                let body = "{bad_json";
                let req_str = format!(
                    "POST /openai/v1/responses HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let mut s = std::net::TcpStream::connect(addr).unwrap();
                s.write_all(req_str.as_bytes()).unwrap();
                let mut resp = String::new();
                s.read_to_string(&mut resp).unwrap();
                assert!(resp.contains("400 Bad Request"));
            }
        });
        let served =
            WireProxyTransformer::serve_http_listener(&listener, None, Some(4), &mut mmu).unwrap();
        assert_eq!(served, 4);
        client_handle.join().unwrap();

        // Also test empty EOF on handle_http_request and OpenAI pager item exemption
        let mut empty_out = Vec::new();
        WireProxyTransformer::handle_http_request(
            io::Cursor::new(b""),
            &mut empty_out,
            None,
            &mut mmu,
        )
        .unwrap();
        assert_eq!(empty_out.len(), 0);
        let oai_pager_req = json!({
            "messages": [
                { "role": "tool", "name": "mcp__icx__icx", "content": big_middle },
                { "role": "assistant", "content": "ok" },
                { "role": "user", "content": [ { "type": "image_url" } ] }
            ]
        })
        .to_string();
        let oai_pager_out: Value = serde_json::from_str(
            &WireProxyTransformer::transform_openai_request(&oai_pager_req, &mut mmu).unwrap(),
        )
        .unwrap();
        assert_eq!(
            oai_pager_out["messages"][0]["content"].as_str().unwrap(),
            big_middle
        );

        // Test `serve_http_listener` with `max_requests: Some(0)`, OpenAI `content` array virtualization
        // and tail append, Anthropic missing `content`, and o1/o3/o4/codex model auto-detection
        assert_eq!(
            WireProxyTransformer::serve_http_listener(&listener, None, Some(0), &mut mmu).unwrap(),
            0
        );
        let oai_arr_req = json!({
            "messages": [
                { "role": "user", "content": [ { "type": "text", "text": big_middle } ] },
                { "role": "assistant", "content": "mid" },
                { "role": "user", "content": [ { "type": "text", "text": "tail_arr\n⟪ICX:TAIL_SLOT│old⟫" } ] }
            ]
        })
        .to_string();
        let oai_arr_out: Value = serde_json::from_str(
            &WireProxyTransformer::transform_openai_request(&oai_arr_req, &mut mmu).unwrap(),
        )
        .unwrap();
        assert!(
            oai_arr_out["messages"][0]["content"][0]["text"]
                .as_str()
                .unwrap()
                .starts_with("⟦Δ:openai_turn_0_p0")
        );
        assert!(
            oai_arr_out["messages"][2]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("⟪ICX:TAIL_SLOT│agent=codex")
        );

        let anth_no_content = json!({
            "messages": [
                { "role": "user" },
                { "role": "assistant", "content": "r1" },
                { "role": "user", "content": "r2" }
            ]
        })
        .to_string();
        let _ =
            WireProxyTransformer::transform_anthropic_messages(&anth_no_content, &mut mmu).unwrap();

        for m in ["o1-preview", "o3-mini", "o4-mini", "codex-mini"] {
            let req = json!({
                "model": m,
                "messages": [{ "role": "user", "content": "hi" }]
            })
            .to_string();
            let out =
                WireProxyTransformer::transform_for_provider(None, "/", &req, &mut mmu).unwrap();
            assert!(out.contains("⟪ICX:TAIL_SLOT│agent=codex"));
        }

        // Verify malformed inline deref range fails closed, L1-2 range works, and redacted provenance is attached
        let bad_range_out = WireProxyTransformer::expand_inline_deref_directives(
            &format!("⟪!icx:deref({aid},bad_range)⟫"),
            &mmu,
        );
        assert!(bad_range_out.contains("⟪ICX:DEREF_ERR│invalid line range `bad_range`⟫"));
        let l_range_out = WireProxyTransformer::expand_inline_deref_directives(
            &format!("⟪!icx:deref({aid},L1-2)⟫"),
            &mmu,
        );
        assert_eq!(l_range_out, "line_alpha\nline_beta");
        let secret_ingested = mmu.ingest_payload(
            AgentKind::ClaudeCode,
            "secret_env",
            "api_key=sk-ant-api03-1234567890abcdef1234567890abcdef",
            false,
        );
        let redacted_deref_out = WireProxyTransformer::expand_inline_deref_directives(
            &format!("⟪!icx:deref({})⟫", secret_ingested.artifact_id),
            &mmu,
        );
        assert!(redacted_deref_out.starts_with("⟪ICX:PROVENANCE│id=#icx:"));
        assert!(redacted_deref_out.contains("│redacted=true│exactness=post_sanitization_cas⟫"));

        // Verify repeated transformations on image-only array turns do not duplicate the tail slot
        let pass2_oai: Value = serde_json::from_str(
            &WireProxyTransformer::transform_openai_request(
                &serde_json::to_string(&oai_pager_out).unwrap(),
                &mut mmu,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            pass2_oai["messages"][2]["content"]
                .as_array()
                .unwrap()
                .len(),
            2
        );

        // Verify Gemini `is_icx_pager_tool` exemption on `functionResponse` and item `name`
        let gemini_pager_req = json!({
            "contents": [
                {
                    "role": "user",
                    "name": "mcp__icx__icx",
                    "parts": [{ "text": big_middle }]
                },
                {
                    "role": "user",
                    "parts": [
                        {
                            "functionResponse": { "name": "mcp__icx__icx" },
                            "text": big_middle
                        }
                    ]
                },
                { "role": "model", "parts": [{ "text": "ok" }] },
                { "role": "user", "parts": [{ "inlineData": { "mimeType": "image/png", "data": "AAAA" } }] }
            ]
        })
        .to_string();
        let gemini_pager_p1 =
            WireProxyTransformer::transform_gemini_request(&gemini_pager_req, &mut mmu).unwrap();
        let gemini_pager_p2: Value = serde_json::from_str(
            &WireProxyTransformer::transform_gemini_request(&gemini_pager_p1, &mut mmu).unwrap(),
        )
        .unwrap();
        assert_eq!(
            gemini_pager_p2["contents"][0]["parts"][0]["text"]
                .as_str()
                .unwrap(),
            big_middle
        );
        assert_eq!(
            gemini_pager_p2["contents"][1]["parts"][0]["text"]
                .as_str()
                .unwrap(),
            big_middle
        );
        assert_eq!(
            gemini_pager_p2["contents"][3]["parts"]
                .as_array()
                .unwrap()
                .len(),
            2
        );

        // Verify Anthropic `tool_use_id` pager exemption and 4-breakpoint `cache_control` ceiling
        let anth_4bp_req = json!({
            "system": [{ "type": "text", "text": "s1", "cache_control": { "type": "ephemeral" } }],
            "tools": [{ "name": "icx", "cache_control": { "type": "ephemeral" } }],
            "messages": [
                {
                    "role": "assistant",
                    "content": [{ "type": "tool_use", "id": "tu_icx_1", "name": "mcp__icx__icx" }]
                },
                {
                    "role": "user",
                    "content": [{ "type": "tool_result", "tool_use_id": "tu_icx_1", "content": big_middle }]
                },
                {
                    "role": "assistant",
                    "cache_control": { "type": "ephemeral" },
                    "content": "tail_1"
                },
                {
                    "role": "user",
                    "cache_control": { "type": "ephemeral" },
                    "content": "tail_2"
                }
            ]
        })
        .to_string();
        let anth_4bp_out: Value = serde_json::from_str(
            &WireProxyTransformer::transform_anthropic_messages(&anth_4bp_req, &mut mmu).unwrap(),
        )
        .unwrap();
        assert_eq!(
            anth_4bp_out["messages"][1]["content"][0]["content"]
                .as_str()
                .unwrap(),
            big_middle
        );
        assert!(anth_4bp_out["messages"][1].get("cache_control").is_none());

        // Verify OpenAI `/v1/responses` `call_id` pager exemption and `"input_text"` tail block type
        let oai_resp_pager = json!({
            "input": [
                { "type": "function_call", "call_id": "call_icx_1", "name": "icx" },
                { "type": "function_call_output", "call_id": "call_icx_1", "output": big_middle },
                { "role": "assistant", "content": "ok" },
                { "role": "user", "content": [{ "type": "input_image" }] }
            ]
        })
        .to_string();
        let oai_resp_out: Value = serde_json::from_str(
            &WireProxyTransformer::transform_openai_request(&oai_resp_pager, &mut mmu).unwrap(),
        )
        .unwrap();
        assert_eq!(
            oai_resp_out["input"][1]["output"].as_str().unwrap(),
            big_middle
        );
        assert_eq!(
            oai_resp_out["input"][3]["content"][1]["type"]
                .as_str()
                .unwrap(),
            "input_text"
        );

        // Verify end-to-end upstream HTTP forwarding (with 100 Continue + chunked SSE and chunked JSON),
        // HTTPS failure handling, and workspace persistence
        let tmp_ws = std::env::temp_dir().join(format!("icx_proxy_ws_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp_ws);
        mmu.save_workspace_dir(&tmp_ws).unwrap();

        let mock_upstream = TcpListener::bind("127.0.0.1:0").unwrap();
        let mock_up_addr = mock_upstream.local_addr().unwrap();
        let aid_for_up = aid;
        let up_thread = std::thread::spawn(move || {
            // Request 1: Chunked SSE response preceded by 100 Continue
            {
                let (mut up_stream, _) = mock_upstream.accept().unwrap();
                let mut up_reader = BufReader::new(up_stream.try_clone().unwrap());
                let mut req_line = String::new();
                up_reader.read_line(&mut req_line).unwrap();
                assert!(req_line.starts_with("POST /v1/messages HTTP/1.1"));
                let mut cl = 0usize;
                let mut saw_auth = false;
                loop {
                    let mut h = String::new();
                    let n = up_reader.read_line(&mut h).unwrap();
                    if n == 0 || h == "\r\n" || h == "\n" {
                        break;
                    }
                    if h.to_ascii_lowercase().starts_with("authorization:") {
                        saw_auth = true;
                    }
                    if let Some((k, v)) = h.split_once(':')
                        && k.trim().eq_ignore_ascii_case("content-length")
                    {
                        cl = v.trim().parse().unwrap();
                    }
                }
                assert!(saw_auth);
                let mut body_buf = vec![0u8; cl];
                up_reader.read_exact(&mut body_buf).unwrap();
                let body_s = String::from_utf8_lossy(&body_buf);
                assert!(body_s.contains("⟪ICX:TAIL_SLOT"));
                let sse_chunk =
                    format!("data: {{\"delta\":\"⟪!icx:deref({aid_for_up},1:1)⟫\"}}\n\n");
                write!(
                    up_stream,
                    "HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nX-Upstream: mock\r\n\r\n{:x};ext=1\r\n{}\r\n0\r\nX-Trailer: done\r\n\r\n",
                    sse_chunk.len(),
                    sse_chunk
                )
                .unwrap();
                up_stream.flush().unwrap();
            }
            // Request 2: Chunked non-SSE JSON response with multi-line deref
            {
                let (mut up_stream, _) = mock_upstream.accept().unwrap();
                let mut up_reader = BufReader::new(up_stream.try_clone().unwrap());
                let mut cl = 0usize;
                loop {
                    let mut h = String::new();
                    let n = up_reader.read_line(&mut h).unwrap();
                    if n == 0 || h == "\r\n" || h == "\n" {
                        break;
                    }
                    if let Some((k, v)) = h.split_once(':')
                        && k.trim().eq_ignore_ascii_case("content-length")
                    {
                        cl = v.trim().parse().unwrap();
                    }
                }
                let mut body_buf = vec![0u8; cl];
                up_reader.read_exact(&mut body_buf).unwrap();
                let json_resp = format!("{{\"text\":\"⟪!icx:deref({aid_for_up},1:2)⟫\"}}");
                write!(
                    up_stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{}\r\n0\r\n\r\n",
                    json_resp.len(),
                    json_resp
                )
                .unwrap();
                up_stream.flush().unwrap();
            }
        });

        let proxy_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();
        let up_url = format!("http://{mock_up_addr}");
        let proxy_client = std::thread::spawn(move || {
            // 1. Forwarded chunked client request -> chunked SSE response
            {
                let body = r#"{"model":"claude-opus-4-6","messages":[{"role":"user","content":"forward me"}]}"#;
                let req = format!(
                    "POST /anthropic/v1/messages HTTP/1.1\r\nAuthorization: Bearer test\r\nX-ICX-Upstream: {up_url}\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{}\r\n0\r\n\r\n",
                    body.len(),
                    body
                );
                let mut s = TcpStream::connect(proxy_addr).unwrap();
                s.write_all(req.as_bytes()).unwrap();
                let mut resp = String::new();
                s.read_to_string(&mut resp).unwrap();
                assert!(resp.contains("200 OK"));
                assert!(resp.contains("X-Upstream: mock"));
                assert!(resp.contains("line_alpha"));
            }
            // 2. Forwarded request -> chunked JSON response with multi-line deref (JSON-escaped)
            {
                let body = r#"{"model":"claude-opus-4-6","messages":[{"role":"user","content":"json deref"}]}"#;
                let req = format!(
                    "POST /anthropic/v1/messages HTTP/1.1\r\nX-ICX-Upstream: {up_url}\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let mut s = TcpStream::connect(proxy_addr).unwrap();
                s.write_all(req.as_bytes()).unwrap();
                let mut resp = String::new();
                s.read_to_string(&mut resp).unwrap();
                assert!(resp.contains("200 OK"));
                assert!(resp.contains("line_alpha\\nline_beta"));
            }
            // 3. Unreachable HTTP upstream -> 502 Bad Gateway
            {
                let body = r#"{"model":"claude-opus-4-6","messages":[{"role":"user","content":"fail me"}]}"#;
                let req = format!(
                    "POST /anthropic/v1/messages HTTP/1.1\r\nX-ICX-Upstream: http://127.0.0.1:1\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let mut s = TcpStream::connect(proxy_addr).unwrap();
                s.write_all(req.as_bytes()).unwrap();
                let mut resp = String::new();
                s.read_to_string(&mut resp).unwrap();
                assert!(resp.contains("502 Bad Gateway"));
            }
            // 4. Unreachable HTTPS upstream -> 502 Bad Gateway (exercises `forward_https_upstream`)
            {
                let body = r#"{"model":"claude-opus-4-6","messages":[{"role":"user","content":"fail https"}]}"#;
                let req = format!(
                    "POST /anthropic/v1/messages HTTP/1.1\r\nX-ICX-Upstream: https://127.0.0.1:1\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let mut s = TcpStream::connect(proxy_addr).unwrap();
                s.write_all(req.as_bytes()).unwrap();
                let mut resp = String::new();
                s.read_to_string(&mut resp).unwrap();
                assert!(resp.contains("502 Bad Gateway"));
            }
        });
        let served_up = WireProxyTransformer::serve_http_listener_with_config(
            &proxy_listener,
            None,
            None,
            Some(&tmp_ws),
            Some(4),
            &mut mmu,
        )
        .unwrap();
        assert_eq!(served_up, 4);
        up_thread.join().unwrap();
        proxy_client.join().unwrap();
        let _ = std::fs::remove_dir_all(&tmp_ws);

        // Verify ProviderFormat::default_upstream_url, strip_provider_route_prefix, is_loopback_upstream,
        // non-chunked SSE relay, `upstream_override: Some("none")`, and OpenAI `tool_calls` + `output` tail
        assert_eq!(
            ProviderFormat::Anthropic.default_upstream_url(),
            "https://api.anthropic.com"
        );
        assert_eq!(
            ProviderFormat::OpenAI.default_upstream_url(),
            "https://api.openai.com"
        );
        assert_eq!(
            ProviderFormat::Gemini.default_upstream_url(),
            "https://generativelanguage.googleapis.com"
        );
        assert_eq!(
            WireProxyTransformer::strip_provider_route_prefix("/anthropic"),
            "/"
        );
        assert!(WireProxyTransformer::is_loopback_upstream(
            "http://localhost:8080"
        ));
        assert!(WireProxyTransformer::is_loopback_upstream(
            "http://[::1]:8080"
        ));
        assert!(!WireProxyTransformer::is_loopback_upstream(
            "https://example.com"
        ));

        // Non-chunked SSE relay via `relay_upstream_response` (`body_already_dechunked: true`)
        let mut raw_sse_out = Vec::new();
        let mut raw_sse_in = io::Cursor::new(format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\ndata: ⟪!icx:deref({aid},1:1)⟫\n"
        ));
        WireProxyTransformer::relay_upstream_response(
            &mut raw_sse_in,
            true,
            &mmu,
            &mut raw_sse_out,
        )
        .unwrap();
        assert!(String::from_utf8_lossy(&raw_sse_out).contains("line_alpha"));

        // Split multi-byte UTF-8 (`⟪` = 0xE2 0x9F 0xAA) across two HTTP chunks in chunked SSE
        let full_sse_line = format!("data: ⟪!icx:deref({aid},1:1)⟫\n");
        let full_bytes = full_sse_line.as_bytes();
        // Split right after the first byte of `⟪` (index 6 is 0xE2, index 7 is 0x9F)
        let part1 = &full_bytes[..7];
        let part2 = &full_bytes[7..];
        let mut split_chunk_stream = Vec::new();
        split_chunk_stream.extend_from_slice(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n",
        );
        split_chunk_stream.extend_from_slice(format!("{:x}\r\n", part1.len()).as_bytes());
        split_chunk_stream.extend_from_slice(part1);
        split_chunk_stream.extend_from_slice(b"\r\n");
        split_chunk_stream.extend_from_slice(format!("{:x}\r\n", part2.len()).as_bytes());
        split_chunk_stream.extend_from_slice(part2);
        split_chunk_stream.extend_from_slice(b"\r\n0\r\n\r\n");
        let mut split_out = Vec::new();
        let mut split_in = io::Cursor::new(split_chunk_stream);
        WireProxyTransformer::relay_upstream_response(&mut split_in, false, &mmu, &mut split_out)
            .unwrap();
        assert!(String::from_utf8_lossy(&split_out).contains("data: line_alpha"));

        // Nested array `content` inside Anthropic `tool_result` block
        let anth_nested_tr = json!({
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "tu_non_pager",
                            "content": [{ "type": "text", "text": big_middle, "cache_control": { "type": "ephemeral" } }]
                        }
                    ]
                },
                { "role": "assistant", "content": "r1" },
                { "role": "user", "content": "r2" }
            ]
        })
        .to_string();
        let anth_nested_out: Value = serde_json::from_str(
            &WireProxyTransformer::transform_anthropic_messages(&anth_nested_tr, &mut mmu).unwrap(),
        )
        .unwrap();
        assert!(
            anth_nested_out["messages"][0]["content"][0]["content"][0]["text"]
                .as_str()
                .unwrap()
                .starts_with("⟦Δ:turn_0_b0_n0")
        );
        assert!(
            anth_nested_out["messages"][0]["content"][0]["content"][0]
                .get("cache_control")
                .is_none()
        );

        // `upstream_override: Some("none")` and `Some("auto")`
        let mut none_out = Vec::new();
        let none_req = "POST /anthropic HTTP/1.1\r\nContent-Length: 45\r\n\r\n{\"messages\":[{\"role\":\"user\",\"content\":\"hi\"}]}";
        WireProxyTransformer::handle_http_request_with_config(
            io::Cursor::new(none_req.as_bytes()),
            &mut none_out,
            None,
            Some("none"),
            None,
            &mut mmu,
        )
        .unwrap();
        assert!(String::from_utf8_lossy(&none_out).contains("200 OK"));

        // OpenAI `tool_calls` pager exemption, already-virtualized middle turn reuse, and `output` tail item
        let oai_tc_req = json!({
            "input": [
                {
                    "role": "assistant",
                    "tool_calls": [{ "id": "tc_icx_9", "function": { "name": "mcp__icx__icx" } }]
                },
                { "role": "tool", "tool_call_id": "tc_icx_9", "output": big_middle },
                { "role": "user", "content": "⟦Δ:already_virt⊢✓ok⟧" },
                { "role": "user", "output": "final_output_turn" },
                { "role": "user", "output": "final_output_turn_2" }
            ]
        })
        .to_string();
        let oai_tc_out: Value = serde_json::from_str(
            &WireProxyTransformer::transform_openai_request(&oai_tc_req, &mut mmu).unwrap(),
        )
        .unwrap();
        assert_eq!(
            oai_tc_out["input"][1]["output"].as_str().unwrap(),
            big_middle
        );
        assert!(
            oai_tc_out["input"][4]["output"]
                .as_str()
                .unwrap()
                .contains("⟪ICX:TAIL_SLOT")
        );

        // Cross-event SSE deref reassembly (directive split across two consecutive `data: {...}` events)
        // and userinfo (`@`) rejection in `is_loopback_upstream`
        assert!(!WireProxyTransformer::is_loopback_upstream(
            "https://127.0.0.1:443@attacker.example"
        ));
        assert!(!WireProxyTransformer::is_loopback_upstream(
            "http://127.0.0.1:not_a_port"
        ));
        let split_events_sse = format!(
            "event: content_block_delta\ndata: {{\"delta\":{{\"text\":\"Start ⟪!icx:deref(\"}}}}\nevent: content_block_delta\ndata: {{\"delta\":{{\"text\":\"{aid},1:1)⟫ end\"}}}}\ndata: [DONE]\n\n"
        );
        let split_events_out = WireProxyTransformer::transform_sse_chunk(&split_events_sse, &mmu);
        assert!(split_events_out.contains("\"text\":\"Start \""));
        assert!(split_events_out.contains("\"text\":\"line_alpha end\""));
        assert!(split_events_out.contains("data: [DONE]"));

        // Split mid-prefix (`⟪!icx:de` + `ref(...)⟫`), unclosed trailing prefix in `transform_sse_chunk`,
        // and array-wrapped SSE JSON + plain `data:` SSE lines
        let mid_prefix_sse = format!(
            "data: [\"prefix ⟪!icx:de\"]\ndata: [\"ref({aid},1:1)⟫ done\"]\ndata: plain ⟪!icx:deref({aid},2:2)⟫\n"
        );
        let mid_prefix_out = WireProxyTransformer::transform_sse_chunk(&mid_prefix_sse, &mmu);
        assert!(mid_prefix_out.contains("line_alpha done"));
        assert!(mid_prefix_out.contains("data: plain line_beta"));

        let unclosed_sse = "data: {\"delta\":{\"text\":\"hello ⟪!icx:deref(#icx:1234\"}}\n";
        let unclosed_out = WireProxyTransformer::transform_sse_chunk(unclosed_sse, &mmu);
        assert!(unclosed_out.contains("hello ⟪!icx:deref(#icx:1234"));

        // Relay upstream with `100 Continue`, `200 Connection established`, and unclosed SSE pending before `[DONE]`
        let mut continue_sse_out = Vec::new();
        let mut continue_sse_in = io::Cursor::new(
            "HTTP/1.1 200 Connection established\r\n\r\nHTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\ndata: {\"delta\":{\"text\":\"tail ⟪!icx:deref(\"}}\ndata: [DONE]\n",
        );
        WireProxyTransformer::relay_upstream_response(
            &mut continue_sse_in,
            true,
            &mmu,
            &mut continue_sse_out,
        )
        .unwrap();
        let continue_sse_str = String::from_utf8_lossy(&continue_sse_out);
        assert!(continue_sse_str.contains("⟪!icx:deref("));
        assert!(continue_sse_str.contains("data: [DONE]"));

        // Endpoint parsing (port 80 default, IPv6, explicit port) and base-path deduplication
        let (addr1, host1, path1) = WireProxyTransformer::parse_http_upstream_endpoint(
            "http://localhost/v1",
            "/v1/responses",
        );
        assert_eq!(addr1, "localhost:80");
        assert_eq!(host1, "localhost");
        assert_eq!(path1, "/v1/responses");
        let (addr2, _, path2) =
            WireProxyTransformer::parse_http_upstream_endpoint("http://[::1]/v1", "/v1beta/models");
        assert_eq!(addr2, "[::1]:80");
        assert_eq!(path2, "/v1/v1beta/models");
        let (addr3, _, _) =
            WireProxyTransformer::parse_http_upstream_endpoint("http://[::1]:8080", "/v1/messages");
        assert_eq!(addr3, "[::1]:8080");
        assert_eq!(
            WireProxyTransformer::join_upstream_base_and_route(
                "https://gateway.example/v1",
                "/v1/responses"
            ),
            "https://gateway.example/v1/responses"
        );
        assert_eq!(
            WireProxyTransformer::join_upstream_base_and_route(
                "https://gateway.example/v1",
                "/v1beta/models"
            ),
            "https://gateway.example/v1/v1beta/models"
        );

        // HTTP 413 Payload Too Large on oversized `Content-Length` and oversized HTTP chunk header
        let mut too_large_cl_out = Vec::new();
        let too_large_cl_req =
            "POST /anthropic/v1/messages HTTP/1.1\r\nContent-Length: 99999999\r\n\r\n{}";
        WireProxyTransformer::handle_http_request(
            io::Cursor::new(too_large_cl_req.as_bytes()),
            &mut too_large_cl_out,
            None,
            &mut mmu,
        )
        .unwrap();
        assert!(String::from_utf8_lossy(&too_large_cl_out).contains("413 Payload Too Large"));

        let mut too_large_chunk_out = Vec::new();
        let too_large_chunk_req = "POST /anthropic/v1/messages HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\nffffffffffffffff\r\n";
        WireProxyTransformer::handle_http_request(
            io::Cursor::new(too_large_chunk_req.as_bytes()),
            &mut too_large_chunk_out,
            None,
            &mut mmu,
        )
        .unwrap();
        assert!(String::from_utf8_lossy(&too_large_chunk_out).contains("413 Payload Too Large"));

        // Interleaved multi-choice OpenAI SSE stream (`index: 0` vs `index: 1`)
        let multi_choice_sse = format!(
            "data: {{\"choices\":[{{\"index\":0,\"delta\":{{\"content\":\"c0 ⟪!icx:deref(\"}}}}]}}\ndata: {{\"choices\":[{{\"index\":1,\"delta\":{{\"content\":\"c1 untouched\"}}}}]}}\ndata: {{\"choices\":[{{\"index\":0,\"delta\":{{\"content\":\"{aid},1:1)⟫ end\"}}}}]}}\n"
        );
        let multi_choice_out = WireProxyTransformer::transform_sse_chunk(&multi_choice_sse, &mmu);
        assert!(multi_choice_out.contains("\"content\":\"c1 untouched\""));
        assert!(multi_choice_out.contains("\"content\":\"line_alpha end\""));
    }
}
