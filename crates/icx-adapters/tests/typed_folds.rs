// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later AND AGPL-3.0-or-later

//! Typed folds through `icx_write` (v1.5.0, T-73-004): the type is recorded, unknown types are
//! refused, and a folded branch expands back to its exact content.

use icx_adapters::mcp::{FOLD_TYPES, McpPagerServer};
use icx_adapters::mmu::InfinityContextMmu;
use icx_core::AgentKind;
use serde_json::json;

fn fold_id(reply: &str) -> String {
    reply
        .split("BRANCH_FOLDED│")
        .nth(1)
        .and_then(|r| r.split(['>', '⟫']).next())
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn a_typed_fold_records_its_type_and_expands_back_exactly() {
    for kind in FOLD_TYPES {
        let mut mmu = InfinityContextMmu::default();
        let open =
            McpPagerServer::execute_icx_op(&json!({"op": "fold", "q": "begin:build"}), &mut mmu);
        assert!(open.contains("BRANCH_OPENED"), "{open}");
        let log = format!("Compiling icx v1.5.0\nwarning: unused import ({kind})\nFinished\n");
        mmu.ingest_payload(AgentKind::ClaudeCode, "Bash:cargo build", &log, false);
        let folded = McpPagerServer::execute_icx_op(
            &json!({"op": "fold", "q": "build is clean", "type": kind}),
            &mut mmu,
        );
        assert!(folded.contains("BRANCH_FOLDED"), "{kind}: {folded}");
        let id = fold_id(&folded);
        let expanded = McpPagerServer::execute_icx_op(&json!({"op": "expand", "id": id}), &mut mmu);
        assert!(
            expanded.contains(&format!("[{kind}] build is clean")),
            "{kind}: {expanded}"
        );
        // Each folded step is a handle whose dereference returns its exact text.
        let step = expanded
            .lines()
            .find(|l| l.contains("cargo_build"))
            .and_then(|l| l.split('↦').nth(1))
            .map(|h| h[..21].to_owned())
            .expect("the folded step");
        let back = McpPagerServer::execute_icx_op(
            &json!({"op": "deref", "id": step, "raw": true}),
            &mut mmu,
        );
        assert!(back.contains(&log), "{kind}: content lost: {back}");
    }
}

#[test]
fn unknown_fold_types_are_refused_and_untyped_folds_are_unchanged() {
    let mut mmu = InfinityContextMmu::default();
    McpPagerServer::execute_icx_op(&json!({"op": "fold", "q": "begin:x"}), &mut mmu);
    let refused = McpPagerServer::execute_icx_op(
        &json!({"op": "fold", "q": "done", "type": "secret"}),
        &mut mmu,
    );
    assert!(refused.starts_with("ERR"), "{refused}");
    assert_eq!(mmu.dag.active_branch_depth(), 1, "nothing was folded");
    let refused =
        McpPagerServer::execute_icx_op(&json!({"op": "fold", "q": "done", "type": 3}), &mut mmu);
    assert!(refused.starts_with("ERR"), "{refused}");
    // The refused value is not echoed.
    let long = "z".repeat(5_000);
    let refused =
        McpPagerServer::execute_icx_op(&json!({"op": "fold", "q": "done", "type": long}), &mut mmu);
    assert!(
        refused.starts_with("ERR") && refused.len() < 200,
        "{refused}"
    );
    // A type on begin: is refused and opens nothing.
    let refused = McpPagerServer::execute_icx_op(
        &json!({"op": "fold", "q": "begin:y", "type": "subtask"}),
        &mut mmu,
    );
    assert!(refused.starts_with("ERR"), "{refused}");
    assert_eq!(mmu.dag.active_branch_depth(), 1);
    let plain = McpPagerServer::execute_icx_op(&json!({"op": "fold", "q": "done"}), &mut mmu);
    let expanded =
        McpPagerServer::execute_icx_op(&json!({"op": "expand", "id": fold_id(&plain)}), &mut mmu);
    assert!(!expanded.contains("[other]"), "{expanded}");
}

#[test]
fn the_read_only_tool_still_has_no_fold_and_the_write_tool_lists_the_types() {
    let schema = McpPagerServer::tool_schema();
    let tools = schema["tools"].as_array().unwrap();
    let write = tools.iter().find(|t| t["name"] == "icx_write").unwrap();
    let types: Vec<&str> = write["inputSchema"]["properties"]["type"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(types, FOLD_TYPES);
    let read = tools.iter().find(|t| t["name"] == "icx").unwrap();
    assert!(read["inputSchema"]["properties"].get("type").is_none());
}
