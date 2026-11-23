//! T-64-001: Claude Code built-in tool results in the shapes recorded live on 2.1.330 (Code Mode
//! off). icx stores the tool's text, so a stub's line range is the real one, and the
//! replacement keeps every key of the tool's own result. Values here are synthetic.

use icx_adapters::{ClaudeCodeAdapter, InfinityContextMmu};
use icx_core::ArtifactId;
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn lines(n: usize, word: &str) -> String {
    (1..=n)
        .map(|i| format!("{word} {i} padded so the output is large"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn post_tool_use(tool: &str, response: &Value) -> Value {
    json!({
        "session_id": "s1",
        "transcript_path": "/tmp/t.jsonl",
        "cwd": "/home/u/project",
        "prompt_id": "p1",
        "permission_mode": "default",
        "effort": {"level": "medium"},
        "hook_event_name": "PostToolUse",
        "tool_name": tool,
        "tool_input": {},
        "tool_response": response,
        "tool_use_id": "toolu_1",
        "duration_ms": 12
    })
}

fn keys(v: &Value) -> BTreeSet<String> {
    v.as_object().unwrap().keys().cloned().collect()
}

/// Runs PostToolUse and returns the replacement, checking the stored text and its line count.
fn check(tool: &str, response: &Value, stored: &str) -> Value {
    let mut mmu = InfinityContextMmu::default();
    let out = ClaudeCodeAdapter::handle_hook(
        "PostToolUse",
        &post_tool_use(tool, response).to_string(),
        &mut mmu,
    )
    .unwrap();
    let id = ArtifactId::hash_bytes(stored.as_bytes()).0;
    let meta = mmu
        .cas
        .get_meta(id)
        .unwrap_or_else(|| panic!("{tool}: text not stored"));
    assert_eq!(meta.total_lines, stored.lines().count(), "{tool}");
    let v: Value = serde_json::from_str(&out).unwrap();
    let stub = &v["hookSpecificOutput"]["updatedToolOutput"];
    assert_eq!(keys(stub), keys(response), "{tool} keeps its shape");
    assert!(
        out.contains(&format!("lines=1..{}", stored.lines().count())),
        "{tool}: {out}"
    );
    stub.clone()
}

#[test]
fn read_stores_the_file_text() {
    let content = lines(3000, "row");
    let response = json!({"type": "text", "file": {"filePath": "/p/big.txt", "content": content,
        "numLines": 3000, "startLine": 1, "totalLines": 3000}});
    let stub = check("Read", &response, &content);
    assert!(
        stub["file"]["content"]
            .as_str()
            .unwrap()
            .starts_with("⟪ICX:STUB")
    );
    assert_eq!(keys(&stub["file"]), keys(&response["file"]));
}

#[test]
fn grep_stores_its_content_or_its_file_list() {
    let content = lines(400, "match_token entry");
    let response = json!({"mode": "content", "numFiles": 0, "filenames": [], "content": content,
        "numLines": 400, "totalLines": 400, "appliedLimit": 250});
    check("Grep", &response, &content);

    let names: Vec<String> = (1..=600)
        .map(|i| format!("src/module_{i}/file.rs"))
        .collect();
    let response = json!({"mode": "files_with_matches", "numFiles": 600, "filenames": names});
    check("Grep", &response, &names.join("\n"));
}

#[test]
fn glob_stores_one_file_name_per_line() {
    let names: Vec<String> = (1..=600)
        .map(|i| format!("many/file_with_a_fairly_long_name_{i}.txt"))
        .collect();
    let response = json!({"filenames": names, "durationMs": 5, "numFiles": 600,
        "truncated": false, "totalMatches": 600, "countIsComplete": true});
    let stub = check("Glob", &response, &names.join("\n"));
    let list = stub["filenames"]
        .as_array()
        .expect("filenames stays a list");
    assert_eq!(list.len(), 1, "one stub entry stands for every name");
    assert!(list[0].as_str().unwrap().starts_with("⟪ICX:STUB"));
}

#[test]
fn unknown_and_image_shapes_are_kept_as_json() {
    let mut mmu = InfinityContextMmu::default();
    let image = json!({"type": "image", "file": {"base64": "iVBORw0KGgo".repeat(800),
        "type": "image/png"}});
    let out = ClaudeCodeAdapter::handle_hook(
        "PostToolUse",
        &post_tool_use("Read", &image).to_string(),
        &mut mmu,
    )
    .unwrap();
    let id = ArtifactId::hash_bytes(image.to_string().as_bytes()).0;
    assert!(
        mmu.cas.get_meta(id).is_some(),
        "an image Read is stored whole"
    );
    // The image reaches the model unchanged: no replacement of its base64 data (review of #88).
    assert!(!out.contains("updatedToolOutput"), "{out}");
}

/// The reply for a large result that holds only binary data: no replacement of any kind.
fn assert_not_replaced(tool: &str, response: &Value) {
    let mut mmu = InfinityContextMmu::default();
    let out = ClaudeCodeAdapter::handle_hook(
        "PostToolUse",
        &post_tool_use(tool, response).to_string(),
        &mut mmu,
    )
    .unwrap();
    assert!(!out.contains("updatedToolOutput"), "{tool}: {out}");
    assert!(!out.contains("updatedMCPToolOutput"), "{tool}: {out}");
    assert!(mmu.cas.artifact_count() > 0, "{tool}: still stored");
}

#[test]
fn binary_data_is_never_replaced() {
    let data = "iVBORw0KGgo".repeat(5000);
    // An Agent report with only an image block.
    assert_not_replaced(
        "Agent",
        &json!({"agentId": "a1", "status": "completed", "content": [{"type": "image",
            "source": {"type": "base64", "media_type": "image/png", "data": data}}]}),
    );
    // An MCP tool returning an image beside a short caption.
    assert_not_replaced(
        "mcp__shots__take",
        &json!({"content": [{"type": "text", "text": "screenshot"},
            {"type": "image", "data": data, "mimeType": "image/png"}]}),
    );
    // A large text value beside an image is replaced; the image is kept.
    let response = json!({"content": [{"type": "text", "text": lines(800, "caption")},
        {"type": "image", "data": data.clone(), "mimeType": "image/png"}], "isError": false});
    let mut mmu = InfinityContextMmu::default();
    let out = ClaudeCodeAdapter::handle_hook(
        "PostToolUse",
        &post_tool_use("mcp__other__get", &response).to_string(),
        &mut mmu,
    )
    .unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    let replaced = &v["hookSpecificOutput"]["updatedToolOutput"];
    assert!(
        replaced["content"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with("⟪ICX:STUB")
    );
    assert_eq!(
        replaced["content"][1]["data"], data,
        "the image data is kept"
    );
    assert!(
        v["hookSpecificOutput"]
            .get("updatedMCPToolOutput")
            .is_none(),
        "a string would replace the image too"
    );
}

#[test]
fn a_code_mode_script_result_is_stored_and_replaced_in_its_result_field() {
    // Shape recorded live on 2.1.330 with Code Mode on: the script's value is in `result`, its
    // console output in `stdout`, and `code` echoes the script.
    let result = lines(2000, "computed");
    let code = format!(
        "const r = await $.tool.call('Bash', {{command: 'x'}}); {}",
        "a".repeat(30_000)
    );
    let response = json!({"code": code, "result": result, "stdout": "", "stderr": "",
        "toolsUsed": ["Bash"]});
    let stub = check("REPL", &response, &result);
    assert!(stub["result"].as_str().unwrap().starts_with("⟪ICX:STUB"));
    assert_eq!(
        stub["code"], response["code"],
        "the script is never replaced"
    );
    assert_eq!(stub["toolsUsed"], json!(["Bash"]));
}

#[test]
fn a_code_mode_script_keeps_its_streams_in_the_stored_text() {
    let mut mmu = InfinityContextMmu::default();
    let response = json!({"code": "x", "result": "done", "stdout": lines(500, "log"),
        "stderr": "warn", "toolsUsed": []});
    ClaudeCodeAdapter::handle_hook(
        "PostToolUse",
        &post_tool_use("REPL", &response).to_string(),
        &mut mmu,
    )
    .unwrap();
    let expected = format!("done\n[stdout]\n{}\n[stderr]\nwarn", lines(500, "log"));
    assert!(
        mmu.cas
            .get_meta(ArtifactId::hash_bytes(expected.as_bytes()).0)
            .is_some()
    );
}

#[test]
fn a_script_that_called_no_tool_has_no_tools_used_and_is_still_a_script() {
    // Recorded live: without inner tool calls the result has no `toolsUsed` key.
    let result = lines(3000, "value");
    let response = json!({"code": "return x", "result": result, "stdout": "", "stderr": ""});
    let stub = check("REPL", &response, &result);
    assert!(stub["result"].as_str().unwrap().starts_with("⟪ICX:STUB"));
}

#[test]
fn web_fetch_stores_its_result_text() {
    let result = lines(900, "fetched");
    let response = json!({"bytes": 41_738, "code": 200, "codeText": "OK", "result": result,
        "durationMs": 900, "url": "https://example.org/rfc.txt"});
    let stub = check("WebFetch", &response, &result);
    assert!(stub["result"].as_str().unwrap().starts_with("⟪ICX:STUB"));
    assert_eq!(stub["url"], response["url"]);
}

#[test]
fn an_agent_report_becomes_one_stub_block() {
    let parts = [
        lines(800, "first"),
        lines(700, "second"),
        lines(600, "third"),
    ];
    let response = json!({"status": "completed", "prompt": "do it", "agentId": "a1b2c3",
        "agentType": "general-purpose", "content": [
            {"type": "text", "text": parts[0]},
            {"type": "text", "text": parts[1]},
            {"type": "text", "text": parts[2]}],
        "totalTokens": 10, "usage": {}, "canContinueAgent": true});
    let stub = check("Agent", &response, &parts.join("\n"));
    let content = stub["content"].as_array().unwrap();
    assert_eq!(content.len(), 1, "no block of the report survives");
    assert!(
        content[0]["text"]
            .as_str()
            .unwrap()
            .starts_with("⟪ICX:STUB")
    );
    assert_eq!(stub["agentId"], "a1b2c3");
}

#[test]
fn an_object_with_code_and_result_is_not_taken_for_a_script() {
    // WebFetch's numeric HTTP `code` and an MCP-style status code are not Code Mode scripts
    // (review of #88): every field is kept, and nothing is replaced as if it were `result`.
    let mut mmu = InfinityContextMmu::default();
    let response = json!({"code": 200, "result": "ok", "items": lines(600, "item")});
    let out = ClaudeCodeAdapter::handle_hook(
        "PostToolUse",
        &post_tool_use("mcp__svc__list", &response).to_string(),
        &mut mmu,
    )
    .unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    let stub = &v["hookSpecificOutput"]["updatedToolOutput"];
    assert_eq!(stub["code"], 200, "the code is kept");
    assert_eq!(stub["result"], "ok", "the small result is not overwritten");
    assert!(stub["items"].as_str().unwrap().starts_with("⟪ICX:STUB"));
    let id = ArtifactId::hash_bytes(response.to_string().as_bytes()).0;
    assert!(
        mmu.cas.get_meta(id).is_some(),
        "stored whole, not just `result`"
    );
}

#[test]
fn a_mixed_agent_report_keeps_its_image_blocks() {
    let image = json!({"type": "image", "source": {"type": "base64", "media_type": "image/png",
        "data": "iVBORw0KGgo"}});
    let response = json!({"status": "completed", "prompt": "p", "agentId": "a1",
        "agentType": "general-purpose", "content": [
            {"type": "text", "text": lines(900, "report")}, image.clone()]});
    let mut mmu = InfinityContextMmu::default();
    let out = ClaudeCodeAdapter::handle_hook(
        "PostToolUse",
        &post_tool_use("Agent", &response).to_string(),
        &mut mmu,
    )
    .unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    let content = v["hookSpecificOutput"]["updatedToolOutput"]["content"]
        .as_array()
        .unwrap();
    assert!(
        content[0]["text"]
            .as_str()
            .unwrap()
            .starts_with("⟪ICX:STUB")
    );
    assert_eq!(content[1], image, "the image block is kept");
    assert_eq!(content.len(), 2);
    let text = lines(900, "report");
    let id = ArtifactId::hash_bytes(text.as_bytes()).0;
    assert!(
        mmu.cas.get_meta(id).is_some(),
        "the stored artifact is the text blocks' text"
    );
}

#[test]
fn builtin_shapes_are_used_only_for_their_own_tool() {
    // An MCP tool's result with WebFetch-like or Grep-like keys (review of #88): the whole result
    // is stored, and the stub replaces its largest field, which is also stored.
    for response in [
        json!({"url": "https://x.invalid", "code": 200, "result": "ok", "body": lines(600, "body")}),
        json!({"mode": "full", "content": "x", "items": lines(600, "item")}),
    ] {
        let mut mmu = InfinityContextMmu::default();
        let out = ClaudeCodeAdapter::handle_hook(
            "PostToolUse",
            &post_tool_use("mcp__svc__fetch", &response).to_string(),
            &mut mmu,
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        let id = ArtifactId::hash_bytes(response.to_string().as_bytes()).0;
        assert!(mmu.cas.get_meta(id).is_some(), "stored whole: {response}");
        let replaced = &v["hookSpecificOutput"]["updatedToolOutput"];
        let big = ["body", "items"]
            .into_iter()
            .find(|k| response.get(*k).is_some())
            .unwrap();
        assert!(replaced[big].as_str().unwrap().starts_with("⟪ICX:STUB"));
        for small in ["result", "content"] {
            if let Some(orig) = response.get(small) {
                assert_eq!(&replaced[small], orig, "{small} is kept");
            }
        }
    }
}

#[test]
fn webfetch_replaces_exactly_the_stored_result() {
    let response = json!({"url": "https://x.invalid", "code": 200, "codeText": "OK",
        "result": lines(600, "page"), "bytes": 1, "durationMs": 1});
    let stub = check("WebFetch", &response, response["result"].as_str().unwrap());
    assert!(stub["result"].as_str().unwrap().starts_with("⟪ICX:STUB"));
    assert_eq!(stub["url"], "https://x.invalid");
    assert_eq!(stub["code"], 200);
}

#[test]
fn a_small_report_beside_a_large_image_is_left_alone() {
    // Only the text is stored, so a short report is not stubbed, however large its image.
    let image = json!({"type": "image", "source": {"type": "base64", "media_type": "image/png",
        "data": "iVBORw0KGgo".repeat(5000)}});
    let response = json!({"status": "completed", "agentId": "a1", "content": [
        {"type": "text", "text": "done"}, image]});
    let mut mmu = InfinityContextMmu::default();
    let out = ClaudeCodeAdapter::handle_hook(
        "PostToolUse",
        &post_tool_use("Agent", &response).to_string(),
        &mut mmu,
    )
    .unwrap();
    assert!(!out.contains("updatedToolOutput"), "{out}");
}

#[test]
fn repl_results_that_are_not_strings_and_undefined_returns() {
    // A non-string `result` is stored as its JSON text, and the stub (a string) takes its place:
    // the model is told where the value went, whatever its type.
    let value: Vec<u32> = (1..=2000).collect();
    let response = json!({"code": "return rows", "result": value, "stdout": "", "stderr": ""});
    let stub = check("REPL", &response, &json!(value).to_string());
    assert!(stub["result"].as_str().unwrap().starts_with("⟪ICX:STUB"));
    assert_eq!(stub["code"], "return rows", "the script is kept");

    // A script returning `undefined` has no `result` key: it is stored and replaced as a
    // Bash-style `{stdout, stderr}` result.
    let printed = lines(600, "printed");
    let response = json!({"code": "console.log(x)", "stdout": printed, "stderr": ""});
    let stub = check("REPL", &response, &printed);
    assert!(stub["stdout"].as_str().unwrap().starts_with("⟪ICX:STUB"));
    assert_eq!(stub["code"], "console.log(x)");
}

#[test]
fn binary_blocks_without_a_mime_type_are_kept() {
    // MCP makes `mimeType` optional; the block's type, or a `blob` beside a `uri`, decides
    // (review of #88).
    let data = "iVBORw0KGgo".repeat(5000);
    for binary in [
        json!({"type": "image", "data": data}),
        json!({"type": "audio", "data": data}),
        json!({"type": "resource", "resource": {"uri": "file:///x.bin", "blob": data}}),
    ] {
        let response = json!({"content": [{"type": "text", "text": lines(800, "caption")},
            binary.clone()]});
        let mut mmu = InfinityContextMmu::default();
        let out = ClaudeCodeAdapter::handle_hook(
            "PostToolUse",
            &post_tool_use("mcp__svc__get", &response).to_string(),
            &mut mmu,
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        let replaced = &v["hookSpecificOutput"]["updatedToolOutput"];
        assert!(
            replaced["content"][0]["text"]
                .as_str()
                .unwrap()
                .starts_with("⟪ICX:STUB"),
            "{binary}"
        );
        assert_eq!(replaced["content"][1], binary, "the binary block is kept");
        assert!(
            v["hookSpecificOutput"]
                .get("updatedMCPToolOutput")
                .is_none()
        );
        // Alone, with no text to replace, it is not replaced at all.
        let alone = json!({"content": [binary.clone()]});
        let mut mmu = InfinityContextMmu::default();
        let out = ClaudeCodeAdapter::handle_hook(
            "PostToolUse",
            &post_tool_use("mcp__svc__get", &alone).to_string(),
            &mut mmu,
        )
        .unwrap();
        assert!(!out.contains("updatedToolOutput"), "{binary}: {out}");
    }
}

#[test]
fn resources_and_links_keep_the_mcp_result_whole() {
    // `updatedMCPToolOutput` would replace the whole result with one string, dropping these
    // blocks; only the shape-preserving replacement is sent (review of #88).
    for block in [
        json!({"type": "resource", "resource": {"uri": "file:///a.txt", "text": "short"}}),
        json!({"type": "resource_link", "uri": "file:///b.txt", "name": "b"}),
    ] {
        let response = json!({"content": [{"type": "text", "text": lines(800, "body")},
            block.clone()]});
        let mut mmu = InfinityContextMmu::default();
        let out = ClaudeCodeAdapter::handle_hook(
            "PostToolUse",
            &post_tool_use("mcp__svc__get", &response).to_string(),
            &mut mmu,
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert!(
            v["hookSpecificOutput"]
                .get("updatedMCPToolOutput")
                .is_none(),
            "{block}"
        );
        assert_eq!(
            v["hookSpecificOutput"]["updatedToolOutput"]["content"][1],
            block
        );
    }
    // An all-text result still gets it.
    let text = json!({"content": [{"type": "text", "text": lines(800, "body")}]});
    let mut mmu = InfinityContextMmu::default();
    let out = ClaudeCodeAdapter::handle_hook(
        "PostToolUse",
        &post_tool_use("mcp__svc__get", &text).to_string(),
        &mut mmu,
    )
    .unwrap();
    assert!(out.contains("updatedMCPToolOutput"));
}

#[test]
fn structured_mcp_json_with_its_own_type_fields_is_still_replaced_whole() {
    // A domain object's `type` is data, not a content block (review of #88).
    let items: Vec<Value> = (0..400)
        .map(|i| json!({"type": "issue", "id": i, "title": format!("issue number {i}")}))
        .collect();
    let response = json!({"type": "FeatureCollection", "items": items});
    let mut mmu = InfinityContextMmu::default();
    let out = ClaudeCodeAdapter::handle_hook(
        "PostToolUse",
        &post_tool_use("mcp__tracker__list", &response).to_string(),
        &mut mmu,
    )
    .unwrap();
    assert!(out.contains("updatedMCPToolOutput"), "{out}");
}
