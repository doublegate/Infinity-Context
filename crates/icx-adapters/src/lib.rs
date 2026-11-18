//! `icx-adapters` — Cross-agent low-level lifecycle hook, `ContextEngine`, and Single-Tool
//! Polymorphic MCP Pager adapters for Claude Code, OpenAI Codex CLI, Google Antigravity CLI (`agy`),
//! and Hermes Agent.

pub mod antigravity;
pub mod claude;
pub mod codex;
pub mod hermes;
pub mod mcp;
pub mod mmu;

pub use antigravity::AntigravityAdapter;
pub use claude::ClaudeCodeAdapter;
pub use codex::{CodexAdapter, CodexRolloutTailer};
pub use hermes::{HermesAdapter, HermesMessage};
pub use mcp::McpPagerServer;
pub use mmu::{
    DEFAULT_TOOL_VIRTUALIZE_THRESHOLD_BYTES, ICX_AHTL_LEGEND, InfinityContextMmu, IngestResult,
};

use icx_core::{AgentKind, IcxError};
use thiserror::Error;

/// Error type returned by `AgentHookAdapter::handle_hook_event` (`docs/interfaces.md` §4).
#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("hook JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("core MMU error: {0}")]
    Icx(#[from] IcxError),
    #[error("adapter I/O error: {0}")]
    Io(String),
}

/// Normative trait implemented by all agent lifecycle hook adapters (`docs/interfaces.md` §4).
pub trait AgentHookAdapter: Send + Sync {
    /// Identifies the target agent harness.
    fn agent_kind(&self) -> AgentKind;

    /// Processes a raw JSON hook payload from `stdin` and produces the exact wire JSON response for `stdout`.
    fn handle_hook_event(
        &self,
        event_name: &str,
        stdin_json: &str,
        mmu: &mut InfinityContextMmu,
    ) -> Result<String, AdapterError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use icx_core::AgentKind;
    use serde_json::Value;

    #[test]
    fn cross_agent_telepathy_and_lossless_mcp_deref_work_end_to_end() {
        let mut mmu = InfinityContextMmu::default();

        // 1. Claude Code executes a Bash command with bulky output (> 512 bytes) containing a panic
        let mut bulky_output = String::from("running 120 tests\n");
        for i in 0..80 {
            bulky_output.push_str(&format!("test suite::case_{i:03} ... ok\n"));
        }
        bulky_output.push_str(
            "thread 'suite::page_table' panicked at crates/mmu/src/page.rs:314:5: PASSKEY_HEX_0xDEADBEEF42\n",
        );

        let claude_stdin = serde_json::json!({
            "tool_name": "Bash",
            "tool_response": bulky_output
        })
        .to_string();

        let claude_stdout =
            ClaudeCodeAdapter::handle_hook("PostToolUse", &claude_stdin, &mut mmu).unwrap();
        let claude_val: Value = serde_json::from_str(&claude_stdout).unwrap();
        let stub = claude_val["hookSpecificOutput"]["updatedToolOutput"]
            .as_str()
            .expect("updatedToolOutput must be present");
        assert!(stub.contains("⟪ICX:STUB│#icx:"));
        assert!(stub.contains("PASSKEY_HEX_0xDEADBEEF42"));

        // Extract the `#icx:<16-hex>` handle from the stub header
        let handle_start = stub.find("#icx:").unwrap();
        let handle_str = &stub[handle_start..handle_start + 21];

        // 2. Antigravity CLI (`agy`) fires `PreInvocation` and automatically receives Claude Code's
        // telepathy broadcast (`⟦Θ:claude-code@Bash...⟧`) inside a UI-silent `systemMessage`!
        let agy_stdin = serde_json::json!({
            "conversationId": "conv-123",
            "lastUserInput": "Check the page_table panic in crates/mmu/src/page.rs"
        })
        .to_string();
        let agy_stdout =
            AntigravityAdapter::handle_hook("PreInvocation", &agy_stdin, &mut mmu).unwrap();
        let agy_val: Value = serde_json::from_str(&agy_stdout).unwrap();
        let sys_msg = agy_val["injectSteps"][0]["systemMessage"]["systemMessage"]
            .as_str()
            .unwrap();
        let user_md = agy_val["injectSteps"][0]["systemMessage"]["metadata"]["user_markdown"]
            .as_str()
            .unwrap();
        assert_eq!(
            user_md, "",
            "UI user_markdown must be empty so AHTL stays hidden"
        );
        assert!(
            sys_msg.contains("⟦Θ:claude-code@Bash"),
            "Antigravity must receive Claude Code's telepathy frame"
        );

        // 3. Codex CLI invokes the Single-Tool Polymorphic MCP Pager (`icx` op="deref") using `handle_str`
        // and recovers the 100% byte-identical raw output!
        let mcp_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "icx",
                "arguments": {
                    "op": "deref",
                    "id": handle_str
                }
            }
        })
        .to_string();
        let mcp_resp = McpPagerServer::handle_jsonrpc(&mcp_req, &mut mmu);
        let deref_text = mcp_resp["result"]["content"][0]["text"].as_str().unwrap();
        assert_eq!(deref_text, bulky_output.trim_end_matches('\n'));

        // 4. Verify MCP `slice` and `grep` work with both 16-hex and 12-hex Profile A handles
        let short_handle = &handle_str[..17]; // `#icx:<12-hex>`
        let slice_out = McpPagerServer::execute_icx_op(
            &serde_json::json!({
                "op": "slice",
                "id": short_handle,
                "range": [1, 2]
            }),
            &mut mmu,
        );
        assert!(slice_out.contains("running 120 tests"));

        let grep_out = McpPagerServer::execute_icx_op(
            &serde_json::json!({
                "op": "grep",
                "id": short_handle,
                "q": "PASSKEY_HEX_0xDEADBEEF42"
            }),
            &mut mmu,
        );
        assert!(grep_out.contains("PASSKEY_HEX_0xDEADBEEF42"));

        // 4b. Verify MCP branch open (`begin:<label>`), fold, and DAG-handle `deref`/`slice`
        let open_out = McpPagerServer::execute_icx_op(
            &serde_json::json!({
                "op": "fold",
                "q": "begin:subtask_loop"
            }),
            &mut mmu,
        );
        assert!(open_out.contains("⟪ICX:BRANCH_OPENED│subtask_loop⟫"));
        mmu.ingest_payload(
            AgentKind::Codex,
            "cargo_check",
            "checking subtask step 1\n",
            false,
        );
        let fold_out = McpPagerServer::execute_icx_op(
            &serde_json::json!({
                "op": "fold",
                "q": "subtask_done"
            }),
            &mut mmu,
        );
        assert!(fold_out.contains("⟪ICX:BRANCH_FOLDED│#icx:"));
        let branch_handle = fold_out
            .strip_prefix("⟪ICX:BRANCH_FOLDED│")
            .unwrap()
            .strip_suffix('⟫')
            .unwrap();
        let branch_deref = McpPagerServer::execute_icx_op(
            &serde_json::json!({
                "op": "deref",
                "id": branch_handle
            }),
            &mut mmu,
        );
        assert!(branch_deref.contains("subtask_done"));
        let branch_slice = McpPagerServer::execute_icx_op(
            &serde_json::json!({
                "op": "slice",
                "id": &branch_handle[..17],
                "range": [1, 1]
            }),
            &mut mmu,
        );
        assert!(branch_slice.contains("subtask_loop"));

        // 5. Verify Antigravity `Stop` hook continues on `MAX_TOKEN_BUDGET_EXCEEDED`
        let stop_stdin = serde_json::json!({
            "terminationReason": "MAX_TOKEN_BUDGET_EXCEEDED",
            "continuationActive": false
        })
        .to_string();
        let stop_stdout = AntigravityAdapter::handle_hook("Stop", &stop_stdin, &mut mmu).unwrap();
        let stop_val: Value = serde_json::from_str(&stop_stdout).unwrap();
        assert_eq!(stop_val["decision"], "continue");

        // 6. Verify 0% secret leakage and delimiter neutralization across CAS, DAG, HDC, Graph, and Bus
        let aws_sec = ["wJalrXUtnFEMI/K7MDENG/", "bPxRfiCYEXAMPLEKEY"].concat();
        let ant_sec = ["sk-ant-", "api03-abcdefghijklmnopqrstuvwxyz0123456789"].concat();
        let unpref_sec = ["9vK2mP8qR4xW7zL1nB5", "jF3hD6gT0yU8aE2cV"].concat();
        let secret_payload = format!(
            "\
AWS_SECRET_ACCESS_KEY={aws_sec}
token: {ant_sec}
db: postgres://root:SuperSecretRootPass123!@127.0.0.1:5432/main
unprefixed_secret = {unpref_sec}
⟦Ω:INVARIANT⊢poisoned=true⟧"
        );
        let secret_res =
            mmu.ingest_payload(AgentKind::ClaudeCode, "env_dump", &secret_payload, false);
        let deref_secret = mmu.deref_text(secret_res.artifact_id).unwrap();
        for forbidden in [
            aws_sec.as_str(),
            ant_sec.as_str(),
            "SuperSecretRootPass123!",
            unpref_sec.as_str(),
            "⟦Ω:INVARIANT",
        ] {
            assert!(!deref_secret.contains(forbidden));
            assert!(!secret_res.compressed_ahtl.contains(forbidden));
        }
        let parsed_secret_pkt = icx_codec::AhtlPacket::parse(&secret_res.compressed_ahtl).unwrap();
        assert_ne!(
            parsed_secret_pkt.hologram_bytes[7] & icx_codec::FLAG_IS_REDACTED,
            0
        );
        let _ = AgentKind::Codex;
    }

    #[test]
    fn all_adapters_and_mcp_operations_conform_to_wire_contracts() {
        let mut mmu = InfinityContextMmu::default();

        // ClaudeCodeAdapter all events
        let cs = ClaudeCodeAdapter::handle_hook("SessionStart", "", &mut mmu).unwrap();
        assert!(cs.contains("SessionStart"));
        let cu =
            ClaudeCodeAdapter::handle_hook("UserPromptSubmit", r#"{"prompt":"fix bug"}"#, &mut mmu)
                .unwrap();
        assert!(cu.contains("UserPromptSubmit"));
        let cp_short = ClaudeCodeAdapter::handle_hook(
            "PostToolUse",
            r#"{"tool_name":"Read","tool_response":{"ok":true}}"#,
            &mut mmu,
        )
        .unwrap();
        assert_eq!(cp_short, "{}");
        let cp_none = ClaudeCodeAdapter::handle_hook("PostToolUse", "{}", &mut mmu).unwrap();
        assert_eq!(cp_none, "{}");
        let cpre = ClaudeCodeAdapter::handle_hook("PreCompact", "{}", &mut mmu).unwrap();
        assert!(cpre.contains("systemMessage"));
        let cpost = ClaudeCodeAdapter::handle_hook("PostCompact", "{}", &mut mmu).unwrap();
        assert!(cpost.contains("systemMessage"));
        assert_eq!(
            ClaudeCodeAdapter::handle_hook("UnknownEvent", "{}", &mut mmu).unwrap(),
            "{}"
        );

        // CodexAdapter all events
        let xs = CodexAdapter::handle_hook("session-start", "", &mut mmu).unwrap();
        assert!(xs.contains("SessionStart"));
        let xu = CodexAdapter::handle_hook(
            "user-prompt-submit",
            r#"{"prompt":"inspect dag"}"#,
            &mut mmu,
        )
        .unwrap();
        assert!(xu.contains("UserPromptSubmit"));
        let xp1 = CodexAdapter::handle_hook(
            "post-tool-use",
            r#"{"tool_name":"shell","tool_response":"line1\nline2"}"#,
            &mut mmu,
        )
        .unwrap();
        assert!(xp1.contains("PostToolUse"));
        let xp2 = CodexAdapter::handle_hook(
            "PostToolUse",
            r#"{"tool_name":"shell","tool_response":{"code":0}}"#,
            &mut mmu,
        )
        .unwrap();
        assert!(xp2.contains("PostToolUse"));
        let xp3 = CodexAdapter::handle_hook("PostToolUse", "{}", &mut mmu).unwrap();
        assert!(xp3.contains("PostToolUse"));
        let xc = CodexAdapter::handle_hook("pre-compact", "{}", &mut mmu).unwrap();
        assert!(xc.contains("systemMessage"));
        let xunk = CodexAdapter::handle_hook("other", "{}", &mut mmu).unwrap();
        assert!(xunk.contains(r#""continue":true"#));

        // AntigravityAdapter remaining events
        let asess = AntigravityAdapter::handle_hook("session-start", "", &mut mmu).unwrap();
        assert!(asess.contains("injectSteps"));
        let apre = AntigravityAdapter::handle_hook("PreToolUse", "{}", &mut mmu).unwrap();
        assert!(apre.contains("allow"));
        let apost_ok = AntigravityAdapter::handle_hook(
            "PostToolUse",
            r#"{"toolCall":{"name":"run","args":{"cmd":"ls"}}}"#,
            &mut mmu,
        )
        .unwrap();
        assert_eq!(apost_ok, "{}");
        let apost_err = AntigravityAdapter::handle_hook(
            "PostToolUse",
            r#"{"toolCall":{"name":"run","args":{"cmd":"ls"}},"error":"failed"}"#,
            &mut mmu,
        )
        .unwrap();
        assert_eq!(apost_err, "{}");
        let astop_done = AntigravityAdapter::handle_hook(
            "Stop",
            r#"{"terminationReason":"COMPLETED","continuationActive":true}"#,
            &mut mmu,
        )
        .unwrap();
        assert!(astop_done.contains(r#""decision":"stop""#));
        assert_eq!(
            AntigravityAdapter::handle_hook("Other", "{}", &mut mmu).unwrap(),
            "{}"
        );

        // HermesAdapter short & long windows
        let short_msgs = vec![HermesMessage {
            role: "user".into(),
            content: "short query".into(),
            tool_call_id: None,
            name: None,
        }];
        let short_sel = HermesAdapter::select_context(&short_msgs, 1, 1, &mut mmu);
        assert_eq!(short_sel.len(), 2);

        let mut long_msgs = Vec::new();
        for i in 0..6 {
            long_msgs.push(HermesMessage {
                role: if i == 0 {
                    "system".into()
                } else {
                    "tool".into()
                },
                content: format!("tool result payload {i}"),
                tool_call_id: Some(format!("call_{i}")),
                name: Some(format!("tool_{i}")),
            });
        }
        let long_sel = HermesAdapter::select_context(&long_msgs, 1, 1, &mut mmu);
        assert_eq!(long_sel.len(), 4);
        assert!(
            long_sel[1]
                .content
                .contains("⟪ICX:HERMES_EPOCH│folded_msgs=4⟫")
        );

        // McpPagerServer JSON-RPC methods and error paths
        let bad_json = McpPagerServer::handle_jsonrpc("{bad", &mut mmu);
        assert_eq!(bad_json["error"]["code"], -32700);
        let init_resp = McpPagerServer::handle_jsonrpc(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#,
            &mut mmu,
        );
        assert_eq!(
            init_resp["result"]["serverInfo"]["name"],
            "infinity-context-mmu"
        );
        let list_resp = McpPagerServer::handle_jsonrpc(
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
            &mut mmu,
        );
        assert_eq!(list_resp["result"]["tools"][0]["name"], "icx");
        let unknown_m = McpPagerServer::handle_jsonrpc(
            r#"{"jsonrpc":"2.0","id":3,"method":"unknown/method"}"#,
            &mut mmu,
        );
        assert_eq!(unknown_m["error"]["code"], -32601);

        assert!(
            McpPagerServer::execute_icx_op(&serde_json::json!({"op":"deref"}), &mut mmu)
                .starts_with("ERR:")
        );
        assert!(
            McpPagerServer::execute_icx_op(&serde_json::json!({"op":"deref","id":"bad"}), &mut mmu)
                .starts_with("ERR:")
        );
        assert!(
            McpPagerServer::execute_icx_op(&serde_json::json!({"op":"slice"}), &mut mmu)
                .starts_with("ERR:")
        );
        assert!(
            McpPagerServer::execute_icx_op(&serde_json::json!({"op":"slice","id":"bad"}), &mut mmu)
                .starts_with("ERR:")
        );
        assert!(
            McpPagerServer::execute_icx_op(
                &serde_json::json!({"op":"grep","id":"bad","q":"x"}),
                &mut mmu
            )
            .starts_with("ERR:")
        );
        let grep_all =
            McpPagerServer::execute_icx_op(&serde_json::json!({"op":"grep","q":"line1"}), &mut mmu);
        assert!(grep_all.contains("line1"));
        let recall_out = McpPagerServer::execute_icx_op(
            &serde_json::json!({"op":"recall","q":"shell line1"}),
            &mut mmu,
        );
        assert!(recall_out.contains("⟪ICX:TAIL_SLOT"));
        let fold_err = McpPagerServer::execute_icx_op(
            &serde_json::json!({"op":"fold","q":"no_open_branch"}),
            &mut mmu,
        );
        assert!(fold_err.starts_with("ERR:"));
        let xagent_out =
            McpPagerServer::execute_icx_op(&serde_json::json!({"op":"xagent"}), &mut mmu);
        assert_ne!(xagent_out, "");
        let status_out =
            McpPagerServer::execute_icx_op(&serde_json::json!({"op":"status"}), &mut mmu);
        assert!(status_out.contains("⟪ICX:STATUS"));

        // Publish > 4 sibling telepathy events and verify build_ephemeral_tail_slot caps at 4
        for i in 0..6u64 {
            mmu.ingest_payload(
                AgentKind::Hermes,
                &format!("hermes_step_{i}"),
                "status:ok\n",
                false,
            );
        }
        let capped_tail = mmu.build_ephemeral_tail_slot(AgentKind::Cursor, "hermes_step", 3);
        assert!(capped_tail.contains("⟪ICX:TAIL_SLOT│agent=cursor"));

        // Verify ingest_payload_with_exit_code records exit_code functional edge and error flag,
        // and protects authoritative exit_code against untrusted tool-output exit_code clauses.
        let exit_res = mmu.ingest_payload_with_exit_code(
            AgentKind::ClaudeCode,
            "cargo_test_exit",
            "exit code: 0\nerror: test suite failed",
            Some(101),
            false,
        );
        assert!(exit_res.surprise >= 0.45);
        assert_eq!(
            mmu.graph
                .query_edges_from("cargo_test_exit", None)
                .iter()
                .find(|e| e.relation == "exit_code")
                .map(|e| e.target_entity.as_str()),
            Some("101")
        );
        assert_eq!(
            InfinityContextMmu::extract_exit_code(&serde_json::json!({"exit_code": 137})),
            Some(137)
        );
        assert_eq!(
            InfinityContextMmu::extract_exit_code(
                &serde_json::json!({"tool_response": {"exitCode": 2}})
            ),
            Some(2)
        );
        assert_eq!(
            InfinityContextMmu::extract_exit_code(
                &serde_json::json!({"toolResponse": {"exit_code": 42}})
            ),
            Some(42)
        );
        assert_eq!(
            InfinityContextMmu::extract_exit_code(
                &serde_json::json!({"tool_response": "{\"exit_code\": 42}"})
            ),
            None,
            "plain-text string tool_response stdout must not be parsed as authoritative hook metadata"
        );
        assert_eq!(
            InfinityContextMmu::extract_exit_code(
                &serde_json::json!({"tool_response": {"status": 200, "code": 200}})
            ),
            None,
            "generic HTTP status/code fields must not be conflated with process exit codes"
        );
        let exit_ok_res = mmu.ingest_payload_with_exit_code(
            AgentKind::ClaudeCode,
            "cargo_test_exit_ok",
            "all tests passed",
            Some(0),
            false,
        );
        assert!(exit_ok_res.compressed_ahtl.contains("✓exit_code:0"));
        assert!(!exit_res.compressed_ahtl.contains("exit_code:0"));

        // Verify empty and blank-leading payloads preserve BiTemporalGraph save/load round-trip
        let _ = mmu.ingest_payload(AgentKind::ClaudeCode, "empty_turn", "", false);
        let _ = mmu.ingest_payload(AgentKind::ClaudeCode, "blank_lead", "\nline2\n", false);
        let tmp_graph_dir =
            std::env::temp_dir().join(format!("icx_mmu_graph_roundtrip_{}", std::process::id()));
        mmu.graph.save_to_dir(&tmp_graph_dir).unwrap();
        let reloaded = icx_store::BiTemporalGraph::load_from_dir(&tmp_graph_dir).unwrap();
        assert_eq!(reloaded.active_edge_count(), mmu.graph.active_edge_count());
        let _ = std::fs::remove_dir_all(&tmp_graph_dir);

        let _ = AntigravityAdapter::handle_hook(
            "PostToolUse",
            r#"{"toolCall":{"name":"run","args":{"cmd":"ls"}},"toolResponse":"file.rs","error":"warn"}"#,
            &mut mmu,
        )
        .unwrap();
        let _ = AntigravityAdapter::handle_hook(
            "PostToolUse",
            r#"{"toolResponse":{"exit_code":0}}"#,
            &mut mmu,
        )
        .unwrap();

        let null_out = AntigravityAdapter::handle_hook(
            "PostToolUse",
            r#"{"toolCall":{"args":null},"toolResponse":null,"result":"ok_fallback","error":{"code":1}}"#,
            &mut mmu,
        )
        .unwrap();
        assert!(null_out.contains("ok") || !null_out.contains("null"));

        let no_args_call = McpPagerServer::handle_jsonrpc(
            r#"{"jsonrpc":"2.0","id":99,"method":"tools/call","params":{}}"#,
            &mut mmu,
        );
        assert!(no_args_call.to_string().contains("ICX:STATUS"));
        let bad_slice = McpPagerServer::execute_icx_op(
            &serde_json::json!({"op": "slice", "id": exit_res.artifact_id.to_string(), "range": [0, 5]}),
            &mut mmu,
        );
        assert!(bad_slice.starts_with("ERR:"));

        // Verify MMU error paths for missing artifacts, malformed handles, and invalid line ranges
        assert!(mmu.resolve_handle("not_a_handle").is_err());
        assert!(mmu.deref_text(icx_core::ArtifactId(0xDEAD_BEEF)).is_err());
        assert!(
            mmu.deref_lines(icx_core::ArtifactId(0xDEAD_BEEF), 1, 2)
                .is_err()
        );
        assert!(mmu.deref_lines(exit_res.artifact_id, 0, 2).is_err());

        // Verify AgentHookAdapter trait surface, PreToolUse, SubagentStart/Stop, and MCP tool virtualization
        let claude_trait: &dyn AgentHookAdapter = &ClaudeCodeAdapter;
        assert_eq!(claude_trait.agent_kind(), AgentKind::ClaudeCode);
        let claude_pre = claude_trait
            .handle_hook_event(
                "PreToolUse",
                r#"{"updatedInput":{"command":"cargo test --quiet"}}"#,
                &mut mmu,
            )
            .unwrap();
        assert!(claude_pre.contains("cargo test --quiet"));
        let claude_sub_start = claude_trait
            .handle_hook_event("SubagentStart", r#"{"subagent_id":"sub-1"}"#, &mut mmu)
            .unwrap();
        assert!(claude_sub_start.contains("SubagentStart"));
        // Unidentified parent PostToolUse stays on the parent trunk while sub-1 is running
        let parent_nodes_before = mmu.dag.node_count();
        let claude_mcp_post = claude_trait
            .handle_hook_event(
                "PostToolUse",
                &serde_json::json!({
                    "tool_name": "mcp__github__search",
                    "tool_response": "result line\n".repeat(60)
                })
                .to_string(),
                &mut mmu,
            )
            .unwrap();
        assert!(claude_mcp_post.contains("updatedMCPToolOutput"));
        assert!(claude_mcp_post.contains("lines=1..60"));
        assert_eq!(mmu.dag.node_count(), parent_nodes_before + 1);
        // Tagged subagent PostToolUse is buffered in sub-1's branch
        let _ = claude_trait
            .handle_hook_event(
                "PostToolUse",
                &serde_json::json!({
                    "subagent_id": "sub-1",
                    "tool_name": "Read",
                    "tool_response": "subagent line\n".repeat(60)
                })
                .to_string(),
                &mut mmu,
            )
            .unwrap();
        assert_eq!(mmu.dag.node_count(), parent_nodes_before + 1);
        // Compaction while sub-1 is active preserves sub-1's open scope
        let _ = claude_trait
            .handle_hook_event("PreCompact", "{}", &mut mmu)
            .unwrap();
        let claude_sub_stop = claude_trait
            .handle_hook_event("SubagentStop", r#"{"summary":"done_sub_1"}"#, &mut mmu)
            .unwrap();
        assert!(claude_sub_stop.contains("ICX:SUBAGENT_FOLDED"));
        let _ = claude_trait
            .handle_hook_event(
                "SubagentStart",
                r#"{"subagent_id":"sub-zero-steps"}"#,
                &mut mmu,
            )
            .unwrap();
        let stop_zero = claude_trait
            .handle_hook_event(
                "SubagentStop",
                r#"{"subagent_id":"sub-zero-steps","summary":"no_tool_calls"}"#,
                &mut mmu,
            )
            .unwrap();
        assert!(stop_zero.contains("ICX:SUBAGENT_FOLDED"));
        let claude_sub_stop_empty = claude_trait
            .handle_hook_event(
                "SubagentStop",
                r#"{"outcome":"no_active_branch"}"#,
                &mut mmu,
            )
            .unwrap();
        assert!(claude_sub_stop_empty.contains("ICX:SUBAGENT_STOP"));

        let codex_trait: &dyn AgentHookAdapter = &CodexAdapter;
        assert_eq!(codex_trait.agent_kind(), AgentKind::Codex);
        let _ = claude_trait
            .handle_hook_event("PreToolUse", "{}", &mut mmu)
            .unwrap();
        let _ = codex_trait
            .handle_hook_event("pre-tool-use", "{}", &mut mmu)
            .unwrap();
        let codex_pre = codex_trait
            .handle_hook_event(
                "pre-tool-use",
                r#"{"updatedInput":{"cmd":"pytest"}}"#,
                &mut mmu,
            )
            .unwrap();
        assert!(codex_pre.contains("PreToolUse"));
        let codex_sub_start = codex_trait
            .handle_hook_event("subagent-start", r#"{"agent_id":"cx-sub"}"#, &mut mmu)
            .unwrap();
        assert!(codex_sub_start.contains("SubagentStart"));
        let codex_mcp_post = codex_trait
            .handle_hook_event(
                "post-tool-use",
                &serde_json::json!({
                    "agent_id": "cx-sub",
                    "tool_name": "mcp__fs__read",
                    "tool_response": "file line\n".repeat(60)
                })
                .to_string(),
                &mut mmu,
            )
            .unwrap();
        assert!(codex_mcp_post.contains("updatedMCPToolOutput"));
        let _ = codex_trait
            .handle_hook_event("pre-compact", "{}", &mut mmu)
            .unwrap();
        let codex_sub_stop = codex_trait
            .handle_hook_event("subagent-stop", r#"{"outcome":"cx_done"}"#, &mut mmu)
            .unwrap();
        assert!(codex_sub_stop.contains("ICX:SUBAGENT_FOLDED"));
        let codex_sub_stop_empty = codex_trait
            .handle_hook_event("subagent-stop", "{}", &mut mmu)
            .unwrap();
        assert!(codex_sub_stop_empty.contains("ICX:SUBAGENT_STOP"));

        // Verify overlapping subagents and ID-keyed subagent branch folding without parent trunk duplication
        let nodes_before_subs = mmu.dag.node_count();
        let _ = claude_trait
            .handle_hook_event("SubagentStart", r#"{"subagent_id":"sub-A"}"#, &mut mmu)
            .unwrap();
        let _ = claude_trait
            .handle_hook_event(
                "PostToolUse",
                &serde_json::json!({
                    "subagent_id": "sub-A",
                    "tool_name": "Bash",
                    "tool_response": "step A\n".repeat(60)
                })
                .to_string(),
                &mut mmu,
            )
            .unwrap();
        let _ = claude_trait
            .handle_hook_event("SubagentStart", r#"{"subagent_id":"sub-B"}"#, &mut mmu)
            .unwrap();
        let _ = claude_trait
            .handle_hook_event(
                "PostToolUse",
                &serde_json::json!({
                    "subagent_id": "sub-B",
                    "tool_name": "Bash",
                    "tool_response": "step B\n".repeat(60)
                })
                .to_string(),
                &mut mmu,
            )
            .unwrap();
        // While sub-A and sub-B are still running, neither step has been appended to the parent DAG trunk
        assert_eq!(mmu.dag.node_count(), nodes_before_subs);
        // Stopping sub-A out-of-order appends its 1 child turn + 1 folded L2 branch node (2 nodes total)
        let stop_a = claude_trait
            .handle_hook_event(
                "SubagentStop",
                r#"{"subagent_id":"sub-A","summary":"done A"}"#,
                &mut mmu,
            )
            .unwrap();
        assert!(stop_a.contains("ICX:SUBAGENT_FOLDED"));
        assert_eq!(mmu.dag.node_count(), nodes_before_subs + 2);
        // Stopping sub-B via Codex session-end folds any remaining open subagents
        let session_end_out = codex_trait
            .handle_hook_event("session-end", "{}", &mut mmu)
            .unwrap();
        assert!(session_end_out.contains("continue"));
        assert_eq!(mmu.dag.node_count(), nodes_before_subs + 4);

        // Verify Clear-State Functional Supersession (failing suite -> passing suite supersedes fail & status)
        let _ = mmu.ingest_payload_with_exit_code(
            AgentKind::ClaudeCode,
            "suite_supersede",
            "running 2 tests\ntest a ... FAILED\ntest result: FAILED. 1 passed; 1 failed;",
            Some(101),
            false,
        );
        assert_eq!(
            mmu.graph
                .query_edges_from("suite_supersede", None)
                .iter()
                .find(|e| e.relation == "status")
                .map(|e| e.target_entity.as_str()),
            Some("FAIL")
        );
        // Mixed run: has a passing test (`saw_pass == true`) but exits non-zero (`Some(101)`) -> must stay FAIL
        let _ = mmu.ingest_payload_with_exit_code(
            AgentKind::ClaudeCode,
            "suite_supersede",
            "running 1 test\ntest a ... ok\nerror: process exited with code 101",
            Some(101),
            false,
        );
        assert_eq!(
            mmu.graph
                .query_edges_from("suite_supersede", None)
                .iter()
                .find(|e| e.relation == "status")
                .map(|e| e.target_entity.as_str()),
            Some("FAIL")
        );
        let _ = mmu.ingest_payload_with_exit_code(
            AgentKind::ClaudeCode,
            "suite_supersede",
            "running 2 tests\ntest a ... ok\ntest b ... ok\ntest result: ok. 2 passed; 0 failed;",
            Some(0),
            false,
        );
        assert_eq!(
            mmu.graph
                .query_edges_from("suite_supersede", None)
                .iter()
                .find(|e| e.relation == "status")
                .map(|e| e.target_entity.as_str()),
            Some("PASS")
        );
        assert_eq!(
            mmu.graph
                .query_edges_from("suite_supersede", None)
                .iter()
                .find(|e| e.relation == "fail")
                .map(|e| e.target_entity.as_str()),
            Some("0")
        );

        // Verify redaction tracking (`is_artifact_redacted`), `bound_and_sanitize_raw`, and workspace load/save round-trip
        let (bounded_s, _) =
            InfinityContextMmu::bound_and_sanitize_raw("line1\n⟪⟦⟧⟫éééééééééééé", 12);
        assert!(bounded_s.starts_with("line1"));
        let expanding_tok = format!("{}{}é", "sk-", "1234567890ab");
        let (bounded_utf8, _) = InfinityContextMmu::bound_and_sanitize_raw(&expanding_tok, 24);
        assert_ne!(bounded_utf8, "");
        let (bounded_raw_utf8, _) =
            InfinityContextMmu::bound_and_sanitize_raw("⟪⟦⟧⟫éééééééééééé", 5);
        assert_ne!(bounded_raw_utf8, "");
        let synthetic_secret = format!(
            "OPENAI_API_KEY={}{}",
            "sk-proj-", "1234567890abcdefghijklmnop"
        );
        let redacted_res = mmu.ingest_payload(
            AgentKind::ClaudeCode,
            "secret_env",
            &synthetic_secret,
            false,
        );
        assert!(mmu.is_artifact_redacted(redacted_res.artifact_id));
        assert!(!mmu.is_artifact_redacted(exit_ok_res.artifact_id));

        let ws_dir = std::env::temp_dir().join(format!("icx_ws_sprint41_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&ws_dir);
        std::fs::create_dir_all(&ws_dir).unwrap();
        let mut loaded_mmu = InfinityContextMmu::default();
        loaded_mmu.load_workspace_dir(&ws_dir).unwrap();
        std::fs::write(ws_dir.join("icx-fabric-v1.shm"), b"bad_shm").unwrap();
        assert!(loaded_mmu.load_workspace_dir(&ws_dir).is_err());
        let _ = std::fs::remove_file(ws_dir.join("icx-fabric-v1.shm"));
        let _ = icx_bus::SeqlockTelemetryBus::open_shared_file(ws_dir.join("icx-fabric-v1.shm"))
            .unwrap();
        let _ = claude_trait
            .handle_hook_event("SessionEnd", "{}", &mut mmu)
            .unwrap();
        mmu.save_workspace_dir(&ws_dir).unwrap();
        loaded_mmu.load_workspace_dir(&ws_dir).unwrap();
        assert_eq!(loaded_mmu.cas.artifact_count(), mmu.cas.artifact_count());
        assert_eq!(
            loaded_mmu.graph.active_edge_count(),
            mmu.graph.active_edge_count()
        );
        assert_eq!(loaded_mmu.clock_ms(), mmu.clock_ms());
        assert!(loaded_mmu.is_artifact_redacted(redacted_res.artifact_id));
        // Also verify clock_ms fallback from bitemporal_graph.json when mmu_state.json is absent
        let _ = std::fs::remove_file(ws_dir.join("mmu_state.json"));
        let mut graph_clock_mmu = InfinityContextMmu::default();
        graph_clock_mmu.load_workspace_dir(&ws_dir).unwrap();
        assert_eq!(graph_clock_mmu.clock_ms(), mmu.clock_ms());

        // Verify CodexRolloutTailer incremental tailing, empty-array fallthrough, partial-line safety, and rotation reset
        let rollout_path = ws_dir.join("rollout.jsonl");
        std::fs::write(
            &rollout_path,
            "{\"type\":\"function_call_output\",\"name\":\"exec\",\"output\":\"hello rollout\",\"exit_code\":0}\n\n{bad_json}\n{\"item\":{\"type\":\"message\",\"role\":\"assistant\",\"content\":\"   \"}}\n{\"type\":\"Compacted\",\"summary\":[],\"text\":\"compacted_fallback_text\"}\n{\"type\":\"TurnContext\",\"payload\":{\"summary\":{\"turn\":1}}}\npartial_trailing_without_newline",
        )
        .unwrap();
        let mut tailer = CodexRolloutTailer::new();
        assert_eq!(tailer.offset_for(&rollout_path), 0);
        let c1 = tailer.poll_file(&rollout_path, &mut loaded_mmu).unwrap();
        assert_eq!(c1, 3);
        let saved_offset = tailer.offset_for(&rollout_path);
        assert!(saved_offset > 0);
        // Append another complete line (>64 KiB total) + an incomplete trailing multibyte UTF-8 sequence (0xC3)
        {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(&rollout_path)
                .unwrap();
            writeln!(
                f,
                "\n{{\"type\":\"RetainedContext\",\"text\":\"{}\"}}",
                "big_payload_".repeat(6000)
            )
            .unwrap();
            f.write_all(&[b'{', b'"', b'x', b'"', b':', b'"', 0xC3])
                .unwrap();
        }
        let c_inc = tailer.poll_file(&rollout_path, &mut loaded_mmu).unwrap();
        assert_eq!(c_inc, 1);
        let saved_offset = tailer.offset_for(&rollout_path);
        assert!(saved_offset >= 65_536);
        // Poll again after completing the trailing UTF-8 sequence to exercise >64 KiB verified_64k_digest reuse
        {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(&rollout_path)
                .unwrap();
            f.write_all(&[
                0xA9, b'"', b',', b'"', b't', b'e', b'x', b't', b'"', b':', b'7', b'}', b'\n',
            ])
            .unwrap();
        }
        let c_utf8_done = tailer.poll_file(&rollout_path, &mut loaded_mmu).unwrap();
        assert_eq!(c_utf8_done, 1);
        let saved_offset = tailer.offset_for(&rollout_path);
        // Direct call with mid-line byte_offset (preceding byte != '\n') resets to 0
        let (_, c_mid) =
            CodexRolloutTailer::ingest_rollout_jsonl_since(&rollout_path, 5, &mut loaded_mmu)
                .unwrap();
        assert_eq!(c_mid, 5);
        // Rewrite file with different content of GREATER length to verify prefix_digest mismatch resets offset to 0
        let rewritten = format!(
            "{{\"type\":\"RetainedContext\",\"text\":\"{}\"}}\n",
            "retained_longer_rewrite_".repeat(3500)
        );
        assert!(rewritten.len() as u64 > saved_offset);
        std::fs::write(&rollout_path, &rewritten).unwrap();
        let c_rewrite = tailer.poll_file(&rollout_path, &mut loaded_mmu).unwrap();
        assert_eq!(c_rewrite, 1);
        // Truncate file to shorter length to verify offset reset to 0
        std::fs::write(
            &rollout_path,
            "{\"type\":\"RetainedContext\",\"text\":\"retained_after_rotate\"}\n",
        )
        .unwrap();
        let c2 = tailer.poll_file(&rollout_path, &mut loaded_mmu).unwrap();
        assert_eq!(c2, 1);
        let _ = std::fs::remove_dir_all(&ws_dir);
    }
}
