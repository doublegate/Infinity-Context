//! Wire-level Cache-Epoch request transformer (`WireProxyTransformer`) for Anthropic
//! (`/v1/messages`) and OpenAI (`/v1/responses`, `/v1/chat/completions`) payloads.

use icx_adapters::InfinityContextMmu;
use icx_core::AgentKind;
use serde_json::{Value, json};

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
                    let role = msg.get("role").and_then(Value::as_str).unwrap_or("");
                    if role != "system" && role != "developer" {
                        if let Some(content_str) = msg.get("content").and_then(Value::as_str) {
                            if content_str.len() >= mmu.tool_virtualize_threshold
                                && !content_str.starts_with('⟦')
                                && !content_str.starts_with("⟪ICX:")
                            {
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

    /// Transforms an OpenAI `/v1/chat/completions` or `/v1/responses` JSON request body,
    /// preserving the 1,024-token automatic prefix cache while virtualizing oversized middle turns
    /// and appending the Ephemeral Tail Slot.
    pub fn transform_openai_request(
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
                    if role == "system" || role == "developer" {
                        continue;
                    }
                    if let Some(content_str) = msg.get("content").and_then(Value::as_str) {
                        if content_str.len() >= mmu.tool_virtualize_threshold
                            && !content_str.starts_with('⟦')
                            && !content_str.starts_with("⟪ICX:")
                        {
                            let subj = format!("openai_turn_{idx}");
                            let res =
                                mmu.ingest_payload(AgentKind::Codex, &subj, content_str, false);
                            msg["content"] = Value::String(res.compressed_ahtl);
                        }
                    }
                }
            }

            let last_query = messages
                .last()
                .and_then(|m| m.get("content"))
                .and_then(Value::as_str)
                .unwrap_or("openai_turn")
                .to_owned();
            let tail_slot = mmu.build_ephemeral_tail_slot(AgentKind::Codex, &last_query, 3);
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
    }
}
