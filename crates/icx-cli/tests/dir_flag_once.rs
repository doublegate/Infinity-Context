//! v1.2.2 (issue #24): every subcommand rejects a repeated `--dir`; `icx hook` fails open.
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn repeated_dir_is_rejected_and_hooks_fail_open() {
    let base = std::env::temp_dir()
        .join(format!(
            "icx-v122-tests-{}",
            std::env::var("USER").unwrap_or_default()
        ))
        .join(format!("dir-once-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let (a, b) = (base.join("a"), base.join("b"));
    let cmd = |args: &[&str]| {
        let mut c = Command::new(env!("CARGO_BIN_EXE_icx"));
        c.env("ICX_CONFIG_HOME", base.join("config"))
            .env("ICX_STATE_HOME", base.join("state"))
            .env("XDG_CONFIG_HOME", base.join("xdg-config"))
            .env("XDG_STATE_HOME", base.join("xdg-state"))
            .env("XDG_RUNTIME_DIR", base.join("runtime"))
            .env("ICX_NO_DAEMON", "1")
            .args(args)
            .args(["--dir", a.to_str().unwrap(), "--dir", b.to_str().unwrap()]);
        c
    };
    for sub in [&["init"][..], &["status"], &["trust"]] {
        let out = cmd(sub).output().unwrap();
        assert!(!out.status.success(), "{sub:?} accepted a repeated --dir");
        assert!(String::from_utf8_lossy(&out.stderr).contains("--dir given more than once"));
        assert!(!a.exists() && !b.exists(), "{sub:?} touched a workspace");
    }
    let mut child = cmd(&[
        "hook",
        "--agent",
        "claude-code",
        "--event",
        "UserPromptSubmit",
    ])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
    child.stdin.take().unwrap().write_all(b"{}").unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    // The agent's own no-op reply (Claude Code: `{"continue":true}`).
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        r#"{"continue":true}"#
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// v1.3.0 (Antigravity review of #25): a trailing `--dir` with no value is an error, not a silent
/// fall-back to the default workspace; `icx hook` still fails open with the agent's own reply.
#[test]
fn trailing_valueless_dir_is_rejected() {
    let base = std::env::temp_dir()
        .join(format!(
            "icx-v130-tests-{}",
            std::env::var("USER").unwrap_or_default()
        ))
        .join(format!("dir-trailing-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_icx"))
            .env("ICX_CONFIG_HOME", base.join("config"))
            .env("ICX_STATE_HOME", base.join("state"))
            .env("XDG_CONFIG_HOME", base.join("xdg-config"))
            .env("XDG_STATE_HOME", base.join("xdg-state"))
            .env("XDG_RUNTIME_DIR", base.join("runtime"))
            .env("ICX_NO_DAEMON", "1")
            .args(args)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    };
    for sub in [
        &["init", "--dir"][..],
        &["status", "--dir"],
        &["status", "--dir", "--raw"],
    ] {
        let out = run(sub);
        assert!(!out.status.success(), "{sub:?} accepted a valueless --dir");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("--dir requires a value"),
            "{out:?}"
        );
    }
    let dash = run(&["status", "--dir", "-h"]);
    assert!(
        !dash.status.success(),
        "single-dash flag taken as --dir value: {dash:?}"
    );
    let empty = run(&["status", "--dir", "  "]);
    assert!(
        !empty.status.success(),
        "whitespace --dir accepted: {empty:?}"
    );
    // agy's no-op reply depends on the event.
    let pre = run(&["hook", "--agent", "agy", "--event", "PreToolUse", "--dir"]);
    assert!(
        String::from_utf8_lossy(&pre.stdout).contains(r#""decision":"allow""#),
        "{pre:?}"
    );
    let stop = run(&["hook", "--agent", "agy", "--event", "Stop", "--dir"]);
    assert!(
        String::from_utf8_lossy(&stop.stdout).contains(r#""decision":"stop""#),
        "{stop:?}"
    );
    let upper = run(&["hook", "--agent", "agy", "--event", "STOP", "--dir"]);
    assert!(
        String::from_utf8_lossy(&upper.stdout).contains(r#""decision":"stop""#),
        "{upper:?}"
    );
    let end = run(&["hook", "--agent", "agy", "--event", "SessionEnd", "--dir"]);
    assert_eq!(String::from_utf8_lossy(&end.stdout).trim(), "{}", "{end:?}");
    // `--agent` without a value does not swallow `--event`.
    let noagent = run(&["hook", "--agent", "--event", "Stop", "--dir"]);
    assert_eq!(
        String::from_utf8_lossy(&noagent.stdout).trim(),
        r#"{"continue":true}"#,
        "{noagent:?}"
    );
    // A trailing valueless `--agent` keeps the earlier valid value.
    let dup = run(&[
        "hook", "--agent", "agy", "--event", "Stop", "--agent", "--dir",
    ]);
    assert!(
        String::from_utf8_lossy(&dup.stdout).contains(r#""decision":"stop""#),
        "{dup:?}"
    );
    let hook = run(&["hook", "--agent", "agy", "--event", "SessionStart", "--dir"]);
    assert!(hook.status.success());
    assert!(
        String::from_utf8_lossy(&hook.stdout).contains("injectSteps"),
        "{hook:?}"
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// Review of #27 (00:13 UTC): `execute_hook` treats `--agent --event Stop` like the fail-open
/// path does: `--event` is not taken as the agent name.
#[test]
fn hook_flag_without_value_does_not_swallow_the_next_flag() {
    let base = std::env::temp_dir()
        .join(format!(
            "icx-v130-tests-{}",
            std::env::var("USER").unwrap_or_default()
        ))
        .join(format!("flag-value-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_icx"))
        .env("ICX_CONFIG_HOME", base.join("config"))
        .env("ICX_STATE_HOME", base.join("state"))
        .env("XDG_CONFIG_HOME", base.join("xdg-config"))
        .env("XDG_STATE_HOME", base.join("xdg-state"))
        .env("XDG_RUNTIME_DIR", base.join("runtime"))
        .env("ICX_NO_DAEMON", "1")
        .args([
            "hook",
            "--agent",
            "--event",
            "Stop",
            "--dir",
            base.join("ws").to_str().unwrap(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"{}").unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{out:?}");
    let reply = String::from_utf8_lossy(&out.stdout);
    assert!(!reply.trim().is_empty(), "{out:?}");
    let _ = std::fs::remove_dir_all(&base);
}
