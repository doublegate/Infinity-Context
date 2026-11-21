//! Acknowledged daemon artifacts survive abrupt process termination (ICX-CODEX-030).
#![cfg(unix)]

use icx_adapters::workspace::{WorkspaceRoots, daemon_socket_path};
use icx_core::ArtifactId;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

struct TestDaemon(Child);

impl Drop for TestDaemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn command(base: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_icx"));
    command
        .env("ICX_STATE_HOME", base.join("state"))
        .env("ICX_CONFIG_HOME", base.join("config"))
        .env("XDG_RUNTIME_DIR", base.join("run"));
    command
}

#[test]
fn every_successful_ack_survives_sigkill_and_cli_reopen() {
    let base = std::env::temp_dir()
        .join(format!(
            "icx-v120-tests-{}",
            std::env::var("USER").unwrap_or_default()
        ))
        .join(format!("kill-{}", std::process::id()));
    std::fs::create_dir_all(&base).unwrap();
    let ws = base.join("workspace");
    let init = command(&base)
        .arg("init")
        .arg("--dir")
        .arg(&ws)
        .output()
        .unwrap();
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    let roots = WorkspaceRoots {
        state_dir: base.join("state"),
        config_dir: base.join("config"),
        runtime_dir: base.join("run/icx"),
    };
    let socket = daemon_socket_path(&ws, &roots);
    let mut daemon = TestDaemon(
        command(&base)
            .arg("daemon")
            .arg("--dir")
            .arg(&ws)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    for _ in 0..400 {
        if socket.exists() {
            break;
        }
        assert!(
            daemon.0.try_wait().unwrap().is_none(),
            "daemon exited before binding"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut acknowledged = Vec::new();
    for n in 0..3 {
        let text = format!("durable-process-test fixture {n}\n");
        let request = json!({"v":1, "agent":"codex", "event":"PostToolUse",
            "payload":json!({"tool_name":"Bash", "tool_response":text}).to_string()});
        let mut stream = UnixStream::connect(&socket).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        writeln!(stream, "{request}").unwrap();
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).unwrap();
        let response: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(response["ok"], true, "{response}");
        acknowledged.push((ArtifactId::hash_bytes(text.as_bytes()).0, text));
    }
    daemon.0.kill().unwrap();
    daemon.0.wait().unwrap();
    for (aid, text) in acknowledged {
        let result = command(&base)
            .arg("deref")
            .arg(aid.to_string())
            .arg("--dir")
            .arg(&ws)
            .arg("--raw")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, text.as_bytes());
    }
    std::fs::remove_dir_all(base).unwrap();
}
