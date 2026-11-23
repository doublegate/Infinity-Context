//! T-65-004: agy runs hook commands from `~/.gemini/config`, not from the project, so
//! `icx hook --agent agy` takes the project from the payload's `workspacePaths[0]` (then `cwd`).
//! This lets the installer register `icx hook` directly, with no directory-changing wrapper.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn hook(root: &Path, cwd: &Path, payload: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_icx"))
        .args(["hook", "--agent", "agy", "--event", "PreInvocation"])
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
    let mut v: Vec<PathBuf> = std::fs::read_dir(root.join("state/workspaces"))
        .map(|it| it.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

#[test]
fn agy_hooks_use_the_project_named_in_the_payload() {
    let root = std::env::temp_dir().join(format!("icx-agy-ws-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (project, config_dir) = (root.join("project"), root.join("gemini-config"));
    git_repo(&project);
    std::fs::create_dir_all(&config_dir).unwrap();

    // Run from agy's config directory, as agy does: the payload names the project.
    let payload = serde_json::json!({
        "hookEventName": "PreInvocation",
        "cwd": config_dir,
        "workspacePaths": [project],
    })
    .to_string();
    let out = hook(&root, &config_dir, &payload);
    assert!(out.contains("injectSteps"), "{out}");
    let first = workspaces(&root);
    assert_eq!(first.len(), 1, "{first:?}");

    // A hook run from inside the project uses that same workspace.
    hook(&root, &project, r#"{"hookEventName": "PreInvocation"}"#);
    assert_eq!(
        workspaces(&root),
        first,
        "the payload's project is the project's workspace"
    );

    // A relative or missing path is ignored, and the hook's own directory is used.
    let other = root.join("other");
    git_repo(&other);
    hook(
        &root,
        &other,
        r#"{"hookEventName": "PreInvocation", "workspacePaths": ["relative/dir"]}"#,
    );
    assert_eq!(workspaces(&root).len(), 2, "other has its own workspace");
    let _ = std::fs::remove_dir_all(&root);
}
