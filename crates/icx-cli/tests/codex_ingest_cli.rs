//! `icx codex-ingest` end to end (T-62-003, T-62-004): plain and real zstd-compressed rollouts,
//! the project filter, bookmarks across runs, and refusal of a corrupt file.

use std::path::Path;
use std::process::{Command, Output};

/// A synthetic rollout (session cwd `/icx-fixture/project`, 40 tool outputs) compressed with
/// `zstd -3`, as Codex does, with compressed blocks and an XXH64 checksum.
const FIXTURE_ZST: &str = "tests/fixtures/codex/rollout-2026-11-22T00-00-00-synthetic.jsonl.zst";

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_icx"))
        .args(args)
        .env("ICX_STATE_HOME", root.join("state"))
        .env("ICX_CONFIG_HOME", root.join("config"))
        .env("ICX_DIR", root.join("ws"))
        .env("XDG_STATE_HOME", root.join("xdg-state"))
        .env("XDG_RUNTIME_DIR", root.join("run"))
        .env("CODEX_HOME", root.join("codex"))
        .env("ICX_NO_DAEMON", "1")
        .output()
        .expect("run icx")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn line(kind: &str, payload: &serde_json::Value) -> String {
    let mut s =
        serde_json::json!({"timestamp": "2026-11-22T00:00:00Z", "type": kind, "payload": payload})
            .to_string();
    s.push('\n');
    s
}

#[test]
fn codex_ingest_reads_plain_and_compressed_rollouts_once() {
    let root = std::env::temp_dir().join(format!("icx-codex-ingest-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let day = root.join("codex/sessions/2026/11/22");
    std::fs::create_dir_all(&day).unwrap();

    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_ZST),
        day.join("rollout-2026-11-22T00-00-00-synthetic.jsonl.zst"),
    )
    .unwrap();
    let mut plain = line(
        "session_meta",
        &serde_json::json!({"id": "t2", "cwd": "/icx-fixture/project/sub"}),
    );
    plain.push_str(&line(
        "response_item",
        &serde_json::json!({"type": "function_call_output", "call_id": "p1", "output": "plain output"}),
    ));
    std::fs::write(day.join("rollout-2026-11-22T01-00-00-plain.jsonl"), &plain).unwrap();
    let mut other = line(
        "session_meta",
        &serde_json::json!({"id": "t3", "cwd": "/elsewhere"}),
    );
    other.push_str(&line(
        "response_item",
        &serde_json::json!({"type": "function_call_output", "call_id": "o1", "output": "not ours"}),
    ));
    std::fs::write(day.join("rollout-2026-11-22T02-00-00-other.jsonl"), &other).unwrap();

    let first = run(
        &root,
        &["codex-ingest", "--project", "/icx-fixture/project"],
    );
    assert!(first.status.success(), "stderr: {}", stderr(&first));
    assert!(
        stdout(&first)
            .contains("2 rollout(s) matched, 2 with new records, 41 record(s) ingested, 0 skipped"),
        "stdout: {}",
        stdout(&first)
    );
    assert!(
        root.join("ws/codex-rollouts.json").is_file(),
        "bookmarks are saved"
    );

    let second = run(
        &root,
        &["codex-ingest", "--project", "/icx-fixture/project"],
    );
    assert!(second.status.success(), "stderr: {}", stderr(&second));
    assert!(
        stdout(&second).contains("0 record(s) ingested"),
        "a second run ingests nothing: {}",
        stdout(&second)
    );

    // A corrupt compressed rollout is skipped with a warning; the run still succeeds.
    std::fs::write(
        day.join("rollout-2026-11-22T03-00-00-bad.jsonl.zst"),
        b"\x28\xb5\x2f\xfdjunk",
    )
    .unwrap();
    let all = run(&root, &["codex-ingest", "--all-projects"]);
    assert!(all.status.success(), "stderr: {}", stderr(&all));
    assert!(
        stdout(&all)
            .contains("4 rollout(s) matched, 1 with new records, 1 record(s) ingested, 1 skipped"),
        "--all-projects adds only the other project's record and skips the corrupt file: {}",
        stdout(&all)
    );
    assert!(
        stderr(&all).contains("rollout-2026-11-22T03-00-00-bad.jsonl.zst"),
        "the skipped file is named: {}",
        stderr(&all)
    );
    // With a project filter the corrupt file has no readable session cwd, so it matches nothing.
    let filtered = run(
        &root,
        &["codex-ingest", "--project", "/icx-fixture/project"],
    );
    assert!(
        stdout(&filtered).contains("0 skipped"),
        "stdout: {}",
        stdout(&filtered)
    );

    let conflict = run(
        &root,
        &["codex-ingest", "--all-projects", "--project", "/x"],
    );
    assert!(!conflict.status.success());
    assert!(stderr(&conflict).contains("conflict"));
    let _ = std::fs::remove_dir_all(&root);
}
