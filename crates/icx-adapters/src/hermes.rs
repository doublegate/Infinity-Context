//! Hermes Agent `ContextEngine.select_context()` and `prune_tool_results_only()` adapter (`HermesAdapter`).

use crate::mmu::InfinityContextMmu;
use icx_core::AgentKind;
use serde::{Deserialize, Serialize};

/// OpenAI-compatible message entry passed by Hermes's `ContextEngine.select_context()`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HermesMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Adapter implementing Hermes's `ContextEngine` transformations without mutating `state.db`.
#[derive(Clone, Debug, Default)]
pub struct HermesAdapter;

impl HermesAdapter {
    /// Implements `ContextEngine.select_context(request_messages, protect_first_n, protect_last_n)`:
    /// 1. Keeps the first `protect_first_n` head messages (system prompt + initial goal) verbatim.
    /// 2. Compresses middle messages (especially bulky tool results) into sealed AHTL Epoch frames.
    /// 3. Keeps the last `protect_last_n` messages verbatim.
    /// 4. Appends the Ephemeral Tail Slot containing HDC/PPR/Fractal recall and `/dev/shm` telepathy.
    pub fn select_context(
        request_messages: &[HermesMessage],
        protect_first_n: usize,
        protect_last_n: usize,
        mmu: &mut InfinityContextMmu,
    ) -> Vec<HermesMessage> {
        let total = request_messages.len();
        if total <= protect_first_n + protect_last_n {
            let mut out = request_messages.to_vec();
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

        let mut selected = Vec::with_capacity(protect_first_n + protect_last_n + 2);
        selected.extend_from_slice(&request_messages[..head_end]);

        let mut middle_ahtl = Vec::new();
        for msg in &request_messages[head_end..tail_start] {
            let subj = msg.name.as_deref().unwrap_or(&msg.role);
            let res = mmu.ingest_payload(AgentKind::Hermes, subj, &msg.content, false);
            middle_ahtl.push(res.compressed_ahtl);
        }

        selected.push(HermesMessage {
            role: "system".into(),
            content: format!(
                "⟪ICX:HERMES_EPOCH│folded_msgs={}⟫\n{}",
                tail_start - head_end,
                middle_ahtl.join("\n")
            ),
            tool_call_id: None,
            name: Some("icx_cache_epoch".into()),
        });

        selected.extend_from_slice(&request_messages[tail_start..]);

        let last_query = request_messages
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
}
