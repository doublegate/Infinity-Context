//! v1.3.0: `icx hook --agent muse` routes to the Muse adapter end to end (no Claude-style
//! output replacement, which Muse rejects), and fails open with `{}`.
use std::io::Write;
use std::process::{Command, Stdio};

fn run(base: &std::path::Path, ws: &std::path::Path, event: &str, stdin: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_icx"))
        .env("ICX_CONFIG_HOME", base.join("config"))
        .env("ICX_STATE_HOME", base.join("state"))
        .env("XDG_CONFIG_HOME", base.join("xdg-config"))
        .env("XDG_STATE_HOME", base.join("xdg-state"))
        .env("XDG_RUNTIME_DIR", base.join("runtime"))
        .env("ICX_NO_DAEMON", "1")
        .args(["hook", "--agent", "muse", "--event", event, "--dir"])
        .arg(ws)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn muse_hooks_route_to_the_capture_only_adapter() {
    let base = std::env::temp_dir()
        .join(format!(
            "icx-v130-tests-{}",
            std::env::var("USER").unwrap_or_default()
        ))
        .join(format!("muse-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let ws = base.join("ws");
    let init = Command::new(env!("CARGO_BIN_EXE_icx"))
        .env("ICX_CONFIG_HOME", base.join("config"))
        .env("ICX_STATE_HOME", base.join("state"))
        .args(["init", "--dir"])
        .arg(&ws)
        .output()
        .unwrap();
    assert!(init.status.success(), "{init:?}");

    let start = run(
        &base,
        &ws,
        "SessionStart",
        r#"{"hook_event_name":"SessionStart","session_id":"s"}"#,
    );
    assert!(
        start.contains(r#""hookEventName":"SessionStart""#) && start.contains("additionalContext"),
        "{start}"
    );

    let big = (1..=400)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join("\\n");
    let post = format!(
        r#"{{"hook_event_name":"PostToolUse","session_id":"s","tool_name":"bash","tool_input":{{"command":"seq 1 400"}},"tool_response":"{{\"output\":\"{big}\",\"exit_code\":0}}"}}"#
    );
    let reply = run(&base, &ws, "PostToolUse", &post);
    assert!(
        !reply.contains("updatedToolOutput") && !reply.contains("updatedMCPToolOutput"),
        "{reply}"
    );
    assert!(
        reply.contains(r#""hookEventName":"PostToolUse""#),
        "{reply}"
    );

    // Malformed stdin fails open with Muse's empty reply.
    assert_eq!(run(&base, &ws, "PostToolUse", "{not json").trim(), "{}");
    let _ = std::fs::remove_dir_all(&base);
}
