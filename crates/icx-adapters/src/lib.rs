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
pub use codex::CodexAdapter;
pub use hermes::{HermesAdapter, HermesMessage};
pub use mcp::McpPagerServer;
pub use mmu::{DEFAULT_TOOL_VIRTUALIZE_THRESHOLD_BYTES, InfinityContextMmu, IngestResult};

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

        // Verify empty and blank-leading payloads preserve BiTemporalGraph save/load round-trip
        let _ = mmu.ingest_payload(AgentKind::ClaudeCode, "empty_turn", "", false);
        let _ = mmu.ingest_payload(AgentKind::ClaudeCode, "blank_lead", "\nline2\n", false);
        let tmp_graph_dir =
            std::env::temp_dir().join(format!("icx_mmu_graph_roundtrip_{}", std::process::id()));
        mmu.graph.save_to_dir(&tmp_graph_dir).unwrap();
        let reloaded = icx_store::BiTemporalGraph::load_from_dir(&tmp_graph_dir).unwrap();
        assert_eq!(reloaded.active_edge_count(), mmu.graph.active_edge_count());
        let _ = std::fs::remove_dir_all(&tmp_graph_dir);

        // Verify MMU error paths for missing artifacts and malformed handles
        assert!(mmu.resolve_handle("not_a_handle").is_err());
        assert!(mmu.deref_text(icx_core::ArtifactId(0xDEAD_BEEF)).is_err());
        assert!(
            mmu.deref_lines(icx_core::ArtifactId(0xDEAD_BEEF), 1, 2)
                .is_err()
        );
    }
}
