//! T-64-003: a Claude Code `CwdChanged` hook resolves the workspace from `new_cwd`, so the agent
//! (and its MCP server, which follows the workspace its hooks record) switches to the project it
//! moved into. Payload fields as recorded live on Claude Code 2.1.330.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn hook(root: &Path, cwd: &Path, event: &str, payload: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_icx"))
        .args(["hook", "--agent", "claude-code", "--event", event])
        .current_dir(cwd)
        .env("ICX_STATE_HOME", root.join("state"))
        .env("ICX_CONFIG_HOME", root.join("config"))
        .env_remove("ICX_DIR")
        .env("XDG_STATE_HOME", root.join("xdg-state"))
        .env("XDG_RUNTIME_DIR", root.join("run"))
        .env("ICX_NO_DAEMON", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("run icx");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn git_repo(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    let ok = Command::new("git")
        .args(["init", "-q"])
        .current_dir(dir)
        .status()
        .is_ok_and(|s| s.success());
    assert!(ok, "git init");
}

fn workspaces(root: &Path) -> Vec<PathBuf> {
    let dir = root.join("state/workspaces");
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|it| it.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

#[test]
fn cwd_changed_moves_the_agent_to_the_new_projects_workspace() {
    let root = std::env::temp_dir().join(format!("icx-cwd-changed-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (a, b) = (root.join("repo-a"), root.join("repo-b"));
    git_repo(&a);
    git_repo(&b);
    std::fs::create_dir_all(b.join("sub")).unwrap();

    // Run from A, moving into a subdirectory of B: only B's workspace is created.
    let payload = serde_json::json!({
        "session_id": "s1", "transcript_path": "/tmp/t.jsonl", "prompt_id": "p1",
        "hook_event_name": "CwdChanged",
        "cwd": b.join("sub"), "old_cwd": a, "new_cwd": b.join("sub")
    })
    .to_string();
    assert_eq!(hook(&root, &a, "CwdChanged", &payload).trim(), "{}");
    let after_move = workspaces(&root);
    assert_eq!(after_move.len(), 1, "{after_move:?}");

    // An ordinary hook from B's root uses the same workspace; one from A creates A's.
    hook(&root, &b, "SessionEnd", "{}");
    assert_eq!(
        workspaces(&root),
        after_move,
        "B's root and B/sub share a workspace"
    );
    hook(&root, &a, "SessionEnd", "{}");
    assert_eq!(workspaces(&root).len(), 2, "A has its own workspace");

    // A relative or missing new_cwd is ignored: the hook's own directory is used.
    let bad =
        serde_json::json!({"hook_event_name": "CwdChanged", "new_cwd": "relative/dir"}).to_string();
    hook(&root, &a, "CwdChanged", &bad);
    assert_eq!(workspaces(&root).len(), 2);
    let _ = std::fs::remove_dir_all(&root);
}
