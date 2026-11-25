// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later AND AGPL-3.0-or-later

//! `icx rescan` (v1.5.0, T-74-001) end to end: a report over a real workspace that changes
//! nothing, `--apply` refused, and argument errors.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn root(tag: &str) -> PathBuf {
    let r = std::env::temp_dir().join(format!("icx-rescan-cli-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&r);
    std::fs::create_dir_all(&r).unwrap();
    r
}

fn icx(root: &Path, args: &[&str], stdin: Option<&str>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_icx"));
    cmd.args(args)
        .current_dir(root)
        .env("ICX_STATE_HOME", root.join("state"))
        .env("ICX_CONFIG_HOME", root.join("config"))
        .env_remove("ICX_DIR")
        .env("XDG_STATE_HOME", root.join("xdg-state"))
        .env("XDG_RUNTIME_DIR", root.join("run"))
        .env("ICX_NO_DAEMON", "1")
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    if let Some(text) = stdin {
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(text.as_bytes())
            .unwrap();
    }
    child.wait_with_output().unwrap()
}

#[test]
fn a_report_over_a_workspace_changes_nothing() {
    let r = root("report");
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {"command": "cat settings.py"},
        "tool_response": {"stdout": "secret_key = settings.SECRET_KEY\n".repeat(200)}
    })
    .to_string();
    let o = icx(
        &r,
        &["hook", "--agent", "claude-code", "--event", "PostToolUse"],
        Some(&payload),
    );
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let before = String::from_utf8_lossy(&icx(&r, &["status"], None).stdout).into_owned();
    let o = icx(&r, &["rescan", "--limit", "5"], None);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let report: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(report["mode"], "report");
    assert!(report["artifacts_scanned"].as_u64().unwrap() >= 1);
    // Stored under the current redactor, so nothing more to hide.
    assert_eq!(report["artifacts_affected"], 0);
    let after = String::from_utf8_lossy(&icx(&r, &["status"], None).stdout).into_owned();
    assert_eq!(before, after, "the report changed the workspace");
    let _ = std::fs::remove_dir_all(&r);
}

#[test]
fn apply_and_bad_arguments_are_refused() {
    let r = root("errors");
    for (args, needle) in [
        (vec!["rescan", "--apply"], "#102"),
        (vec!["rescan", "--limit", "ten"], "needs a number"),
        (vec!["rescan", "--bogus", "1"], "unknown option"),
        (vec!["rescan", "--bogus"], "unknown option"),
        (
            vec!["rescan", "--limit", "1", "--limit", "2"],
            "more than once",
        ),
        (
            vec!["rescan", "--dir", "/nonexistent/icx-rescan"],
            "no ready workspace",
        ),
    ] {
        let o = icx(&r, &args, None);
        assert!(!o.status.success(), "{args:?} succeeded");
        let err = String::from_utf8_lossy(&o.stderr);
        assert!(err.contains(needle), "{args:?}: {err}");
    }
    let _ = std::fs::remove_dir_all(&r);
}
