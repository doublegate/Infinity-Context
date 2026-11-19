//! v1.1.0 file-open hardening (ICX-AUD-018, ICX-AUD-019).
#![cfg(unix)]

use icx_adapters::AntigravityAdapter;
use icx_bus::SeqlockTelemetryBus;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("icx_harden_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn transcript_path_pointing_at_a_fifo_returns_immediately() {
    let dir = scratch("fifo");
    let fifo = dir.join("transcript.fifo");
    let status = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap();
    assert!(status.success());
    let (tx, rx) = std::sync::mpsc::channel();
    let p = fifo.clone();
    std::thread::spawn(move || {
        let t = Instant::now();
        let out = AntigravityAdapter::read_transcript_step(&p, None);
        let _ = tx.send((out, t.elapsed()));
    });
    let (out, elapsed) = rx
        .recv_timeout(Duration::from_secs(3))
        .expect("read_transcript_step blocked on a FIFO");
    assert!(out.is_none());
    assert!(elapsed < Duration::from_secs(1));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn transcript_symlink_is_not_followed() {
    let dir = scratch("symlink");
    let real = dir.join("real.jsonl");
    std::fs::write(&real, b"{}\n").unwrap();
    let link = dir.join("link.jsonl");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    assert!(AntigravityAdapter::read_transcript_step(&link, None).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bus_file_refuses_symlinks_and_permissive_preexisting_files() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("bus");
    let target = dir.join("victim");
    std::fs::write(&target, b"").unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600)).unwrap();
    let link = dir.join("bus.shm");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    assert!(
        SeqlockTelemetryBus::open_shared_file(&link).is_err(),
        "followed a symlink"
    );

    let loose = dir.join("loose.shm");
    std::fs::write(&loose, b"").unwrap();
    std::fs::set_permissions(&loose, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(
        SeqlockTelemetryBus::open_shared_file(&loose).is_err(),
        "accepted 0644 file"
    );

    let fresh = dir.join("fresh.shm");
    assert!(SeqlockTelemetryBus::open_shared_file(&fresh).is_ok());
    let mode = std::fs::metadata(&fresh).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    let _ = std::fs::remove_dir_all(&dir);
}
