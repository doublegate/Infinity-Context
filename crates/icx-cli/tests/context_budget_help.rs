//! `icx context-budget --help` prints usage instead of a missing-value error (PR #22 review).

use std::process::Command;

#[test]
fn context_budget_help_prints_usage() {
    let state =
        std::env::temp_dir().join(format!("icx-context-budget-help-{}", std::process::id()));
    for flag in ["--help", "-h"] {
        let output = Command::new(env!("CARGO_BIN_EXE_icx"))
            .args(["context-budget", flag])
            .env("ICX_STATE_HOME", &state)
            .env("ICX_CONFIG_HOME", &state)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(output.status.success(), "{flag}: {output:?}");
        assert!(
            stdout.contains("context-budget") && stdout.contains("--context-tokens"),
            "{stdout}"
        );
    }
    let _ = std::fs::remove_dir_all(state);
}
