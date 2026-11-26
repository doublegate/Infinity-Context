// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later AND AGPL-3.0-or-later

//! #117: `icx proxy` serves a request whose URL carries `/p/<id>` (from `icx proxy-url`) from that
//! project's own workspace. The packets it folds are resolvable by that project's `icx
//! mcp-serve`, not by another project's, and an unknown project is refused.
#![cfg(target_os = "linux")]
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

fn base_dir(name: &str) -> PathBuf {
    let base = std::env::temp_dir()
        .join(format!(
            "icx-tests-{}",
            std::env::var("USER").unwrap_or_default()
        ))
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    std::fs::canonicalize(&base).unwrap()
}

fn icx(base: &Path, cwd: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_icx"));
    cmd.current_dir(cwd)
        .env("ICX_CONFIG_HOME", base.join("config"))
        .env("ICX_STATE_HOME", base.join("state"))
        .env("XDG_RUNTIME_DIR", base.join("runtime"))
        .env("ICX_NO_DAEMON", "1")
        .env("ICX_MCP_FOLLOW_HOOKS", "0")
        .env_remove("ICX_DIR")
        .env_remove("ICX_UPSTREAM_URL")
        .env_remove("ICX_PROXY_BIND");
    cmd
}

fn run(mut cmd: Command) -> String {
    let out = cmd.output().unwrap();
    assert!(out.status.success(), "{out:?}");
    String::from_utf8(out.stdout).unwrap()
}

fn deref(base: &Path, cwd: &Path, handle: &str) -> String {
    let mut cmd = icx(base, cwd);
    cmd.arg("mcp-serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    let call = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {"name": "icx", "arguments": {"op": "deref", "id": handle}}
    });
    let input = format!(
        "{}\n{}\n{call}\n",
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"1"}}}"#,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#
    );
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    String::from_utf8(out.stdout).unwrap()
}

/// A one-shot-per-connection fake provider: records each request body and answers with usage.
fn fake_upstream(bodies: Arc<Mutex<Vec<String>>>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut len = 0usize;
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap_or(0) > 0 {
                if line == "\r\n" {
                    break;
                }
                if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap_or(0);
                }
                line.clear();
            }
            let mut body = vec![0u8; len];
            let _ = reader.read_exact(&mut body);
            bodies
                .lock()
                .unwrap()
                .push(String::from_utf8_lossy(&body).into_owned());
            let resp = r#"{"type":"message","content":[{"type":"text","text":"ok"}],"usage":{"input_tokens":3,"cache_read_input_tokens":100,"cache_creation_input_tokens":0}}"#;
            let mut s = stream;
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{resp}",
                resp.len()
            );
        }
    });
    port
}

fn post(port: u16, path: &str, body: &str) -> String {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(
        s,
        "POST {path} HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nanthropic-version: 2023-06-01\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    let mut out = String::new();
    let _ = s.read_to_string(&mut out);
    out
}

#[test]
fn proxy_routes_each_project_into_its_own_workspace() {
    let base = base_dir("proxy-projects");
    let a = base.join("project-a");
    let b = base.join("project-b");
    for p in [&a, &b] {
        std::fs::create_dir_all(p).unwrap();
        run({
            let mut c = icx(&base, p);
            c.arg("init");
            c
        });
    }
    let free = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = free.local_addr().unwrap().port();
    drop(free);
    let listen = format!("127.0.0.1:{port}");
    let url_a = run({
        let mut c = icx(&base, &a);
        c.args(["proxy-url", "--listen", &listen]);
        c
    });
    let url_a = url_a.trim().to_owned();
    let path_a = url_a
        .strip_prefix(&format!("http://{listen}"))
        .unwrap()
        .to_owned();
    assert!(path_a.contains("/p/"), "{url_a}");

    let bodies = Arc::new(Mutex::new(Vec::new()));
    let up = fake_upstream(Arc::clone(&bodies));
    let mut proxy = icx(&base, &base)
        .args(["proxy", "--listen", &listen, "--max-requests", "7"])
        .args(["--upstream", &format!("http://127.0.0.1:{up}")])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Wait for the "listening" line rather than probing the port: a probe connection would
    // count against --max-requests.
    let mut err = BufReader::new(proxy.stderr.take().unwrap());
    let mut first = String::new();
    err.read_line(&mut first).unwrap();
    assert!(first.contains("listening"), "{first}");
    let err_log = Arc::new(Mutex::new(String::new()));
    let log = Arc::clone(&err_log);
    std::thread::spawn(move || {
        let mut rest = String::new();
        let _ = err.read_to_string(&mut rest);
        *log.lock().unwrap() = rest;
    });

    let mut big = String::new();
    for k in 0..200 {
        big.push_str(&format!("row {k} marker-qqzz value {}\n", k * 7));
    }
    let body = serde_json::json!({
        "model": "m", "max_tokens": 8,
        "messages": [
            {"role": "user", "content": "run it"},
            {"role": "assistant", "content": [{"type": "tool_use", "id": "t1", "name": "Bash", "input": {"command": "ls"}}]},
            {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "t1", "content": big}]},
            {"role": "assistant", "content": "done"},
            {"role": "user", "content": "next"},
            {"role": "assistant", "content": "ok"},
            {"role": "user", "content": "again"}
        ]
    })
    .to_string();
    let resp = post(port, &format!("{path_a}/v1/messages"), &body);
    assert!(
        resp.starts_with("HTTP/1.1 200"),
        "{resp} | proxy stderr: {}",
        err_log.lock().unwrap()
    );

    let token_part = path_a.split("/p/").next().unwrap().to_owned();
    // An unrouted request between two projects' requests (the proxy's own workspace is not
    // initialized here, so it runs without one), then project B, then project A again.
    let unrouted = post(port, &format!("{token_part}/v1/messages"), &body);
    assert!(unrouted.starts_with("HTTP/1.1 200"), "{unrouted}");
    let url_b = run({
        let mut c = icx(&base, &b);
        c.args(["proxy-url", "--listen", &listen]);
        c
    });
    let path_b = url_b
        .trim()
        .strip_prefix(&format!("http://{listen}"))
        .unwrap()
        .to_owned();
    let body_b = body.replace("marker-qqzz", "other-rrss");
    let resp_b = post(port, &format!("{path_b}/v1/messages"), &body_b);
    assert!(resp_b.starts_with("HTTP/1.1 200"), "{resp_b}");
    let again = post(port, &format!("{path_a}/v1/messages"), &body);
    assert!(again.starts_with("HTTP/1.1 200"), "{again}");
    // An unknown project and a malformed segment are refused, not served from a shared store.
    for bad in ["/p/ffffffffffffffff", "/p/"] {
        let refused = post(port, &format!("{token_part}{bad}/v1/messages"), &body);
        assert!(refused.starts_with("HTTP/1.1 403"), "{bad}: {refused}");
    }
    // A project whose workspace fails the trust check is refused too.
    let id_b = path_b.rsplit("/p/").next().unwrap().to_owned();
    let ws_b = base.join("state").join("workspaces").join(&id_b);
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&ws_b, std::fs::Permissions::from_mode(0o777)).unwrap();
    }
    let untrusted = post(port, &format!("{path_b}/v1/messages"), &body_b);
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&ws_b, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    assert!(untrusted.starts_with("HTTP/1.1 403"), "{untrusted}");
    let _ = proxy.wait();

    let forwarded_all = bodies.lock().unwrap().clone();
    assert_eq!(
        forwarded_all.len(),
        4,
        "A, unrouted, B, A reached the provider"
    );
    // Project A's folded history is byte-identical on its second request, with an unrouted
    // request and project B in between; B's request carries none of A's content.
    assert_eq!(forwarded_all[0], forwarded_all[3]);
    assert!(
        !forwarded_all[2].contains("marker-qqzz"),
        "{}",
        forwarded_all[2]
    );
    let forwarded = forwarded_all[0].clone();
    assert!(forwarded.contains("src:proxy"), "{forwarded}");
    let at = forwarded.find("#icx:").expect("a folded handle");
    let handle = &forwarded[at..at + 5 + 16];

    // Project A's MCP server resolves it; project B's cannot, and is told not to retry.
    let got_a = deref(&base, &a, handle);
    assert!(got_a.contains("marker-qqzz"), "{got_a}");
    let got_b = deref(&base, &b, handle);
    assert!(
        got_b.contains("not found") && got_b.contains("do not retry"),
        "{got_b}"
    );

    // The cache ledger is in project A's workspace.
    let ws_a = PathBuf::from(&base)
        .join("state")
        .join("workspaces")
        .join(path_a.rsplit("/p/").next().unwrap());
    let ledger = std::fs::read_to_string(ws_a.join("proxy-cache-usage.jsonl")).unwrap();
    assert!(ledger.contains("\"cache_read_tokens\":100"), "{ledger}");
    assert_eq!(ledger.lines().count(), 2, "A's two requests only: {ledger}");
    let ledger_b = std::fs::read_to_string(ws_b.join("proxy-cache-usage.jsonl")).unwrap();
    assert_eq!(ledger_b.lines().count(), 1, "{ledger_b}");
    let _ = std::fs::remove_dir_all(&base);
}
