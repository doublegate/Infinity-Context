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
    assert!(stub["filenames"].is_array(), "filenames stays a list");
}

#[test]
fn unknown_and_image_shapes_are_kept_as_json() {
    let mut mmu = InfinityContextMmu::default();
    let image = json!({"type": "image", "file": {"base64": "iVBORw0KGgo".repeat(800),
        "type": "image/png"}});
    ClaudeCodeAdapter::handle_hook(
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
