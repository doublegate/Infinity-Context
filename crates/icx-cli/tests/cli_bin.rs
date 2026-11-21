//! Integration test exercising the compiled `icx` binary `main()` entrypoint.

use std::process::Command;

#[test]
fn icx_binary_main_entrypoint_handles_success_and_error_exit_codes() {
    let bin = env!("CARGO_BIN_EXE_icx");
    // Never resolve the per-project default workspace: that is the live state used by the
    // installed hooks. Point every root at a fresh per-process temp directory instead.
    let root = std::env::temp_dir().join(format!("icx-cli-bin-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create isolated test root");
    let isolate = |cmd: &mut Command| {
        cmd.env("ICX_STATE_HOME", root.join("state"))
            .env("ICX_CONFIG_HOME", root.join("config"))
            .env("ICX_DIR", root.join("ws"))
            .env("XDG_STATE_HOME", root.join("xdg-state"))
            .env("XDG_RUNTIME_DIR", root.join("run"));
    };

    let mut ok_cmd = Command::new(bin);
    isolate(&mut ok_cmd);
    let ok_out = ok_cmd.arg("status").output().expect("execute icx status");
    assert!(ok_out.status.success());
    assert!(String::from_utf8_lossy(&ok_out.stdout).contains("⟪ICX:STATUS"));

    let mut err_cmd = Command::new(bin);
    isolate(&mut err_cmd);
    let err_out = err_cmd
        .arg("deref")
        .output()
        .expect("execute icx deref without handle");
    assert!(!err_out.status.success());
    assert!(String::from_utf8_lossy(&err_out.stderr).contains("Usage: icx deref"));
    let _ = std::fs::remove_dir_all(&root);
}
