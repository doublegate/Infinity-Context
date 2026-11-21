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
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "{}");
    let _ = std::fs::remove_dir_all(&base);
}
