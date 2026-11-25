// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later AND AGPL-3.0-or-later

//! `icx eval-recall` (v1.5.0, T-72-002) end to end: a synthetic run, a query set over a real
//! workspace (read only), the report fields, and argument errors.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn root(tag: &str) -> PathBuf {
    let r = std::env::temp_dir().join(format!("icx-eval-cli-{tag}-{}", std::process::id()));
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
        .env_remove("ICX_RECALL_RANKING")
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

fn json(o: &Output) -> serde_json::Value {
    assert!(
        o.status.success(),
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    serde_json::from_slice(&o.stdout).unwrap()
}

#[test]
fn a_synthetic_run_reports_every_ranker_and_writes_the_file() {
    let r = root("synthetic");
    let out = r.join("report.json");
    let o = icx(
        &r,
        &[
            "eval-recall",
            "--synthetic",
            "200",
            "--vocab",
            "20000",
            "--queries",
            "20",
            "--seed",
            "3",
            "--commit",
            "abc1234",
            "--out",
            out.to_str().unwrap(),
        ],
        None,
    );
    let report = json(&o);
    assert_eq!(report["source"], "synthetic:200");
    assert_eq!(report["seed"], 3);
    assert_eq!(report["commit"], "abc1234");
    assert_eq!(report["queries"], 20);
    let rankers: Vec<&str> = report["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["ranker"].as_str().unwrap())
        .collect();
    assert_eq!(rankers, ["legacy", "bm25", "rrf", "recent"]);
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(written, report);
    let _ = std::fs::remove_dir_all(&r);
}

#[test]
fn a_query_set_is_scored_over_the_workspace_without_changing_it() {
    let r = root("set");
    // Put one artifact into the workspace through a hook.
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {"command": "cat parser.rs"},
        "tool_response": {"stdout": "pub fn parseHttpRequest(buf: &[u8]) {}\n".repeat(200)}
    })
    .to_string();
    let o = icx(
        &r,
        &["hook", "--agent", "claude-code", "--event", "PostToolUse"],
        Some(&payload),
    );
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let status = String::from_utf8_lossy(&icx(&r, &["status"], None).stdout).into_owned();
    let handle = String::from_utf8_lossy(&o.stdout)
        .split("#icx:")
        .nth(1)
        .map(|h| format!("#icx:{}", &h[..16]))
        .expect("the hook stubbed the output");
    let set = r.join("set.jsonl");
    std::fs::write(
        &set,
        format!(
            "{{\"query\":\"parse http request\",\"relevant\":[\"{handle}\"],\"split\":\"heldout\"}}\n\
             {{\"query\":\"not stored here\",\"relevant\":[\"#icx:00000000000000aa\"],\"split\":\"heldout\"}}\n\
             {{\"query\":\"train only\",\"relevant\":[\"{handle}\"],\"split\":\"train\"}}\n"
        ),
    )
    .unwrap();
    let report = json(&icx(
        &r,
        &[
            "eval-recall",
            "--set",
            set.to_str().unwrap(),
            "--ranker",
            "bm25",
        ],
        None,
    ));
    assert_eq!(report["split"], "heldout");
    assert_eq!(report["queries"], 1);
    assert_eq!(report["unanswerable_dropped"], 1);
    assert_eq!(report["results"][0]["ranker"], "bm25");
    assert_eq!(report["results"][0]["recall_at_1"], 1.0);
    let all = json(&icx(
        &r,
        &[
            "eval-recall",
            "--set",
            set.to_str().unwrap(),
            "--split",
            "all",
            "--min-age",
            "0",
        ],
        None,
    ));
    assert_eq!(all["queries"], 2);
    // Read only: the workspace looks the same afterwards.
    let after = String::from_utf8_lossy(&icx(&r, &["status"], None).stdout).into_owned();
    assert_eq!(status, after);
    let _ = std::fs::remove_dir_all(&r);
}

#[test]
fn bad_arguments_are_refused_with_a_message() {
    let r = root("errors");
    for (args, needle) in [
        (vec!["eval-recall"], "give --set"),
        (
            vec!["eval-recall", "--synthetic", "10", "--set", "x"],
            "not both",
        ),
        (vec!["eval-recall", "--synthetic", "ten"], "needs a number"),
        (
            vec!["eval-recall", "--synthetic", "10", "--ranker", "magic"],
            "unknown ranker",
        ),
        (vec!["eval-recall", "--bogus", "1"], "unknown option"),
        (
            vec![
                "eval-recall",
                "--synthetic",
                "5",
                "--seed",
                "1",
                "--seed",
                "2",
            ],
            "more than once",
        ),
        (
            vec!["eval-recall", "--set", "/nonexistent/set.jsonl"],
            "reading",
        ),
    ] {
        let o = icx(&r, &args, None);
        assert!(!o.status.success(), "{args:?} succeeded");
        let err = String::from_utf8_lossy(&o.stderr);
        assert!(err.contains(needle), "{args:?}: {err}");
    }
    // A set over a directory with no workspace.
    let set = r.join("s.jsonl");
    std::fs::write(&set, "").unwrap();
    let o = icx(
        &r,
        &[
            "eval-recall",
            "--set",
            set.to_str().unwrap(),
            "--dir",
            r.join("none").to_str().unwrap(),
        ],
        None,
    );
    assert!(String::from_utf8_lossy(&o.stderr).contains("no ready workspace"));
    let _ = std::fs::remove_dir_all(&r);
}
