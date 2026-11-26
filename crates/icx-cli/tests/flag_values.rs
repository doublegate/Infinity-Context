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
    // A command that rejects its flags may exit before reading stdin; its exit status is what
    // these tests check, so a broken pipe here is not a failure (CI flake on #106).
    if let Err(e) = child.stdin.take().unwrap().write_all(stdin) {
        assert_eq!(e.kind(), std::io::ErrorKind::BrokenPipe, "{e}");
    }
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

/// #33: a repeated value-taking flag is rejected instead of silently keeping the last value, and
/// `deref` / `encode-ahtl` find their positional argument wherever it sits.
#[test]
fn repeated_flags_are_rejected_and_positionals_are_found_anywhere() {
    let b = base("repeated");
    let ws = b.join("ws");
    let ws = ws.to_str().unwrap();
    let init = run(&b, &["init", "--dir", ws], b"");
    assert!(init.status.success(), "{init:?}");
    for args in [
        &[
            "daemon",
            "--idle-exit",
            "1",
            "--idle-exit",
            "2",
            "--dir",
            ws,
        ][..],
        &[
            "daemon",
            "--max-requests",
            "1",
            "--max-requests",
            "2",
            "--dir",
            ws,
        ][..],
        &[
            "deref",
            "#icx:0000000000000000",
            "--lines",
            "1:2",
            "--lines",
            "3:4",
        ][..],
        &["proxy", "--provider", "openai", "--provider", "anthropic"][..],
        &[
            "proxy",
            "--listen",
            "127.0.0.1:1",
            "--listen",
            "127.0.0.1:2",
        ][..],
        &[
            "proxy",
            "--upstream",
            "http://a.invalid",
            "--upstream",
            "http://b.invalid",
        ][..],
        &[
            "proxy-transform",
            "--provider",
            "openai",
            "--provider",
            "anthropic",
        ][..],
        &["encode-ahtl", "f", "--profile", "a", "--profile", "b"][..],
    ] {
        let out = run(&b, args, b"{}");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            !out.status.success() && err.contains("more than once"),
            "{args:?}: {err}"
        );
    }
    // A flag is never taken as the handle or file.
    for args in [
        &["deref", "--lines", "1:2"][..],
        &["encode-ahtl", "--profile", "a"][..],
    ] {
        let out = run(&b, args, b"");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            !out.status.success() && err.contains("Usage:"),
            "{args:?}: {err}"
        );
    }
    // The positional may follow the flags.
    let file = b.join("f.txt");
    std::fs::write(&file, "fn main() {}\n".repeat(50)).unwrap();
    let out = run(
        &b,
        &[
            "encode-ahtl",
            "--dir",
            ws,
            "--profile",
            "b",
            file.to_str().unwrap(),
        ],
        b"",
    );
    assert!(out.status.success(), "{out:?}");
    let handle = String::from_utf8_lossy(&out.stdout)
        .split("#icx:")
        .nth(1)
        .map(|r| format!("#icx:{}", &r[..16]))
        .expect("handle");
    let out = run(&b, &["deref", "--dir", ws, "--lines", "1:1", &handle], b"");
    assert!(
        out.status.success() && String::from_utf8_lossy(&out.stdout).contains("fn main"),
        "{out:?}"
    );
    // An end past the last line is clamped, as MCP `slice` does (review of #38).
    let out = run(
        &b,
        &["deref", &handle, "--dir", ws, "--lines", "1:100000"],
        b"",
    );
    assert!(out.status.success(), "{out:?}");
    // A typo or a second handle is an error, not silently ignored.
    for args in [
        &["deref", &handle, "--dir", ws, "--line", "1:2"][..],
        &["deref", &handle, &handle, "--dir", ws][..],
    ] {
        let out = run(&b, args, b"");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            !out.status.success() && err.contains("unexpected argument"),
            "{args:?}: {err}"
        );
    }
    let _ = std::fs::remove_dir_all(&b);
}
