//! Single-Tool Polymorphic MCP Pager (`McpPagerServer`).
//!
//! Exposes the entire Infinity-Context MMU syscall surface (`deref`, `slice`, `grep`, `recall`,
//! `fold`, `expand`, `xagent`, `status`) behind a single `< 180 token` MCP tool named `icx`.

use crate::mmu::InfinityContextMmu;
use icx_core::{AgentKind, IcxError};
use serde_json::{Value, json};

/// Single-tool polymorphic Model Context Protocol (`MCP`) server.
#[derive(Clone, Debug, Default)]
pub struct McpPagerServer;

impl McpPagerServer {
    /// Returns the compact single-tool `tools/list` schema payload.
    pub fn tool_schema() -> Value {
        json!({
            "tools": [
                {
                    "name": "icx",
                    "description": "Infinity-Context MMU syscall: deref #icx:<id>, slice lines, grep cold CAS, associative recall, fold/expand branch, or poll/broadcast /dev/shm bus.",
                    "inputSchema": {
                        "type": "object",
                        "required": ["op"],
                        "properties": {
                            "op": {
                                "type": "string",
                                "enum": ["deref", "slice", "grep", "recall", "fold", "expand", "xagent", "status"]
                            },
                            "id": { "type": "string" },
                            "q": {
                                "type": "string",
                                "description": "Search pattern for grep/recall, 'begin:<label>' or '<summary>' for fold, or message for xagent."
                            },
                            "range": {
                                "type": "array",
                                "items": { "type": "integer" }
                            },
                            "provenance": { "type": "boolean" }
                        }
                    }
                }
            ]
        })
    }

    /// Estimates the BPE token footprint (`(chars + 3) / 4`) of `tool_schema()`, guaranteed `< 180` tokens.
    #[must_use]
    pub fn schema_token_estimate() -> usize {
        Self::tool_schema().to_string().len().div_ceil(4)
    }

    pub(crate) fn format_error(err: &IcxError) -> String {
        match err {
            IcxError::AmbiguousHandlePrefix(p) => {
                format!("ERR: {err} (retry with full 16-hex handle #icx:<16-hex> for prefix `{p}`)")
            }
            _ => format!("ERR: {err}"),
        }
    }

    fn attach_provenance(
        text: String,
        aid: icx_core::ArtifactId,
        args: &Value,
        mmu: &InfinityContextMmu,
    ) -> String {
        let want_prov = args
            .get("provenance")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let is_redacted = mmu.is_artifact_redacted(aid);
        if want_prov || is_redacted {
            format!(
                "⟪ICX:PROVENANCE│id={aid}│redacted={is_redacted}│exactness=post_sanitization_cas⟫\n{text}"
            )
        } else {
            text
        }
    }

    /// Processes a single JSON-RPC 2.0 MCP request line and returns a JSON-RPC 2.0 response.
    pub fn handle_jsonrpc(request_json: &str, mmu: &mut InfinityContextMmu) -> Value {
        let req: Value = match serde_json::from_str(request_json) {
            Ok(v) => v,
            Err(e) => {
                return json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": format!("Parse error: {e}") }
                });
            }
        };

        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req.get("method").and_then(Value::as_str).unwrap_or("");

        match method {
            "initialize" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": "infinity-context-mmu",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }
            }),
            "notifications/initialized" | "ping" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {}
            }),
            "tools/list" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": Self::tool_schema()
            }),
            "tools/call" => {
                let args = req
                    .get("params")
                    .and_then(|p| p.get("arguments"))
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                let result_text = Self::execute_icx_op(&args, mmu);
                let is_err = result_text.starts_with("ERR:");
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "isError": is_err,
                        "content": [
                            {
                                "type": "text",
                                "text": result_text
                            }
                        ]
                    }
                })
            }
            _ => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("Method not found: {method}") }
            }),
        }
    }

    /// Executes the polymorphic `icx` tool operation.
    pub fn execute_icx_op(args: &Value, mmu: &mut InfinityContextMmu) -> String {
        let op = args.get("op").and_then(Value::as_str).unwrap_or("status");

        match op {
            "deref" | "expand" => {
                let Some(id_str) = args.get("id").and_then(Value::as_str) else {
                    return "ERR: missing `id` (#icx:<16-hex> or #icx:<12-hex>)".into();
                };
                match mmu.resolve_handle(id_str) {
                    Ok(aid) => match mmu.deref_text(aid) {
                        Ok(text) => Self::attach_provenance(text, aid, args, mmu),
                        Err(e) => Self::format_error(&e),
                    },
                    Err(e) => Self::format_error(&e),
                }
            }
            "slice" => {
                let Some(id_str) = args.get("id").and_then(Value::as_str) else {
                    return "ERR: missing `id` (#icx:<16-hex> or #icx:<12-hex>)".into();
                };
                let range = args
                    .get("range")
                    .and_then(Value::as_array)
                    .map(|arr| {
                        let s = arr.first().and_then(Value::as_u64).unwrap_or(1) as usize;
                        let e = arr.get(1).and_then(Value::as_u64).unwrap_or(s as u64) as usize;
                        (s, e)
                    })
                    .unwrap_or((1, 50));

                match mmu.resolve_handle(id_str) {
                    Ok(aid) => match mmu.deref_lines(aid, range.0, range.1) {
                        Ok(text) => Self::attach_provenance(text, aid, args, mmu),
                        Err(e) => Self::format_error(&e),
                    },
                    Err(e) => Self::format_error(&e),
                }
            }
            "grep" => {
                let pattern = args.get("q").and_then(Value::as_str).unwrap_or("");
                let target_id = match args
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|s| !s.trim().is_empty())
                {
                    Some(s) => match mmu.cas.resolve_handle(s) {
                        Ok(aid) => Some(aid),
                        Err(e) => return Self::format_error(&e),
                    },
                    None => None,
                };
                match mmu.grep_cold(target_id, pattern, 25) {
                    Ok(matches) => matches
                        .iter()
                        .map(|m| format!("{}#L{}: {}", m.artifact_id, m.line_number, m.line_text))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    Err(e) => Self::format_error(&e),
                }
            }
            "recall" => {
                let q = args.get("q").and_then(Value::as_str).unwrap_or("");
                mmu.build_ephemeral_tail_slot(AgentKind::GenericMcp, q, 5)
            }
            "fold" => {
                let summary = args.get("q").and_then(Value::as_str).unwrap_or("completed");
                if let Some(label) = summary.strip_prefix("begin:") {
                    let clean_label = InfinityContextMmu::sanitize_hook_label(label, "branch");
                    mmu.epochs.begin_branch(&mut mmu.dag, &clean_label);
                    format!("⟪ICX:BRANCH_OPENED│{clean_label}⟫")
                } else {
                    let clean_summary =
                        InfinityContextMmu::sanitize_hook_label(summary, "completed");
                    match mmu.epochs.fold_branch(&mut mmu.dag, &clean_summary) {
                        Ok(bid) => {
                            mmu.sync_global_dag_root();
                            format!("⟪ICX:BRANCH_FOLDED│{bid}⟫")
                        }
                        Err(e) => Self::format_error(&e),
                    }
                }
            }
            "xagent" => {
                if let Some(broadcast_q) = args
                    .get("q")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    let subj = args
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .unwrap_or("xagent_broadcast");
                    let res = mmu.ingest_payload(AgentKind::GenericMcp, subj, broadcast_q, false);
                    return format!(
                        "⟪ICX:XAGENT_PUBLISHED│{}│seq={}⟫\n{}",
                        res.artifact_id,
                        mmu.bus.write_sequence(),
                        res.compressed_ahtl
                    );
                }
                let events = mmu.bus.poll_since(0);
                events
                    .iter()
                    .rev()
                    .take(10)
                    .map(|e| e.ahtl_packet.clone())
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            _ => {
                if let Some(id_str) = args
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|s| !s.trim().is_empty())
                {
                    match mmu.resolve_handle(id_str) {
                        Ok(aid) => format!(
                            "⟪ICX:STATUS│id={aid}│redacted={}│cas_artifacts={}│unique_chunks={}│dag_nodes={}│sealed_epochs={}│hdc_entries={}│graph_edges={}│shm_seq={}⟫",
                            mmu.is_artifact_redacted(aid),
                            mmu.cas.artifact_count(),
                            mmu.cas.unique_chunk_count(),
                            mmu.dag.node_count(),
                            mmu.epochs.sealed_epoch_count(),
                            mmu.hdc_index.len(),
                            mmu.graph.active_edge_count(),
                            mmu.bus.write_sequence()
                        ),
                        Err(e) => Self::format_error(&e),
                    }
                } else {
                    format!(
                        "⟪ICX:STATUS│cas_artifacts={}│redacted_artifacts={}│unique_chunks={}│dag_nodes={}│sealed_epochs={}│hdc_entries={}│graph_edges={}│shm_seq={}⟫",
                        mmu.cas.artifact_count(),
                        mmu.redacted_artifact_count(),
                        mmu.cas.unique_chunk_count(),
                        mmu.dag.node_count(),
                        mmu.epochs.sealed_epoch_count(),
                        mmu.hdc_index.len(),
                        mmu.graph.active_edge_count(),
                        mmu.bus.write_sequence()
                    )
                }
            }
        }
    }
}
