//! v1.2.0 protocol regressions for ICX-CODEX-031 and ICX-CODEX-035.
use icx_adapters::{CodexAdapter, InfinityContextMmu, McpPagerServer};
use serde_json::{Value, json};

fn rpc(request: &Value, mmu: &mut InfinityContextMmu) -> Value {
    McpPagerServer::handle_jsonrpc(&request.to_string(), mmu)
}

#[test]
fn invalid_rpc_envelopes_are_rejected_before_dispatch() {
    let mut mmu = InfinityContextMmu::default();
    for request in [
        json!({"id":1,"method":"tools/list"}),
        json!({"jsonrpc":"1.0","id":1,"method":"tools/list"}),
        json!({"jsonrpc":"2.0","id":{},"method":"tools/list"}),
        json!({"jsonrpc":"2.0","id":true,"method":"tools/list"}),
        json!({"jsonrpc":"2.0","id":1.5,"method":"tools/list"}),
        json!({"jsonrpc":"2.0","id":null,"method":"tools/list"}),
        json!({"jsonrpc":"2.0","id":1,"method":4}),
        json!([]),
    ] {
        let response = rpc(&request, &mut mmu);
        assert_eq!(response["error"]["code"], -32600, "{request}: {response}");
        assert_eq!(response["id"], Value::Null);
        assert!(!McpPagerServer::requires_workspace_state(
            &request.to_string()
        ));
        assert!(!McpPagerServer::is_mutating_request(&request.to_string()));
    }
}

#[test]
fn invalid_tool_arguments_are_errors_and_do_not_mutate_state() {
    let mut mmu = InfinityContextMmu::default();
    for args in [
        json!({}),
        json!({"op":"TYPO"}),
        json!({"op":1}),
        json!([]),
        json!({"op":"fold","q":false}),
        json!({"op":"recall","k":1.5}),
        json!({"op":"status","id":true}),
        json!({"op":"status","provenance":"yes"}),
        json!({"op":"slice","id":"#icx:0000000000000000","range":["1",2]}),
    ] {
        let request = json!({"jsonrpc":"2.0","id":"bad-args","method":"tools/call",
                             "params":{"name":"icx","arguments":args}});
        let response = rpc(&request, &mut mmu);
        assert_eq!(response["result"]["isError"], true, "{request}: {response}");
        assert!(!McpPagerServer::is_mutating_request(&request.to_string()));
    }
    assert_eq!(mmu.cas.artifact_count(), 0);
    assert_eq!(mmu.dag.active_branch_depth(), 0);
}

#[test]
fn malformed_call_params_report_invalid_params() {
    let mut mmu = InfinityContextMmu::default();
    for params in [
        json!({}),
        json!({"name":1}),
        json!({"name":"icx"}),
        json!([]),
    ] {
        let response = rpc(
            &json!({"jsonrpc":"2.0","id":2,"method":"tools/call",
                                  "params":params}),
            &mut mmu,
        );
        assert_eq!(response["error"]["code"], -32602, "{response}");
        assert_eq!(response["id"], 2);
    }
}

#[test]
fn pager_like_hook_aliases_are_not_advertised_mcp_tool_names() {
    let mut mmu = InfinityContextMmu::default();
    for name in [
        "ICX",
        " icx ",
        "mcp__other__icx",
        "mcp__icx__other",
        "mcp__infinity_context_fake",
    ] {
        let request = json!({"jsonrpc":"2.0","id":7,"method":"tools/call",
            "params":{"name":name,"arguments":{"op":"fold","q":"begin:wrong-tool"}}});
        let response = rpc(&request, &mut mmu);
        assert_eq!(response["error"]["code"], -32602, "{request}: {response}");
        assert!(!McpPagerServer::requires_workspace_state(
            &request.to_string()
        ));
        assert!(!McpPagerServer::is_mutating_request(&request.to_string()));
    }
    assert_eq!(mmu.dag.active_branch_depth(), 0);
}

#[test]
fn workspace_access_and_mutation_require_a_valid_named_tool_call() {
    // Each op on its own tool (ADR-0009).
    for (tool, op, mutating) in [("icx", "status", false), ("icx_write", "fold", true)] {
        let valid = json!({"jsonrpc":"2.0","id":1,"method":"tools/call",
                           "params":{"name":tool,"arguments":{"op":op}}});
        assert!(McpPagerServer::requires_workspace_state(&valid.to_string()));
        assert_eq!(
            McpPagerServer::is_mutating_request(&valid.to_string()),
            mutating
        );
        let mut unknown = valid.clone();
        unknown["params"]["name"] = json!("other_tool");
        let mut unnamed = valid.clone();
        unnamed["params"].as_object_mut().unwrap().remove("name");
        let mut notification = valid.clone();
        notification.as_object_mut().unwrap().remove("id");
        let mut bad_args = valid;
        bad_args["params"]["arguments"] = json!({});
        for request in [unknown, unnamed, notification, bad_args] {
            assert!(
                !McpPagerServer::requires_workspace_state(&request.to_string()),
                "{request}"
            );
            assert!(
                !McpPagerServer::is_mutating_request(&request.to_string()),
                "{request}"
            );
        }
    }
}

#[test]
fn valid_notifications_are_silent_and_explicit_status_still_works() {
    let mut mmu = InfinityContextMmu::default();
    assert!(
        rpc(
            &json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            &mut mmu
        )
        .is_null()
    );
    for id in [json!(0), json!(-4), json!("request")] {
        let response = rpc(
            &json!({"jsonrpc":"2.0","id":id,"method":"tools/call",
                                  "params":{"name":"icx","arguments":{"op":"status"}}}),
            &mut mmu,
        );
        assert_eq!(response["id"], id);
        assert_eq!(response["result"]["isError"], false);
    }
}

#[test]
fn codex_large_mcp_outputs_are_captured_without_unsupported_replacement() {
    let mut mmu = InfinityContextMmu::default();
    let raw = "ordinary file output\n".repeat(100);
    let response: Value = serde_json::from_str(
        &CodexAdapter::handle_hook(
            "PostToolUse",
            &json!({"tool_name":"mcp__fs__read", "tool_response":raw}).to_string(),
            &mut mmu,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(response["continue"], true);
    assert_eq!(
        response["hookSpecificOutput"],
        json!({"hookEventName":"PostToolUse"})
    );
    assert_eq!(mmu.cas.artifact_count(), 1);
    let prompt: Value = serde_json::from_str(
        &CodexAdapter::handle_hook(
            "UserPromptSubmit",
            r#"{"prompt":"ordinary file output"}"#,
            &mut mmu,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(prompt["hookSpecificOutput"]["additionalContext"].is_string());
}
