//! v1.3.1 (#28, #30): every value-taking flag rejects a missing, flag-shaped or blank value
//! instead of taking the next flag as its value; `icx hook` still fails open.
use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run(base: &std::path::Path, args: &[&str], stdin: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_icx"))
        .env("ICX_CONFIG_HOME", base.join("config"))
        .env("ICX_STATE_HOME", base.join("state"))
        .env("XDG_CONFIG_HOME", base.join("xdg-config"))
        .env("XDG_STATE_HOME", base.join("xdg-state"))
        .env("XDG_RUNTIME_DIR", base.join("runtime"))
        .env("ICX_NO_DAEMON", "1")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    child.wait_with_output().unwrap()
}

fn base(name: &str) -> std::path::PathBuf {
    let b = std::env::temp_dir()
        .join(format!(
            "icx-v131-tests-{}",
            std::env::var("USER").unwrap_or_default()
        ))
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&b);
    std::fs::create_dir_all(&b).unwrap();
    b
}

#[test]
fn value_taking_flags_reject_flag_shaped_missing_or_blank_values() {
    let b = base("flags");
    let ws = b.join("ws");
    let ws = ws.to_str().unwrap();
    let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    for args in [
        &[
            "deref",
            "#icx:0123456789abcdef",
            "--lines",
            "--raw",
            "--dir",
            ws,
        ][..],
        &["deref", "#icx:0123456789abcdef", "--dir", ws, "--lines"],
        &[
            "deref",
            "#icx:0123456789abcdef",
            "--dir",
            ws,
            "--lines",
            "  ",
        ],
        &[
            "context-budget",
            "--dir",
            ws,
            "--context-tokens",
            "--external-reserve",
        ],
        &["encode-ahtl", manifest, "--profile", "--dir", ws],
        &["doctor", "--dir", "--json"],
    ] {
        let out = run(&b, args, b"");
        assert!(!out.status.success(), "{args:?} accepted: {out:?}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("requires a value"),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    #[cfg(unix)]
    for args in [
        &["daemon", "--dir", ws, "--idle-exit", "--max-requests", "3"][..],
        &["daemon", "--dir", ws, "--max-requests"],
    ] {
        let out = run(&b, args, b"");
        assert!(!out.status.success(), "{args:?} accepted: {out:?}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("requires a value"),
            "{out:?}"
        );
    }
    for args in [
        &["proxy", "--upstream", "--dir", ws][..],
        &["proxy", "--listen", "--dir", ws],
        &["proxy", "--provider", "", "--dir", ws],
        &["proxy-transform", "--provider", "--dir", ws],
        &["proxy-transform", "--dir", ws, "--provider"],
    ] {
        let out = run(&b, args, b"");
        assert!(!out.status.success(), "{args:?} accepted: {out:?}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("requires a value"),
            "{out:?}"
        );
    }
    let _ = std::fs::remove_dir_all(&b);
}

/// #30: blank `--agent` / `--event` values are rejected like `--dir`; the hook fails open with
/// the Claude Code no-op and the default event.
#[test]
fn blank_agent_and_event_values_fail_open() {
    let b = base("blank");
    let ws = b.join("ws");
    for args in [
        &[
            "hook",
            "--agent",
            "",
            "--event",
            "Stop",
            "--dir",
            ws.to_str().unwrap(),
        ][..],
        &[
            "hook",
            "--agent",
            "agy",
            "--event",
            "   ",
            "--dir",
            ws.to_str().unwrap(),
        ],
    ] {
        let out = run(&b, args, b"{}");
        assert!(out.status.success(), "{out:?}");
        let reply = String::from_utf8_lossy(&out.stdout);
        assert!(
            serde_json::from_str::<serde_json::Value>(reply.trim()).is_ok(),
            "{reply}"
        );
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("requires a value"),
            "{out:?}"
        );
    }
    let _ = std::fs::remove_dir_all(&b);
}
