//! v1.1.0 workspace location and trust (ICX-AUD-010, ICX-AUD-013).

use icx_adapters::workspace::{self, TrustError, WorkspaceRoots, WorkspaceSource};
use std::path::{Path, PathBuf};

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "icx_trust_{tag}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn roots(base: &Path) -> WorkspaceRoots {
    WorkspaceRoots {
        state_dir: base.join("state"),
        config_dir: base.join("config"),
        runtime_dir: base.join("run"),
    }
}

#[test]
fn default_workspace_lives_outside_the_repository_and_is_stable_across_subdirs() {
    let base = scratch("default");
    let repo = base.join("repo");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    std::fs::create_dir_all(repo.join("src/deep")).unwrap();
    let r = roots(&base);
    let a = workspace::resolve(None, None, &repo, &r).unwrap();
    let b = workspace::resolve(None, None, &repo.join("src/deep"), &r).unwrap();
    assert_eq!(a.dir, b.dir);
    assert_eq!(a.source, WorkspaceSource::Default);
    assert!(a.dir.starts_with(&r.state_dir));
    assert!(!a.dir.starts_with(&repo));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn repo_local_icx_is_reported_but_never_selected_by_default() {
    let base = scratch("legacy");
    let repo = base.join("repo");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    std::fs::create_dir_all(repo.join(".icx")).unwrap();
    let ws = workspace::resolve(None, None, &repo, &roots(&base)).unwrap();
    assert_eq!(
        ws.legacy_repo_local.as_deref(),
        Some(repo.join(".icx").as_path())
    );
    assert_ne!(ws.dir, repo.join(".icx"));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn new_workspace_is_created_private_sealed_and_ignored_by_git() {
    let base = scratch("create");
    let r = roots(&base);
    let dir = base.join("explicit_ws");
    workspace::prepare(&dir, &r).expect("create + seal");
    workspace::verify(&dir, &r).expect("own seal verifies");
    assert!(dir.join(".gitignore").is_file());
    assert_eq!(
        std::fs::read_to_string(dir.join(".gitignore"))
            .unwrap()
            .trim(),
        "*"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
        let key_mode = std::fs::metadata(r.config_dir.join("install.key"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(key_mode, 0o600);
    }
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn existing_unsealed_workspace_is_refused_until_explicitly_trusted() {
    let base = scratch("unsealed");
    let r = roots(&base);
    let cloned = base.join("repo/.icx");
    std::fs::create_dir_all(&cloned).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&cloned, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    std::fs::write(cloned.join("icx.sqlite3"), b"attacker store").unwrap();
    assert_eq!(workspace::verify(&cloned, &r), Err(TrustError::MissingSeal));
    assert!(
        workspace::prepare(&cloned, &r).is_err(),
        "prepare never adopts an existing dir"
    );
    workspace::trust(&cloned, &r).expect("explicit opt-in");
    workspace::verify(&cloned, &r).expect("trusted after opt-in");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn seal_does_not_transfer_to_a_copied_or_moved_workspace() {
    let base = scratch("copied");
    let r = roots(&base);
    let original = base.join("a/.icx");
    workspace::prepare(&original, &r).unwrap();
    let copy = base.join("b/.icx");
    std::fs::create_dir_all(copy.parent().unwrap()).unwrap();
    std::fs::rename(&original, &copy).unwrap();
    assert_eq!(workspace::verify(&copy, &r), Err(TrustError::BadSeal));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn seal_from_another_install_key_is_rejected() {
    let base = scratch("otherkey");
    let dir = base.join("ws");
    workspace::prepare(&dir, &roots(&base.join("installA"))).unwrap();
    assert_eq!(
        workspace::verify(&dir, &roots(&base.join("installB"))),
        Err(TrustError::BadSeal)
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[cfg(unix)]
#[test]
fn group_or_world_accessible_workspace_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let base = scratch("perms");
    let r = roots(&base);
    let dir = base.join("ws");
    workspace::prepare(&dir, &r).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        workspace::verify(&dir, &r),
        Err(TrustError::PermissiveMode(0o755))
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn explicit_and_env_directories_take_precedence_over_default() {
    let base = scratch("explicit");
    let r = roots(&base);
    let cwd = base.join("repo");
    std::fs::create_dir_all(&cwd).unwrap();
    let e = workspace::resolve(Some("custom"), Some("/ignored/env"), &cwd, &r).unwrap();
    assert_eq!(e.source, WorkspaceSource::Explicit);
    assert_eq!(e.dir, cwd.join("custom"));
    let v = workspace::resolve(None, Some("envdir"), &cwd, &r).unwrap();
    assert_eq!(v.source, WorkspaceSource::Env);
    assert_eq!(v.dir, cwd.join("envdir"));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn unresolvable_roots_fail_closed_instead_of_using_a_shared_directory() {
    let empty = WorkspaceRoots {
        state_dir: PathBuf::new(),
        config_dir: PathBuf::new(),
        runtime_dir: PathBuf::new(),
    };
    let base = scratch("noroots");
    assert!(workspace::prepare(&base.join("ws"), &empty).is_err());
    assert!(workspace::install_key(&empty).is_err());
    let _ = std::fs::remove_dir_all(&base);
}

#[cfg(unix)]
#[test]
fn permissive_install_key_is_rejected() {
    use std::os::unix::fs::PermissionsExt;
    let base = scratch("keyperm");
    let r = roots(&base);
    workspace::install_key(&r).unwrap();
    let key = r.config_dir.join("install.key");
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        workspace::install_key(&r),
        Err(TrustError::PermissiveMode(0o644))
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn store_replaced_through_version_control_is_rejected_even_in_a_trusted_dir() {
    // Security review follow-up: the OWNER seal binds the path only, so the store itself also
    // carries a keyed MAC over (path, generation) written on every save.
    use icx_adapters::mmu::InfinityContextMmu;
    use icx_core::AgentKind;
    let base = scratch("storeswap");
    let r = roots(&base);
    let ws = base.join("repo/.icx");
    workspace::prepare(&ws, &r).unwrap();
    let key = workspace::store_seal_key(&r).unwrap();

    let mut mine = InfinityContextMmu::default();
    mine.set_store_seal_key(Some(key));
    mine.ingest_payload(AgentKind::ClaudeCode, "Bash:ls", "mine\n", false);
    mine.save_workspace_dir(&ws).unwrap();
    let mut reload = InfinityContextMmu::default();
    reload.set_store_seal_key(Some(key));
    reload.load_workspace_dir(&ws).expect("own store verifies");

    // Simulate `git pull` replacing the store with one produced elsewhere (no install key).
    let other = base.join("attacker_ws");
    let mut attacker = InfinityContextMmu::default();
    attacker.ingest_payload(AgentKind::ClaudeCode, "Bash:INJECT", "evil\n", false);
    attacker.save_workspace_dir(&other).unwrap();
    for side in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(ws.join(format!("icx.sqlite3{side}")));
    }
    std::fs::copy(other.join("icx.sqlite3"), ws.join("icx.sqlite3")).unwrap();
    workspace::verify(&ws, &r).expect("path seal alone still passes");
    let mut victim = InfinityContextMmu::default();
    victim.set_store_seal_key(Some(key));
    assert!(
        victim.load_workspace_dir(&ws).is_err(),
        "swapped store must be refused"
    );
    let _ = std::fs::remove_dir_all(&base);
}
