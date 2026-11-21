use std::process::Command;

#[test]
fn cli_can_set_and_inspect_a_persisted_model_budget() {
    let base = std::path::PathBuf::from("/tmp/Infinity-Context/v120")
        .join(format!("cli-budget-{}", std::process::id()));
    std::fs::create_dir_all(&base).unwrap();
    let ws = base.join("workspace");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_icx"))
            .env("ICX_CONFIG_HOME", base.join("config"))
            .env("ICX_STATE_HOME", base.join("state"))
            .env("XDG_RUNTIME_DIR", base.join("runtime"))
            .args(args)
            .arg("--dir")
            .arg(&ws)
            .output()
            .unwrap()
    };
    assert!(run(&["init"]).status.success());
    let changed = run(&[
        "context-budget",
        "--context-tokens",
        "8192",
        "--completion-reserve",
        "1024",
        "--external-reserve",
        "2048",
    ]);
    assert!(
        changed.status.success(),
        "{}",
        String::from_utf8_lossy(&changed.stderr)
    );
    let shown = run(&["context-budget"]);
    let policy: serde_json::Value = serde_json::from_slice(&shown.stdout).unwrap();
    assert_eq!(policy["policy"]["context_tokens"], 8192);
    assert_eq!(policy["window_bytes"], 5120);
    assert_eq!(policy["estimator"], "utf8-bytes-upper-bound-v1");
    assert!(
        !run(&["context-budget", "--context-tokens", "10"])
            .status
            .success()
    );
    assert!(
        !run(&["context-budget", "--context-tokens"])
            .status
            .success()
    );
    let unchanged: serde_json::Value =
        serde_json::from_slice(&run(&["context-budget"]).stdout).unwrap();
    assert_eq!(policy, unchanged);
    std::fs::remove_dir_all(base).unwrap();
}
