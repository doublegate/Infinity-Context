//! Integration test exercising the compiled `icx` binary `main()` entrypoint.

use std::process::Command;

#[test]
fn icx_binary_main_entrypoint_handles_success_and_error_exit_codes() {
    let bin = env!("CARGO_BIN_EXE_icx");

    let ok_out = Command::new(bin)
        .arg("status")
        .output()
        .expect("execute icx status");
    assert!(ok_out.status.success());
    assert!(String::from_utf8_lossy(&ok_out.stdout).contains("⟪ICX:STATUS"));

    let err_out = Command::new(bin)
        .arg("deref")
        .output()
        .expect("execute icx deref without handle");
    assert!(!err_out.status.success());
    assert!(String::from_utf8_lossy(&err_out.stderr).contains("Usage: icx deref"));
}
