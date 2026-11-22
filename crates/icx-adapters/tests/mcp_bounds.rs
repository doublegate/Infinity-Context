//! v1.1.0 MCP resource bounds (ICX-AUD-014).

use icx_adapters::mcp::{MAX_FOLD_DEPTH, McpPagerServer};
use icx_adapters::mmu::InfinityContextMmu;
use serde_json::json;

#[test]
fn nested_fold_branches_are_bounded() {
    let mut mmu = InfinityContextMmu::default();
    for i in 0..MAX_FOLD_DEPTH {
        let out = McpPagerServer::execute_icx_op(
            &json!({"op": "fold", "q": format!("begin:b{i}")}),
            &mut mmu,
        );
        assert!(out.contains("BRANCH_OPENED"), "level {i}: {out}");
    }
    let refused =
        McpPagerServer::execute_icx_op(&json!({"op": "fold", "q": "begin:one-too-many"}), &mut mmu);
    assert!(refused.contains("ERR"), "{refused}");
    assert_eq!(mmu.dag.active_branch_depth(), MAX_FOLD_DEPTH);
    // Folding still works and frees a level.
    let folded = McpPagerServer::execute_icx_op(&json!({"op": "fold", "q": "done"}), &mut mmu);
    assert!(folded.contains("BRANCH_FOLDED"), "{folded}");
}

// --- v1.1.0 MCP dual-era protocol support (ICX-AUD-008) ---------------------------------------

fn rpc(req: &serde_json::Value) -> serde_json::Value {
    let mut mmu = InfinityContextMmu::default();
    McpPagerServer::handle_jsonrpc(&req.to_string(), &mut mmu)
}

#[test]
fn legacy_initialize_negotiates_the_requested_supported_version() {
    for v in ["2024-11-05", "2025-06-18", "2025-11-25"] {
        let r = rpc(
            &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion": v}}),
        );
        assert_eq!(r["result"]["protocolVersion"], v);
    }
    let r = rpc(
        &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"1999-01-01"}}),
    );
    assert_eq!(
        r["result"]["protocolVersion"], "2025-11-25",
        "falls back to latest legacy"
    );
}

#[test]
fn server_discover_advertises_versions_capabilities_and_identity() {
    let r = rpc(
        &json!({"jsonrpc":"2.0","id":"d1","method":"server/discover","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28"}}}),
    );
    let res = &r["result"];
    assert_eq!(res["resultType"], "complete");
    assert!(
        res["supportedVersions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "2026-07-28")
    );
    assert!(res["capabilities"]["tools"].is_object());
    assert_eq!(
        res["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
        "infinity-context-mmu"
    );
}

#[test]
fn modern_requests_carry_result_type_and_reject_unknown_versions() {
    let ok = rpc(
        &json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28"}}}),
    );
    assert_eq!(ok["result"]["resultType"], "complete");
    assert_eq!(ok["result"]["tools"][0]["name"], "icx");
    assert!(ok["result"]["ttlMs"].is_number());
    let bad = rpc(
        &json!({"jsonrpc":"2.0","id":3,"method":"tools/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"1900-01-01"}}}),
    );
    assert_eq!(bad["error"]["code"], -32022);
    assert_eq!(bad["error"]["data"]["requested"], "1900-01-01");
    assert!(bad["error"]["data"]["supported"].as_array().unwrap().len() >= 2);
}

#[test]
fn unknown_tool_names_are_reported_as_errors() {
    let r = rpc(
        &json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"rm_rf","arguments":{}}}),
    );
    assert_eq!(r["error"]["code"], -32602);
    assert!(
        r["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Unknown tool")
    );
}

#[test]
fn grep_results_carry_the_untrusted_data_provenance_header() {
    // PR #20 review (Antigravity): SECURITY.md promises provenance framing for grep too.
    let mut mmu = InfinityContextMmu::default();
    mmu.ingest_payload(
        icx_core::AgentKind::ClaudeCode,
        "Bash:cat log",
        "needle-7781 in a haystack\n",
        false,
    );
    let req = json!({"jsonrpc":"2.0","id":9,"method":"tools/call",
        "params":{"name":"icx","arguments":{"op":"grep","q":"needle-7781"}}});
    let r = McpPagerServer::handle_jsonrpc(&req.to_string(), &mut mmu);
    let text = r["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("needle-7781"), "{text}");
    assert!(text.starts_with("⟪ICX:PROVENANCE│op=grep"), "{text}");
    assert!(text.contains("trust=untrusted_data"), "{text}");
}

/// v1.3.2: a slice end past the last line is clamped instead of failing, and a grep reply is
/// bounded per line and in total (a minified line used to produce a reply of hundreds of KB).
#[test]
fn slice_ends_are_clamped_and_grep_replies_are_bounded() {
    use icx_adapters::mmu::InfinityContextMmu;
    use icx_adapters::{ClaudeCodeAdapter, McpPagerServer};
    use serde_json::json;
    let mut mmu = InfinityContextMmu::default();
    let call = |mmu: &mut InfinityContextMmu, args: serde_json::Value| {
        let req = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {"name": "icx", "arguments": args}});
        let resp = McpPagerServer::handle_jsonrpc(&req.to_string(), mmu);
        let text = resp["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_owned();
        (text, resp["result"]["isError"].as_bool().unwrap_or(false))
    };
    let stub = |mmu: &mut InfinityContextMmu, stdout: String| {
        let payload = json!({"hook_event_name": "PostToolUse", "tool_name": "Bash",
            "tool_input": {"command": "x"},
            "tool_response": {"stdout": stdout, "stderr": "", "interrupted": false, "isImage": false}});
        let out = ClaudeCodeAdapter::handle_hook("PostToolUse", &payload.to_string(), mmu).unwrap();
        format!("#icx:{}", &out.split("#icx:").nth(1).unwrap()[..16])
    };
    let short = stub(
        &mut mmu,
        (1..=200)
            .map(|n| format!("line {n}"))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
    );
    let (text, err) = call(
        &mut mmu,
        json!({"op": "slice", "id": short, "range": [195, 400]}),
    );
    assert!(
        !err && text.contains("line 200") && text.contains("line 195"),
        "{text}"
    );
    let (text, err) = call(
        &mut mmu,
        json!({"op": "slice", "id": short, "range": [500, 600]}),
    );
    assert!(err, "a start past the end is still an error: {text}");
    // A short folded branch (a DAG node, not a CAS artifact) clamps too, so the default range
    // `[1, 50]` works on it (review of #38).
    call(&mut mmu, json!({"op": "fold", "q": "begin:clamp"}));
    let (fold, _) = call(&mut mmu, json!({"op": "fold", "q": "short branch"}));
    let node = fold
        .split("#icx:")
        .nth(1)
        .map(|r| format!("#icx:{}", &r[..16]))
        .unwrap_or_else(|| panic!("no fold handle: {fold}"));
    let (text, err) = call(&mut mmu, json!({"op": "slice", "id": node}));
    assert!(!err, "default slice of a short fold node failed: {text}");

    let minified = format!("{{\"needle\":1,\"pad\":\"{}\"}}\n", "z".repeat(50_000)).repeat(100);
    stub(&mut mmu, minified);
    let (text, err) = call(&mut mmu, json!({"op": "grep", "q": "needle", "k": 250}));
    assert!(
        !err && text.len() <= 33 * 1024,
        "reply of {} bytes",
        text.len()
    );
    assert!(
        text.contains("…[+") && text.contains("more matches omitted"),
        "{}",
        &text[..300]
    );
}
