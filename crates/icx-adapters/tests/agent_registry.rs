//! v1.3.2 (#34, review of #38): an agent workspace record is used only when the registry is
//! private and the record is owned by the same user and not group- or world-accessible; records
//! of exited agents are pruned when a new record is written.
#![cfg(target_os = "linux")]
use icx_adapters::workspace::{WorkspaceRoots, agent_workspace, record_agent_workspace};
use std::os::unix::fs::PermissionsExt;

#[test]
fn records_are_used_only_when_private_and_stale_ones_are_pruned() {
    let base = std::env::temp_dir()
        .join(format!(
            "icx-tests-{}",
            std::env::var("USER").unwrap_or_default()
        ))
        .join(format!("agent-registry-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let base = std::fs::canonicalize(&base).unwrap();
    let roots = WorkspaceRoots {
        state_dir: base.join("state"),
        config_dir: base.join("config"),
        runtime_dir: base.join("runtime"),
    };
    let ws = roots.state_dir.join("workspaces").join("0123456789abcdef");
    std::fs::create_dir_all(&ws).unwrap();

    // A stale record (PID 1 never matches this start time) is pruned by the next write.
    let agents = roots.runtime_dir.join("agents");
    record_agent_workspace(&ws, &roots);
    let stale = agents.join("1-1.ws");
    std::fs::write(&stale, ws.to_string_lossy().as_bytes()).unwrap();
    std::fs::set_permissions(&stale, std::fs::Permissions::from_mode(0o600)).unwrap();
    let other = roots.state_dir.join("workspaces").join("fedcba9876543210");
    std::fs::create_dir_all(&other).unwrap();
    record_agent_workspace(&other, &roots);
    assert!(!stale.exists(), "stale record kept");
    assert_eq!(
        agent_workspace(&roots),
        Some(std::fs::canonicalize(&other).unwrap())
    );

    // A group- or world-accessible record is ignored.
    let record = std::fs::read_dir(&agents)
        .unwrap()
        .flatten()
        .find(|e| e.file_name().to_string_lossy().ends_with(".ws"))
        .unwrap()
        .path();
    std::fs::set_permissions(&record, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(agent_workspace(&roots), None);
    let _ = std::fs::remove_dir_all(&base);
}
