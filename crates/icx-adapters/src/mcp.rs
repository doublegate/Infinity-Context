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
                            "k": { "type": "integer" },
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

    /// Returns `true` if a JSON-RPC 2.0 request line requires reading or mutating persistent MMU
    /// workspace state (`id` is present and non-null, and `method == "tools/call"`).
    #[must_use]
    pub fn requires_workspace_state(request_json: &str) -> bool {
        serde_json::from_str::<Value>(request_json).is_ok_and(|req| {
            !req.get("id").is_none_or(Value::is_null)
                && req.get("method").and_then(Value::as_str) == Some("tools/call")
                && req
                    .get("params")
                    .and_then(|p| p.get("name"))
                    .and_then(Value::as_str)
                    .is_none_or(InfinityContextMmu::is_icx_pager_tool)
        })
    }

    /// Returns `true` if a JSON-RPC 2.0 request line mutates persistent MMU state (`fold`,
    /// `recall`, or `xagent` with a non-empty `q` broadcast).
    #[must_use]
    pub fn is_mutating_request(request_json: &str) -> bool {
        let Ok(req) = serde_json::from_str::<Value>(request_json) else {
            return false;
        };
        if req.get("id").is_none_or(Value::is_null)
            || req.get("method").and_then(Value::as_str) != Some("tools/call")
        {
            return false;
        }
        let Some(args) = req.get("params").and_then(|p| p.get("arguments")) else {
            return false;
        };
        match args.get("op").and_then(Value::as_str).unwrap_or("status") {
            "fold" | "recall" => true,
            "xagent" => args
                .get("q")
                .and_then(Value::as_str)
                .is_some_and(|s| !s.trim().is_empty()),
            _ => false,
        }
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
        let exactness = if mmu.is_cas_artifact(aid) {
            "post_sanitization_cas"
        } else {
            "merkle_dag_representation"
        };
        if want_prov || is_redacted {
            format!(
                "⟪ICX:PROVENANCE│id={aid}│redacted={is_redacted}│exactness={exactness}⟫\n{text}"
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

        let has_id = !req.get("id").is_none_or(Value::is_null);
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req.get("method").and_then(Value::as_str).unwrap_or("");

        if !has_id || method.starts_with("notifications/") {
            return Value::Null;
        }

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
            "ping" => json!({
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
                let tool_name = req
                    .get("params")
                    .and_then(|p| p.get("name"))
                    .and_then(Value::as_str);
                let (result_text, is_err) = if let Some(name) = tool_name
                    && !InfinityContextMmu::is_icx_pager_tool(name)
                {
                    (format!("ERR: unknown tool `{name}` (expected `icx`)"), true)
                } else {
                    let args = req
                        .get("params")
                        .and_then(|p| p.get("arguments"))
                        .cloned()
                        .unwrap_or_else(|| json!({}));
                    Self::execute_icx_op_checked(&args, mmu)
                };
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

    /// Executes the polymorphic `icx` tool operation and returns the formatted output text.
    pub fn execute_icx_op(args: &Value, mmu: &mut InfinityContextMmu) -> String {
        Self::execute_icx_op_checked(args, mmu).0
    }

    /// Executes the polymorphic `icx` tool operation and returns `(output_text, is_error)`.
    ///
    /// Result count `k` is clamped per operation (`1..=250` for `grep`, `1..=50` for `recall`,
    /// and `1..=256` for `xagent`).
    pub fn execute_icx_op_checked(args: &Value, mmu: &mut InfinityContextMmu) -> (String, bool) {
        let op = args.get("op").and_then(Value::as_str).unwrap_or("status");

        match op {
            "deref" | "expand" => {
                let Some(id_str) = args.get("id").and_then(Value::as_str) else {
                    return (
                        "ERR: missing `id` (#icx:<16-hex> or #icx:<12-hex>)".into(),
                        true,
                    );
                };
                match mmu.resolve_handle(id_str) {
                    Ok(aid) => match mmu.deref_text(aid) {
                        Ok(text) => (Self::attach_provenance(text, aid, args, mmu), false),
                        Err(e) => (Self::format_error(&e), true),
                    },
                    Err(e) => (Self::format_error(&e), true),
                }
            }
            "slice" => {
                let Some(id_str) = args.get("id").and_then(Value::as_str) else {
                    return (
                        "ERR: missing `id` (#icx:<16-hex> or #icx:<12-hex>)".into(),
                        true,
                    );
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
                        Ok(text) => (Self::attach_provenance(text, aid, args, mmu), false),
                        Err(e) => (Self::format_error(&e), true),
                    },
                    Err(e) => (Self::format_error(&e), true),
                }
            }
            "grep" => {
                let pattern = args.get("q").and_then(Value::as_str).unwrap_or("");
                let limit = args
                    .get("k")
                    .and_then(Value::as_u64)
                    .unwrap_or(25)
                    .clamp(1, 250) as usize;
                let target_id = match args
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|s| !s.trim().is_empty())
                {
                    Some(s) => match mmu.resolve_handle(s) {
                        Ok(aid) => Some(aid),
                        Err(e) => return (Self::format_error(&e), true),
                    },
                    None => None,
                };
                match mmu.grep_cold(target_id, pattern, limit) {
                    Ok(matches) => (
                        matches
                            .iter()
                            .map(|m| {
                                format!("{}#L{}: {}", m.artifact_id, m.line_number, m.line_text)
                            })
                            .collect::<Vec<_>>()
                            .join("\n"),
                        false,
                    ),
                    Err(e) => (Self::format_error(&e), true),
                }
            }
            "recall" => {
                let q = args.get("q").and_then(Value::as_str).unwrap_or("");
                let top_k = args
                    .get("k")
                    .and_then(Value::as_u64)
                    .unwrap_or(5)
                    .clamp(1, 50) as usize;
                (
                    mmu.build_ephemeral_tail_slot(AgentKind::GenericMcp, q, top_k),
                    false,
                )
            }
            "fold" => {
                let summary = args.get("q").and_then(Value::as_str).unwrap_or("completed");
                if let Some(label) = summary.strip_prefix("begin:") {
                    let clean_label = InfinityContextMmu::sanitize_hook_label(label, "branch");
                    mmu.epochs.begin_branch(&mut mmu.dag, &clean_label);
                    (format!("⟪ICX:BRANCH_OPENED│{clean_label}⟫"), false)
                } else {
                    let clean_summary =
                        InfinityContextMmu::sanitize_hook_label(summary, "completed");
                    match mmu.epochs.fold_branch(&mut mmu.dag, &clean_summary) {
                        Ok(bid) => {
                            mmu.sync_global_dag_root();
                            (format!("⟪ICX:BRANCH_FOLDED│{bid}⟫"), false)
                        }
                        Err(e) => (Self::format_error(&e), true),
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
                    return (
                        format!(
                            "⟪ICX:XAGENT_PUBLISHED│{}│seq={}⟫\n{}",
                            res.artifact_id,
                            mmu.bus.write_sequence(),
                            res.compressed_ahtl
                        ),
                        false,
                    );
                }
                let limit = args
                    .get("k")
                    .and_then(Value::as_u64)
                    .unwrap_or(10)
                    .clamp(1, 256) as usize;
                let events = mmu.bus.poll_workspace_since(0, mmu.workspace_hash);
                (
                    events
                        .iter()
                        .rev()
                        .take(limit)
                        .map(|e| e.ahtl_packet.clone())
                        .collect::<Vec<_>>()
                        .join("\n"),
                    false,
                )
            }
            _ => {
                if let Some(id_str) = args
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|s| !s.trim().is_empty())
                {
                    match mmu.resolve_handle(id_str) {
                        Ok(aid) => (
                            format!(
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
                            false,
                        ),
                        Err(e) => (Self::format_error(&e), true),
                    }
                } else {
                    (
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
                        ),
                        false,
                    )
                }
            }
        }
    }
}
