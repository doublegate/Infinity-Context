// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later AND AGPL-3.0-or-later

//! #117: handles the wire proxy writes into folded history live in the proxy's own store. The
//! proxy registers that store, and `icx mcp-serve` running for another project resolves the
//! handles from it (read-only); without the registration the error says not to retry.
#![cfg(target_os = "linux")]
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn base_dir(name: &str) -> PathBuf {
    let base = std::env::temp_dir()
        .join(format!(
            "icx-tests-{}",
            std::env::var("USER").unwrap_or_default()
        ))
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    std::fs::canonicalize(&base).unwrap()
}

fn icx(base: &Path, cwd: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_icx"));
    cmd.current_dir(cwd)
        .env("ICX_CONFIG_HOME", base.join("config"))
        .env("ICX_STATE_HOME", base.join("state"))
        .env("XDG_RUNTIME_DIR", base.join("runtime"))
        .env("ICX_NO_DAEMON", "1")
        .env("ICX_MCP_FOLLOW_HOOKS", "0")
        .env_remove("ICX_DIR");
    cmd
}

fn piped(mut cmd: Command, stdin: &str) -> String {
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{out:?}");
    String::from_utf8(out.stdout).unwrap()
}

fn deref(base: &Path, cwd: &Path, handle: &str) -> String {
    let mut cmd = icx(base, cwd);
    cmd.arg("mcp-serve");
    let call = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {"name": "icx", "arguments": {"op": "deref", "id": handle}}
    });
    let input = format!(
        "{}\n{}\n{call}\n",
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"1"}}}"#,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#
    );
    piped(cmd, &input)
}

#[test]
fn mcp_serve_resolves_handles_from_a_registered_proxy_store() {
    let base = base_dir("proxy-store");
    let project = base.join("project");
    std::fs::create_dir_all(&project).unwrap();
    let proxy_ws = base.join("proxy-ws");
    // The agent's own project workspace, and the proxy's separate store.
    let out = icx(&base, &project).arg("init").output().unwrap();
    assert!(out.status.success(), "{out:?}");
    let out = icx(&base, &project)
        .args(["init", "--dir"])
        .arg(&proxy_ws)
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");

    // A request whose history holds a large tool result, which the proxy folds.
    let mut big = String::new();
    for k in 0..200 {
        big.push_str(&format!("row {k} marker-qqzz value {}\n", k * 7));
    }
    let body = serde_json::json!({
        "model": "m", "max_tokens": 8,
        "messages": [
            {"role": "user", "content": "run it"},
            {"role": "assistant", "content": [{"type": "tool_use", "id": "t1", "name": "Bash", "input": {"command": "ls"}}]},
            {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "t1", "content": big}]},
            {"role": "assistant", "content": "done"},
            {"role": "user", "content": "next"},
            {"role": "assistant", "content": "ok"},
            {"role": "user", "content": "again"}
        ]
    });
    let mut cmd = icx(&base, &project);
    cmd.args(["proxy-transform", "--provider", "anthropic", "--dir"])
        .arg(&proxy_ws);
    let out = piped(cmd, &body.to_string());
    assert!(out.contains("src:proxy"), "proxy packets are marked: {out}");
    let at = out.find("#icx:").expect("a folded handle");
    let handle = &out[at..at + 5 + 16];

    // Resolved from the registered proxy store although the project workspace lacks it.
    let resolved = deref(&base, &project, handle);
    assert!(resolved.contains("marker-qqzz"), "{resolved}");
    assert!(!resolved.contains("\"isError\":true"), "{resolved}");

    // Without the registration the handle is unresolvable, and the error says not to retry.
    std::fs::remove_file(base.join("state").join("proxy-stores")).unwrap();
    let missing = deref(&base, &project, handle);
    assert!(missing.contains("not found"), "{missing}");
    assert!(missing.contains("do not retry"), "{missing}");
    let _ = std::fs::remove_dir_all(&base);
}
