//! Wire-level Cache-Epoch request transformer (`WireProxyTransformer`) for Anthropic
//! (`/v1/messages`), OpenAI (`/v1/responses`, `/v1/chat/completions`), and Gemini
//! (`/v1beta/models/*`) payloads, plus inbound SSE inline `⟪!icx:deref(#icx:...)⟫` expansion and
//! local HTTP/1.1 reverse-proxy request handling.

use icx_adapters::InfinityContextMmu;
use icx_core::AgentKind;
use serde_json::{Value, json};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::TcpListener;

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

        if let Some(messages) = root.get_mut("messages").and_then(Value::as_array_mut) {
            let total = messages.len();
            if total > protect_tail {
                let fold_end = total - protect_tail;
                for (idx, msg) in messages[..fold_end].iter_mut().enumerate() {
                    let role = msg.get("role").and_then(Value::as_str).unwrap_or("");
                    if role != "system" && role != "developer" {
                        let subj = format!("turn_{idx}");
                        Self::virtualize_anthropic_msg_content(msg, &subj, mmu);
                    }
                    // Pin Anthropic prompt-cache breakpoint on the last sealed epoch message
                    if idx + 1 == fold_end {
                        msg["cache_control"] = json!({ "type": "ephemeral" });
                    } else if let Some(obj) = msg.as_object_mut() {
                        obj.remove("cache_control");
                    }
                }
            }

            // Append Ephemeral Tail Slot to the final message (stripping any prior tail slot)
            let last_query = messages
                .last()
                .and_then(Self::extract_item_text)
                .map(Self::strip_existing_tail_slot)
                .unwrap_or("anthropic_turn")
                .to_owned();
            let tail_slot = mmu.build_ephemeral_tail_slot(AgentKind::ClaudeCode, &last_query, 3);
            if let Some(last_msg) = messages.last_mut() {
                Self::append_tail_to_item(last_msg, &tail_slot);
            }
        }

        serde_json::to_string(&root)
    }

    fn virtualize_anthropic_msg_content(msg: &mut Value, subj: &str, mmu: &mut InfinityContextMmu) {
        let threshold = mmu.tool_virtualize_threshold;
        let Some(content_val) = msg.get_mut("content") else {
            return;
        };
        match content_val {
            Value::String(s)
                if s.len() >= threshold && !s.starts_with('⟦') && !s.starts_with("⟪ICX:") =>
            {
                let res = mmu.ingest_payload(AgentKind::ClaudeCode, subj, s, false);
                *s = res.compressed_ahtl;
            }
            Value::Array(blocks) => {
                for (b_idx, block) in blocks.iter_mut().enumerate() {
                    if let Some(obj) = block.as_object_mut() {
                        obj.remove("cache_control");
                    }
                    let b_name = block
                        .get("name")
                        .or_else(|| block.get("tool_name"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    if InfinityContextMmu::is_icx_pager_tool(b_name) {
                        continue;
                    }
                    let block_subj = format!("{subj}_b{b_idx}");
                    if let Some(Value::String(t)) = block.get_mut("text") {
                        if t.len() >= threshold && !t.starts_with('⟦') && !t.starts_with("⟪ICX:")
                        {
                            let res =
                                mmu.ingest_payload(AgentKind::ClaudeCode, &block_subj, t, false);
                            *t = res.compressed_ahtl;
                        }
                    } else if let Some(Value::String(c)) = block.get_mut("content") {
                        if c.len() >= threshold && !c.starts_with('⟦') && !c.starts_with("⟪ICX:")
                        {
                            let res =
                                mmu.ingest_payload(AgentKind::ClaudeCode, &block_subj, c, false);
                            *c = res.compressed_ahtl;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn strip_existing_tail_slot(text: &str) -> &str {
        match text.find("\n⟪ICX:TAIL_SLOT│") {
            Some(idx) => &text[..idx],
            None => text,
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
            Self::transform_openai_items(messages, mmu, protect_tail);
        } else if let Some(input_val) = root.get_mut("input") {
            match input_val {
                Value::Array(items) => {
                    Self::transform_openai_items(items, mmu, protect_tail);
                }
                Value::String(prompt_str) => {
                    let base = Self::strip_existing_tail_slot(prompt_str).to_owned();
                    let tail_slot = mmu.build_ephemeral_tail_slot(AgentKind::Codex, &base, 3);
                    *prompt_str = format!("{base}\n{tail_slot}");
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
                for (idx, content_item) in contents[..fold_end].iter_mut().enumerate() {
                    if let Some(parts) = content_item.get_mut("parts").and_then(Value::as_array_mut)
                    {
                        for (p_idx, part) in parts.iter_mut().enumerate() {
                            if let Some(Value::String(t)) = part.get_mut("text")
                                && t.len() >= threshold
                                && !t.starts_with('⟦')
                                && !t.starts_with("⟪ICX:")
                            {
                                let subj = format!("gemini_turn_{idx}_p{p_idx}");
                                let res =
                                    mmu.ingest_payload(AgentKind::AntigravityCli, &subj, t, false);
                                *t = res.compressed_ahtl;
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
                    let base = Self::strip_existing_tail_slot(t);
                    *t = format!("{base}\n{tail_slot}");
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
                    Ok(aid) => {
                        if let Some(r_spec) = range_opt
                            && let Some((s_str, e_str)) = r_spec.split_once(':')
                            && let (Ok(s), Ok(e)) = (s_str.parse::<usize>(), e_str.parse::<usize>())
                        {
                            mmu.deref_lines(aid, s, e)
                                .unwrap_or_else(|err| format!("⟪ICX:DEREF_ERR│{err}⟫"))
                        } else {
                            mmu.deref_text(aid)
                                .unwrap_or_else(|err| format!("⟪ICX:DEREF_ERR│{err}⟫"))
                        }
                    }
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

    /// Transforms an inbound Server-Sent Events (`SSE`) chunk, expanding any inline
    /// `⟪!icx:deref(#icx:...)⟫` directives inside `data: {...}` JSON lines or plain text lines.
    #[must_use]
    pub fn transform_sse_chunk(sse_chunk: &str, mmu: &InfinityContextMmu) -> String {
        if !sse_chunk.contains("⟪!icx:deref(") {
            return sse_chunk.to_owned();
        }
        let mut out_lines = Vec::new();
        for line in sse_chunk.split('\n') {
            if let Some(json_part) = line.strip_prefix("data: ") {
                let trimmed = json_part.trim();
                if let Ok(mut val) = serde_json::from_str::<Value>(trimmed) {
                    Self::expand_deref_in_json_value(&mut val, mmu);
                    if let Ok(re_serialized) = serde_json::to_string(&val) {
                        out_lines.push(format!("data: {re_serialized}"));
                        continue;
                    }
                }
            }
            out_lines.push(Self::expand_inline_deref_directives(line, mmu));
        }
        out_lines.join("\n")
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

    /// Dispatches a JSON request body according to `format` (or auto-detects from the route/body).
    pub fn transform_for_provider(
        format: Option<ProviderFormat>,
        route_path: &str,
        body_json: &str,
        mmu: &mut InfinityContextMmu,
    ) -> Result<String, serde_json::Error> {
        let resolved = format
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
            });
        match resolved {
            ProviderFormat::Anthropic => Self::transform_anthropic_messages(body_json, mmu),
            ProviderFormat::OpenAI => Self::transform_openai_request(body_json, mmu),
            ProviderFormat::Gemini => Self::transform_gemini_request(body_json, mmu),
        }
    }

    /// Handles a single HTTP/1.1 request on `reader` and writes an HTTP/1.1 response to `writer`.
    /// Supports `/health` (`GET`), `/sse-expand` (`POST`), and `/anthropic/v1/messages`,
    /// `/openai/v1/responses`, `/openai/v1/chat/completions`, and `/gemini/v1beta/models/*` (`POST`).
    pub fn handle_http_request<R: Read, W: Write>(
        reader: R,
        mut writer: W,
        default_provider: Option<ProviderFormat>,
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
        loop {
            let mut header_line = String::new();
            let n = buf_reader.read_line(&mut header_line)?;
            if n == 0 || header_line == "\r\n" || header_line == "\n" {
                break;
            }
            if let Some((k, v)) = header_line.split_once(':')
                && k.trim().eq_ignore_ascii_case("content-length")
            {
                content_length = v.trim().parse::<usize>().unwrap_or(0).min(16 * 1024 * 1024);
            }
        }

        let mut body_bytes = vec![0u8; content_length];
        if content_length > 0 {
            buf_reader.read_exact(&mut body_bytes)?;
        }
        let body_str = String::from_utf8_lossy(&body_bytes);

        if method.eq_ignore_ascii_case("GET") && path.starts_with("/health") {
            let resp_body = format!(
                "{{\"status\":\"ok\",\"cas_artifacts\":{},\"sealed_epochs\":{}}}",
                mmu.cas.artifact_count(),
                mmu.epochs.sealed_epoch_count()
            );
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
            write!(
                writer,
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                expanded.len(),
                expanded
            )?;
            return writer.flush();
        }

        match Self::transform_for_provider(default_provider, path, &body_str, mmu) {
            Ok(transformed) => {
                write!(
                    writer,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    transformed.len(),
                    transformed
                )?;
            }
            Err(err) => {
                let err_json = json!({ "error": err.to_string() }).to_string();
                write!(
                    writer,
                    "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    err_json.len(),
                    err_json
                )?;
            }
        }
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
        if max_requests == Some(0) {
            return Ok(0);
        }
        let mut served = 0usize;
        for stream_res in listener.incoming() {
            let mut stream = stream_res?;
            let reader = stream.try_clone()?;
            Self::handle_http_request(reader, &mut stream, default_provider, mmu)?;
            served += 1;
            if max_requests.is_some_and(|max| served >= max) {
                break;
            }
        }
        Ok(served)
    }

    fn transform_openai_items(
        items: &mut [Value],
        mmu: &mut InfinityContextMmu,
        protect_tail: usize,
    ) {
        let total = items.len();
        if total > protect_tail {
            let fold_end = total - protect_tail;
            for (idx, item) in items[..fold_end].iter_mut().enumerate() {
                let role = item.get("role").and_then(Value::as_str).unwrap_or("");
                if role == "system" || role == "developer" {
                    continue;
                }
                let name = item.get("name").and_then(Value::as_str).unwrap_or("");
                if InfinityContextMmu::is_icx_pager_tool(name) {
                    continue;
                }
                let subj = format!("openai_turn_{idx}");
                Self::virtualize_item_text(item, &subj, mmu);
            }
        }

        let last_query = items
            .last()
            .and_then(Self::extract_item_text)
            .map(Self::strip_existing_tail_slot)
            .unwrap_or("openai_turn")
            .to_owned();
        let tail_slot = mmu.build_ephemeral_tail_slot(AgentKind::Codex, &last_query, 3);
        if let Some(last_item) = items.last_mut() {
            Self::append_tail_to_item(last_item, &tail_slot);
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

    fn virtualize_item_text(item: &mut Value, subj: &str, mmu: &mut InfinityContextMmu) {
        let threshold = mmu.tool_virtualize_threshold;
        if let Some(content_val) = item.get_mut("content") {
            match content_val {
                Value::String(s)
                    if s.len() >= threshold && !s.starts_with('⟦') && !s.starts_with("⟪ICX:") =>
                {
                    let res = mmu.ingest_payload(AgentKind::Codex, subj, s, false);
                    *s = res.compressed_ahtl;
                    return;
                }
                Value::Array(parts) => {
                    for (p_idx, part) in parts.iter_mut().enumerate() {
                        if let Some(Value::String(t)) = part.get_mut("text") {
                            if t.len() >= threshold
                                && !t.starts_with('⟦')
                                && !t.starts_with("⟪ICX:")
                            {
                                let part_subj = format!("{subj}_p{p_idx}");
                                let res =
                                    mmu.ingest_payload(AgentKind::Codex, &part_subj, t, false);
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
            if out_s.len() >= threshold && !out_s.starts_with('⟦') && !out_s.starts_with("⟪ICX:")
            {
                let res = mmu.ingest_payload(AgentKind::Codex, subj, out_s, false);
                *out_s = res.compressed_ahtl;
            }
        }
    }

    fn append_tail_to_item(item: &mut Value, tail_slot: &str) {
        if let Some(content_val) = item.get_mut("content") {
            match content_val {
                Value::String(s) => {
                    let base = Self::strip_existing_tail_slot(s);
                    *s = format!("{base}\n{tail_slot}");
                    return;
                }
                Value::Array(parts) => {
                    if let Some(last_part) = parts.last_mut() {
                        if let Some(Value::String(t)) = last_part.get_mut("text") {
                            let base = Self::strip_existing_tail_slot(t);
                            *t = format!("{base}\n{tail_slot}");
                            return;
                        }
                    }
                    parts.push(json!({ "type": "text", "text": tail_slot }));
                    return;
                }
                _ => {}
            }
        }
        if let Some(Value::String(out_s)) = item.get_mut("output") {
            let base = Self::strip_existing_tail_slot(out_s);
            *out_s = format!("{base}\n{tail_slot}");
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
    }
}
