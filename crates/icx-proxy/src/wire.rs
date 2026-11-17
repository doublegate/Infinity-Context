//! Wire-level Cache-Epoch request transformer (`WireProxyTransformer`) for Anthropic
//! (`/v1/messages`) and OpenAI (`/v1/responses`, `/v1/chat/completions`) payloads.

use icx_adapters::InfinityContextMmu;
use icx_core::AgentKind;
use serde_json::{json, Value};

/// Transforms outbound LLM API requests to enforce Prefix-Cache-Locked Epochs (`arXiv:2610.28577`)
/// and inject the Ephemeral Tail Slot without invalidating provider KV caches.
#[derive(Clone, Debug, Default)]
pub struct WireProxyTransformer;

impl WireProxyTransformer {
    /// Transforms an Anthropic `/v1/messages` request JSON body:
    /// 1. Pins `cache_control: {"type": "ephemeral"}` on the sealed Cache Epoch block.
    /// 2. Compresses oversized middle-history messages into deterministic AHTL stubs.
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
                    if let Some(content_str) = msg.get("content").and_then(Value::as_str) {
                        if content_str.len() >= mmu.tool_virtualize_threshold {
                            let subj = format!("turn_{idx}");
                            let res = mmu.ingest_payload(
                                AgentKind::ClaudeCode,
                                &subj,
                                content_str,
                                false,
                            );
                            msg["content"] = Value::String(res.compressed_ahtl);
                        }
                    }
                    // Pin Anthropic prompt-cache breakpoint on the last sealed epoch message
                    if idx + 1 == fold_end {
                        msg["cache_control"] = json!({ "type": "ephemeral" });
                    }
                }
            }

            // Append Ephemeral Tail Slot to the final message
            let last_query = messages
                .last()
                .and_then(|m| m.get("content"))
                .and_then(Value::as_str)
                .unwrap_or("anthropic_turn")
                .to_owned();
            let tail_slot = mmu.build_ephemeral_tail_slot(AgentKind::ClaudeCode, &last_query, 3);
            if let Some(last_msg) = messages.last_mut() {
                if let Some(orig) = last_msg.get("content").and_then(Value::as_str) {
                    last_msg["content"] = Value::String(format!("{orig}\n{tail_slot}"));
                }
            }
        }

        serde_json::to_string(&root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anthropic_wire_transformer_injects_cache_control_and_virtualizes_middle_history() {
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
    }
}
