//! `icx context-budget` rejects a repeated option instead of silently keeping the last value
//! (v1.2.1, PR #22 review).

use std::process::Command;

#[test]
fn context_budget_rejects_repeated_options() {
    let base = std::env::temp_dir()
        .join(format!(
            "icx-v121-tests-{}",
            std::env::var("USER").unwrap_or_default()
        ))
        .join(format!("cli-budget-dup-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let ws = base.join("workspace");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_icx"))
            .env("ICX_CONFIG_HOME", base.join("config"))
            .env("ICX_STATE_HOME", base.join("state"))
            .env("XDG_CONFIG_HOME", base.join("xdg-config"))
            .env("XDG_STATE_HOME", base.join("xdg-state"))
            .env("XDG_RUNTIME_DIR", base.join("runtime"))
            .args(args)
            .arg("--dir")
            .arg(&ws)
            .output()
            .unwrap()
    };
    assert!(run(&["init"]).status.success());
    let before = run(&["context-budget"]);
    assert!(before.status.success());

    let ws_str = ws.to_str().unwrap();
    for args in [
        &[
            "context-budget",
            "--context-tokens",
            "262144",
            "--context-tokens",
            "131072",
        ][..],
        &[
            "context-budget",
            "--completion-reserve",
            "1",
            "--completion-reserve",
            "2",
        ][..],
        &[
            "context-budget",
            "--external-reserve",
            "1",
            "--external-reserve",
            "2",
        ][..],
        // `run` appends a second `--dir`.
        &["context-budget", "--dir", ws_str][..],
    ] {
        let out = run(args);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success(), "{args:?} was accepted");
        assert!(
            stderr.contains("more than once"),
            "{args:?}: unclear error: {stderr}"
        );
    }

    let after = run(&["context-budget"]);
    assert_eq!(
        before.stdout, after.stdout,
        "a rejected call changed the budget"
    );
    std::fs::remove_dir_all(base).unwrap();
}
