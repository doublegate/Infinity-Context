//! v1.3.2: hooks leave outputs under 2 KiB inline (a stub would save little and cost a deref
//! round trip); `ICX_STUB_MIN_BYTES` adjusts the floor.
use std::io::Write;
use std::process::{Command, Stdio};

fn hook(base: &std::path::Path, ws: &std::path::Path, min: Option<&str>, lines: usize) -> String {
    let body = (1..=lines)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let payload = serde_json::json!({
        "hook_event_name": "PostToolUse", "tool_name": "Bash",
        "tool_input": {"command": format!("seq 1 {lines}")},
        "tool_response": {"stdout": body, "stderr": "", "interrupted": false, "isImage": false}
    });
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_icx"));
    cmd.env("ICX_CONFIG_HOME", base.join("config"))
        .env("ICX_STATE_HOME", base.join("state"))
        .env("XDG_RUNTIME_DIR", base.join("runtime"))
        .env("ICX_NO_DAEMON", "1")
        .env_remove("ICX_STUB_MIN_BYTES")
        .args([
            "hook",
            "--agent",
            "claude-code",
            "--event",
            "PostToolUse",
            "--dir",
        ])
        .arg(ws);
    if let Some(m) = min {
        cmd.env("ICX_STUB_MIN_BYTES", m);
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.to_string().as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn small_outputs_stay_inline_and_the_floor_is_configurable() {
    let base = std::env::temp_dir()
        .join(format!(
            "icx-tests-{}",
            std::env::var("USER").unwrap_or_default()
        ))
        .join(format!("stub-threshold-{}", std::process::id()));
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
    // 1,492 bytes: inline by default, stubbed with a 512-byte floor.
    assert!(!hook(&base, &ws, None, 400).contains("ICX:STUB"));
    assert!(hook(&base, &ws, Some("512"), 400).contains("ICX:STUB"));
    // 3,893 bytes: stubbed by default. An out-of-range override falls back to the default.
    assert!(hook(&base, &ws, None, 1000).contains("ICX:STUB"));
    assert!(!hook(&base, &ws, Some("1"), 400).contains("ICX:STUB"));
    let _ = std::fs::remove_dir_all(&base);
}
