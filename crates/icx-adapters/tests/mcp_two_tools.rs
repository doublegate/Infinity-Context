//! ADR-0009 (T-64-008): the read-only `icx` tool carries `readOnlyHint`; `fold` and `xagent`
//! broadcasts move to `icx_write`, and `icx` still runs them for one release with a note.

use icx_adapters::mcp::{McpPagerServer, READ_TOOL, WRITE_TOOL};
use icx_adapters::mmu::InfinityContextMmu;
use serde_json::{Value, json};

fn call(tool: &str, args: &Value, mmu: &mut InfinityContextMmu) -> (String, bool) {
    let req = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": tool, "arguments": args}});
    let r = McpPagerServer::handle_jsonrpc(&req.to_string(), mmu);
    let result = &r["result"];
    (
        result["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_owned(),
        result["isError"].as_bool().unwrap_or(false),
    )
}

#[test]
fn tools_list_has_a_read_only_tool_and_a_write_tool() {
    let mut mmu = InfinityContextMmu::default();
    let r = McpPagerServer::handle_jsonrpc(
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
        &mut mmu,
    );
    let tools = r["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 2);
    let read = &tools[0];
    let write = &tools[1];
    assert_eq!(read["name"], READ_TOOL);
    assert_eq!(read["annotations"]["readOnlyHint"], true);
    let read_ops: Vec<&str> = read["inputSchema"]["properties"]["op"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(
        !read_ops.contains(&"fold"),
        "the read-only tool does not offer fold"
    );
    assert_eq!(write["name"], WRITE_TOOL);
    assert!(
        write.get("annotations").is_none(),
        "no read-only hint on the write tool"
    );
    assert!(
        McpPagerServer::per_tool_token_estimates()
            .iter()
            .all(|&t| t < 180)
    );
}

#[test]
fn writes_go_through_icx_write_and_reads_through_icx() {
    let mut mmu = InfinityContextMmu::default();
    let (out, err) = call(
        WRITE_TOOL,
        &json!({"op": "fold", "q": "begin:task"}),
        &mut mmu,
    );
    assert!(!err && out.contains("BRANCH_OPENED"), "{out}");
    assert!(!out.contains("DEPRECATED"));
    let (out, err) = call(WRITE_TOOL, &json!({"op": "fold", "q": "done"}), &mut mmu);
    assert!(!err, "{out}");
    let (out, err) = call(READ_TOOL, &json!({"op": "status"}), &mut mmu);
    assert!(!err && out.contains("ICX:STATUS"), "{out}");
    let (out, err) = call(READ_TOOL, &json!({"op": "xagent"}), &mut mmu);
    assert!(
        !err && !out.contains("DEPRECATED"),
        "a poll is a read: {out}"
    );
}

#[test]
fn a_read_sent_to_icx_write_is_refused() {
    let mut mmu = InfinityContextMmu::default();
    for args in [
        json!({"op": "status"}),
        json!({"op": "xagent"}),
        json!({"op": "xagent", "q": "  "}),
    ] {
        let (out, err) = call(WRITE_TOOL, &args, &mut mmu);
        assert!(err && out.contains(READ_TOOL), "{args}: {out}");
    }
}

#[test]
fn a_write_sent_to_icx_still_runs_with_a_deprecation_note() {
    let mut mmu = InfinityContextMmu::default();
    let (out, err) = call(
        READ_TOOL,
        &json!({"op": "fold", "q": "begin:legacy"}),
        &mut mmu,
    );
    assert!(!err && out.contains("BRANCH_OPENED"), "{out}");
    assert!(
        out.contains("ICX:DEPRECATED") && out.contains(WRITE_TOOL),
        "{out}"
    );
    assert_eq!(mmu.dag.active_branch_depth(), 1, "the fold really happened");
    let (out, _) = call(READ_TOOL, &json!({"op": "xagent", "q": "hello"}), &mut mmu);
    assert!(out.contains("ICX:DEPRECATED"), "{out}");
}

#[test]
fn invalid_arguments_report_their_own_error_on_either_tool() {
    let mut mmu = InfinityContextMmu::default();
    for tool in [READ_TOOL, WRITE_TOOL] {
        let (out, err) = call(tool, &json!({"op": "nonsense"}), &mut mmu);
        assert!(err && out.contains("unknown `op`"), "{tool}: {out}");
    }
}

#[test]
fn both_tools_are_mutation_aware_and_never_ingested() {
    let fold = |tool: &str| {
        json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {"name": tool, "arguments": {"op": "fold", "q": "begin:x"}}})
        .to_string()
    };
    assert!(McpPagerServer::is_mutating_request(&fold(WRITE_TOOL)));
    assert!(McpPagerServer::is_mutating_request(&fold(READ_TOOL)));
    for name in [
        "icx_write",
        "mcp__icx__icx_write",
        "mcp_icx_icx_write",
        "mcp_infinity_context_icx_write",
        "plugin__icx_write",
        "mcp__icx__icx",
    ] {
        assert!(InfinityContextMmu::is_icx_pager_tool(name), "{name}");
    }
    for name in [
        "icx_writer",
        "mcp_icxtools_icx_write",
        "mcp_other_icx_write",
        "write",
    ] {
        assert!(!InfinityContextMmu::is_icx_pager_tool(name), "{name}");
    }
}
