//! v1.3.2 (#34, #37): a hook running in a git worktree stores into the main project's
//! workspace, and an MCP server follows the workspace its agent's hooks use, so stubs written
//! by a worktree subagent or after the session moved to a nested project can be dereferenced.
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
        .env_remove("ICX_DIR")
        .env_remove("ICX_MCP_FOLLOW_HOOKS");
    cmd
}

fn git(cwd: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(cwd)
        // Independent of the user's git config (commit signing through an agent).
        .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
        .args(["-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false"])
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {out:?}");
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

fn deref(base: &Path, cwd: &Path, handle: &str, follow: bool) -> String {
    let mut cmd = icx(base, cwd);
    cmd.arg("mcp-serve");
    if !follow {
        cmd.env("ICX_MCP_FOLLOW_HOOKS", "0");
    }
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
fn worktree_stubs_resolve_from_the_main_project_and_from_a_following_mcp_server() {
    let base = base_dir("workspace-follow");
    let main = base.join("main");
    std::fs::create_dir_all(&main).unwrap();
    git(&main, &["init", "-q"]);
    std::fs::write(main.join("README"), "x\n").unwrap();
    git(&main, &["add", "README"]);
    git(&main, &["commit", "-q", "-m", "init"]);
    git(&main, &["worktree", "add", "-q", "../wt", "-b", "agent"]);
    let wt = base.join("wt");
    let other = base.join("other");
    std::fs::create_dir_all(&other).unwrap();
    git(&other, &["init", "-q"]);

    // A PostToolUse hook inside the worktree stubs a large output (over the 2 KiB stub floor).
    let lines = (1..=1000)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let payload = serde_json::json!({
        "hook_event_name": "PostToolUse", "session_id": "s", "cwd": wt,
        "tool_name": "Bash", "tool_input": {"command": "seq 1 1000"},
        "tool_response": {"stdout": lines, "stderr": "", "interrupted": false, "isImage": false}
    });
    let mut hook = icx(&base, &wt);
    hook.args(["hook", "--agent", "claude-code", "--event", "PostToolUse"]);
    let out = piped(hook, &payload.to_string());
    let handle = out
        .split("#icx:")
        .nth(1)
        .map(|rest| format!("#icx:{}", &rest[..16]))
        .unwrap_or_else(|| panic!("no stub in hook output: {out}"));

    // #37: the worktree resolves to the main project's workspace.
    let from_main = deref(&base, &main, &handle, false);
    assert!(
        from_main.contains("1000") && !from_main.contains("not found"),
        "{from_main}"
    );
    // #34: an MCP server launched elsewhere follows the workspace its agent's hooks used.
    let followed = deref(&base, &other, &handle, true);
    assert!(
        followed.contains("1000") && !followed.contains("not found"),
        "{followed}"
    );
    // Pinned to an unrelated project, the error names the workspace it searched.
    let pinned = deref(&base, &other, &handle, false);
    assert!(
        pinned.contains("not found") && pinned.contains("searched workspace"),
        "{pinned}"
    );
    let _ = std::fs::remove_dir_all(&base);
}
