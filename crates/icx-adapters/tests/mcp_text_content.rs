//! v1.3.2 (#37): an MCP tool result made of text content blocks (Context7, docs servers) is
//! stored as its text with real lines, so `slice` and `grep` can address parts of it; a result
//! with non-text blocks keeps its JSON, so nothing is lost.
use icx_adapters::mmu::InfinityContextMmu;
use icx_adapters::{ClaudeCodeAdapter, McpPagerServer};
use serde_json::{Value, json};

fn stub_handle(mmu: &mut InfinityContextMmu, response: &Value) -> String {
    let payload = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "mcp__claude_ai_Context7__query-docs",
        "tool_input": {"libraryId": "/serde-rs/json", "query": "from_str"},
        "tool_response": response
    });
    let out = ClaudeCodeAdapter::handle_hook("PostToolUse", &payload.to_string(), mmu).unwrap();
    let rest = out.split("#icx:").nth(1).expect("stubbed");
    format!("#icx:{}", &rest[..16])
}

fn call(mmu: &mut InfinityContextMmu, args: &Value) -> String {
    let req = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": "icx", "arguments": args}});
    let resp = McpPagerServer::handle_jsonrpc(&req.to_string(), mmu);
    resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn docs(sections: usize) -> String {
    (0..sections)
        .map(|n| format!("### Section {n}\n\nSource: https://example.invalid/{n}\n\n```rust\nlet x{n} = {n};\n```\n"))
        .collect::<Vec<_>>()
        .join("\n--------------------------------\n\n")
}

#[test]
fn text_blocks_are_stored_as_lines() {
    let mut mmu = InfinityContextMmu::default();
    let body = docs(40);
    let handle = stub_handle(&mut mmu, &json!([{"type": "text", "text": body}]));
    let full = call(&mut mmu, &json!({"op": "deref", "id": handle}));
    assert!(
        full.contains("### Section 7\n") && !full.contains(r#""type":"text""#),
        "{full}"
    );
    // A slice now addresses individual lines of the docs.
    let slice = call(
        &mut mmu,
        &json!({"op": "slice", "id": handle, "range": [1, 3]}),
    );
    assert!(
        slice.contains("### Section 0") && !slice.contains("Section 5"),
        "{slice}"
    );
}

#[test]
fn call_tool_result_objects_and_several_blocks_are_joined() {
    let mut mmu = InfinityContextMmu::default();
    let response = json!({"content": [
        {"type": "text", "text": docs(20)},
        {"type": "text", "text": docs(20).replace("Section", "Part")}
    ], "isError": false});
    let handle = stub_handle(&mut mmu, &response);
    let full = call(&mut mmu, &json!({"op": "deref", "id": handle}));
    assert!(
        full.contains("### Section 3\n") && full.contains("### Part 3\n"),
        "{full}"
    );
}

#[test]
fn non_text_blocks_keep_the_json_form() {
    let mut mmu = InfinityContextMmu::default();
    let response = json!([
        {"type": "text", "text": docs(30)},
        {"type": "image", "data": "aGVsbG8=", "mimeType": "image/png"}
    ]);
    let handle = stub_handle(&mut mmu, &response);
    let full = call(&mut mmu, &json!({"op": "deref", "id": handle}));
    assert!(full.contains(r#""type":"image""#), "{full}");
}
