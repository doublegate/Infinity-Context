// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later AND AGPL-3.0-or-later

//! Provider-reported prompt-cache counters (T-81-003): read from proxied responses, JSON or SSE,
//! so cache behaviour is measured from the provider instead of inferred from byte reuse. A field
//! the provider did not send stays `None`: absent is never recorded as zero, and nothing is
//! estimated.
//!
//! Field names (checked against the provider docs on 2026-11-26):
//! - Anthropic Messages: `usage.cache_read_input_tokens`, `usage.cache_creation_input_tokens`,
//!   `usage.cache_creation.{ephemeral_5m_input_tokens, ephemeral_1h_input_tokens}`, and
//!   `usage.input_tokens`, in a JSON body or in the SSE `message_start` (`message.usage`) and
//!   `message_delta` (`usage`) events.
//! - OpenAI Chat Completions: `usage.prompt_tokens_details.{cached_tokens, cache_write_tokens}`
//!   and `usage.prompt_tokens`, in a JSON body or the final SSE chunk.
//! - OpenAI Responses: `usage.input_tokens_details.{cached_tokens, cache_write_tokens,
//!   cache_write_tokens_details.{ttl_30m_tokens, ttl_12h_tokens}}` and `usage.input_tokens`,
//!   in a JSON body or the `response.completed` event (`response.usage`).

use serde_json::Value;

/// The cache counters of one proxied response.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CacheCounters {
    /// `anthropic` or `openai`, from the shape of the usage object; `None` when none was seen.
    pub provider: Option<&'static str>,
    /// Tokens read from the cache.
    pub cache_read: Option<u64>,
    /// Tokens written to the cache.
    pub cache_write: Option<u64>,
    /// Anthropic: tokens written with the 5-minute TTL.
    pub write_5m: Option<u64>,
    /// Anthropic: tokens written with the 1-hour TTL.
    pub write_1h: Option<u64>,
    /// OpenAI Responses: tokens written with the 30-minute TTL.
    pub write_30m: Option<u64>,
    /// OpenAI Responses: tokens written with the 12-hour TTL.
    pub write_12h: Option<u64>,
    /// Uncached input tokens (Anthropic) or all prompt tokens (OpenAI), as the provider counts.
    pub input: Option<u64>,
    /// Anthropic cache diagnostics (beta `cache-diagnosis-2026-04-07`): the names of the
    /// `diagnostics` fields the provider returned, without their values, so no request content
    /// reaches the ledger (T-81-002).
    pub diagnostics: Option<Vec<String>>,
}

impl CacheCounters {
    /// Whether any usage object was seen.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.provider.is_none()
    }

    /// Reads the usage object of one JSON value (a response body or one SSE `data:` payload).
    /// Later values override earlier ones field by field, so a stream's final usage wins while
    /// a field only an earlier event carried is kept.
    pub fn observe(&mut self, value: &Value) {
        for usage in usage_objects(value) {
            self.observe_usage(usage);
        }
        let diag = value
            .get("diagnostics")
            .or_else(|| value.get("message").and_then(|m| m.get("diagnostics")));
        if let Some(Value::Object(d)) = diag {
            self.diagnostics = Some(d.keys().take(16).cloned().collect());
        }
    }

    /// Reads one SSE `data:` line or a JSON body; text that is not JSON is ignored.
    pub fn observe_text(&mut self, text: &str) {
        let t = text.trim();
        let t = t.strip_prefix("data:").map_or(t, str::trim_start);
        if !(t.starts_with('{') && (t.contains("usage") || t.contains("diagnostics"))) {
            return;
        }
        if let Ok(v) = serde_json::from_str::<Value>(t) {
            self.observe(&v);
        }
    }

    fn observe_usage(&mut self, u: &Value) {
        let get = |v: &Value, k: &str| v.get(k).and_then(Value::as_u64);
        if u.get("cache_read_input_tokens").is_some()
            || u.get("cache_creation_input_tokens").is_some()
        {
            self.provider = Some("anthropic");
            set(&mut self.cache_read, get(u, "cache_read_input_tokens"));
            set(&mut self.cache_write, get(u, "cache_creation_input_tokens"));
            if let Some(c) = u.get("cache_creation") {
                set(&mut self.write_5m, get(c, "ephemeral_5m_input_tokens"));
                set(&mut self.write_1h, get(c, "ephemeral_1h_input_tokens"));
            }
            set(&mut self.input, get(u, "input_tokens"));
            return;
        }
        let details = u
            .get("prompt_tokens_details")
            .or_else(|| u.get("input_tokens_details"));
        if let Some(d) = details {
            self.provider = Some("openai");
            set(&mut self.cache_read, get(d, "cached_tokens"));
            set(&mut self.cache_write, get(d, "cache_write_tokens"));
            if let Some(t) = d.get("cache_write_tokens_details") {
                set(&mut self.write_30m, get(t, "ttl_30m_tokens"));
                set(&mut self.write_12h, get(t, "ttl_12h_tokens"));
            }
            set(
                &mut self.input,
                get(u, "prompt_tokens").or_else(|| get(u, "input_tokens")),
            );
        }
    }

    /// One JSON line for the usage ledger, with absent fields written as `null`.
    #[must_use]
    pub fn to_json_line(&self, epoch: u64, unix_ms: u128) -> String {
        serde_json::json!({
            "unix_ms": unix_ms,
            "epoch": epoch,
            "provider": self.provider,
            "cache_read_tokens": self.cache_read,
            "cache_write_tokens": self.cache_write,
            "cache_write_5m_tokens": self.write_5m,
            "cache_write_1h_tokens": self.write_1h,
            "cache_write_30m_tokens": self.write_30m,
            "cache_write_12h_tokens": self.write_12h,
            "input_tokens": self.input,
            "diagnostics_fields": self.diagnostics,
        })
        .to_string()
    }
}

/// A later present value replaces the earlier one; an absent one leaves it.
fn set(slot: &mut Option<u64>, value: Option<u64>) {
    if value.is_some() {
        *slot = value;
    }
}

/// The `usage` objects of a value: top-level, under `message` (Anthropic `message_start`) and
/// under `response` (OpenAI `response.completed`).
fn usage_objects(v: &Value) -> impl Iterator<Item = &Value> {
    [
        v.get("usage"),
        v.get("message").and_then(|m| m.get("usage")),
        v.get("response").and_then(|r| r.get("usage")),
    ]
    .into_iter()
    .flatten()
    .filter(|u| u.is_object())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anthropic_json_and_sse() {
        let mut c = CacheCounters::default();
        c.observe_text(
            r#"{"usage":{"input_tokens":12,"cache_read_input_tokens":4000,"cache_creation_input_tokens":300,"cache_creation":{"ephemeral_5m_input_tokens":300,"ephemeral_1h_input_tokens":0}}}"#,
        );
        assert_eq!(c.provider, Some("anthropic"));
        assert_eq!(c.cache_read, Some(4000));
        assert_eq!(c.cache_write, Some(300));
        assert_eq!(c.write_5m, Some(300));
        assert_eq!(c.write_1h, Some(0));
        assert_eq!(c.input, Some(12));

        let mut s = CacheCounters::default();
        s.observe_text(r#"data: {"type":"message_start","message":{"usage":{"input_tokens":5,"cache_read_input_tokens":100,"cache_creation_input_tokens":0}}}"#);
        s.observe_text(r#"data: {"type":"message_delta","usage":{"output_tokens":20}}"#);
        assert_eq!(s.cache_read, Some(100));
        assert_eq!(s.input, Some(5));
    }

    #[test]
    fn openai_chat_and_responses() {
        let mut c = CacheCounters::default();
        c.observe_text(
            r#"data: {"choices":[],"usage":{"prompt_tokens":2048,"prompt_tokens_details":{"cached_tokens":1920,"cache_write_tokens":128}}}"#,
        );
        assert_eq!(c.provider, Some("openai"));
        assert_eq!(
            (c.cache_read, c.cache_write, c.input),
            (Some(1920), Some(128), Some(2048))
        );

        let mut r = CacheCounters::default();
        r.observe_text(r#"data: {"type":"response.completed","response":{"usage":{"input_tokens":3000,"input_tokens_details":{"cached_tokens":2500,"cache_write_tokens":500,"cache_write_tokens_details":{"ttl_30m_tokens":100,"ttl_12h_tokens":400}}}}}"#);
        assert_eq!(r.write_30m, Some(100));
        assert_eq!(r.write_12h, Some(400));
        assert_eq!(r.cache_read, Some(2500));
    }

    #[test]
    fn diagnostics_fields_are_named_without_values() {
        let mut c = CacheCounters::default();
        c.observe_text(r#"{"usage":{"cache_read_input_tokens":0,"cache_creation_input_tokens":900},"diagnostics":{"divergence":"system","detail":"request text here"}}"#);
        let line = c.to_json_line(1, 1);
        assert!(
            line.contains(r#""diagnostics_fields":["detail","divergence"]"#),
            "{line}"
        );
        assert!(!line.contains("request text here"), "{line}");
    }

    #[test]
    fn absent_and_malformed_fields_stay_absent() {
        let mut c = CacheCounters::default();
        // No cache fields: not a cache-usage object at all.
        c.observe_text(r#"{"usage":{"input_tokens":10,"output_tokens":3}}"#);
        assert!(c.is_empty());
        // Present key, malformed value: the counter stays absent, never zero.
        c.observe_text(
            r#"{"usage":{"cache_read_input_tokens":"many","cache_creation_input_tokens":null}}"#,
        );
        assert_eq!(c.provider, Some("anthropic"));
        assert_eq!(c.cache_read, None);
        assert_eq!(c.cache_write, None);
        // A `usage` that is not an object, and invalid JSON, are ignored.
        let mut d = CacheCounters::default();
        d.observe_text(r#"{"usage":"none"}"#);
        d.observe_text(r#"{"usage": {"cache_read_input_tokens": 1"#);
        assert!(d.is_empty());
        // Text that is not JSON, and the stream terminator, are ignored.
        c.observe_text("data: [DONE]");
        c.observe_text("not json usage");
        let line = c.to_json_line(7, 1);
        assert!(line.contains(r#""cache_read_tokens":null"#), "{line}");
        assert!(line.contains(r#""epoch":7"#), "{line}");
    }
}
