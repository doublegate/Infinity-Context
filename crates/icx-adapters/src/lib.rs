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
pub use mmu::{InfinityContextMmu, IngestResult, DEFAULT_TOOL_VIRTUALIZE_THRESHOLD_BYTES};

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
        let _ = AgentKind::Codex;
    }
}
