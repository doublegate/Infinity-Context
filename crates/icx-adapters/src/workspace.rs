//! Workspace location and trust policy (v1.1.0; ICX-AUD-010, ICX-AUD-013).
//!
//! **Location.** By default a project's workspace lives *outside* the repository, at
//! `<state_dir>/workspaces/<blake3(canonical project root)[..16]>`, where the project root is the
//! nearest ancestor containing `.git`. A repository-local `.icx/` is never selected implicitly.
//!
//! **Trust.** Every workspace directory (default, `--dir`, or `ICX_DIR`) must
//!
//! 1. be a directory owned by the effective uid with no group/other permission bits, and
//! 2. carry an `OWNER` seal: a keyed BLAKE3 MAC of its canonical path under a per-install
//!    secret (`<config_dir>/install.key`, mode `0600`).
//!
//! A directory icx creates is sealed automatically. An *existing* unsealed directory (for example
//! a `.icx/` shipped inside a cloned repository) is refused until the user runs `icx trust`. A
//! seal does not survive copying or moving the directory, nor a different install key.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

const SEAL_FILE: &str = "OWNER";
const KEY_FILE: &str = "install.key";
const SEAL_DOMAIN: &[u8] = b"ICX_WORKSPACE_OWNER_V1\0";
const LEGACY_DIR: &str = ".icx";

/// Base directories used to place workspaces, the install key, and runtime sockets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceRoots {
    pub state_dir: PathBuf,
    pub config_dir: PathBuf,
    pub runtime_dir: PathBuf,
}

impl WorkspaceRoots {
    /// Resolves roots from `ICX_STATE_HOME` / `XDG_STATE_HOME`, `ICX_CONFIG_HOME` /
    /// `XDG_CONFIG_HOME`, and `XDG_RUNTIME_DIR`, falling back to `$HOME/.local/state/icx` and
    /// `$HOME/.config/icx`. Only absolute paths are accepted. If no private location can be
    /// determined the affected root is left empty, which makes every trust operation fail closed
    /// (never a fallback to a shared directory such as `/tmp`).
    pub fn from_env() -> Self {
        let var = |k: &str| {
            std::env::var_os(k)
                .map(PathBuf::from)
                .filter(|v| v.is_absolute())
        };
        let home = var("HOME");
        let under_home = |rel: &str| home.as_ref().map(|h| h.join(rel));
        let state_dir = var("ICX_STATE_HOME")
            .or_else(|| var("XDG_STATE_HOME").map(|d| d.join("icx")))
            .or_else(|| under_home(".local/state/icx"))
            .unwrap_or_default();
        let config_dir = var("ICX_CONFIG_HOME")
            .or_else(|| var("XDG_CONFIG_HOME").map(|d| d.join("icx")))
            .or_else(|| under_home(".config/icx"))
            .unwrap_or_default();
        let runtime_dir = var("XDG_RUNTIME_DIR")
            .map(|d| d.join("icx"))
            .unwrap_or_else(|| {
                if state_dir.as_os_str().is_empty() {
                    PathBuf::new()
                } else {
                    state_dir.join("run")
                }
            });
        Self {
            state_dir,
            config_dir,
            runtime_dir,
        }
    }
}

/// How a workspace directory was selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceSource {
    /// `--dir <PATH>` on the command line.
    Explicit,
    /// `ICX_DIR` environment variable.
    Env,
    /// Derived from the project root under the state directory.
    Default,
}

/// A resolved workspace location.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedWorkspace {
    pub dir: PathBuf,
    pub source: WorkspaceSource,
    /// A repository-local `.icx/` that exists but was *not* selected (v1.0.x layout). Callers
    /// should tell the user how to adopt it with `icx trust` instead of silently ignoring it.
    pub legacy_repo_local: Option<PathBuf>,
}

/// Reason a workspace directory is not trusted.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TrustError {
    #[error("workspace path is not a directory")]
    NotDirectory,
    #[error("workspace directory is not owned by the current user")]
    NotOwned,
    #[error("workspace directory mode {0:o} grants group/other access (expected 0700)")]
    PermissiveMode(u32),
    #[error("workspace has no owner seal; run `icx trust --dir <PATH>` to adopt it")]
    MissingSeal,
    #[error("workspace owner seal is invalid (copied, moved, or from another install)")]
    BadSeal,
    #[error("workspace I/O error: {0}")]
    Io(String),
}

fn io_err(e: impl std::fmt::Display) -> TrustError {
    TrustError::Io(e.to_string())
}

/// Nearest ancestor of `cwd` containing `.git` (directory or worktree file), else `cwd`.
pub fn project_root(cwd: &Path) -> PathBuf {
    let start = std::fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    start
        .ancestors()
        .find(|p| p.join(".git").exists())
        .map_or_else(|| start.clone(), Path::to_path_buf)
}

/// Default workspace directory for the project containing `cwd`.
pub fn default_workspace_dir(cwd: &Path, roots: &WorkspaceRoots) -> PathBuf {
    let root = project_root(cwd);
    let digest = blake3::hash(root.as_os_str().as_encoded_bytes());
    roots
        .state_dir
        .join("workspaces")
        .join(&digest.to_hex()[..16])
}

/// Resolves the workspace directory: `--dir` beats `ICX_DIR` beats the per-project default.
/// Relative paths are resolved against `cwd`.
pub fn resolve(
    cli_dir: Option<&str>,
    env_dir: Option<&str>,
    cwd: &Path,
    roots: &WorkspaceRoots,
) -> Result<ResolvedWorkspace, TrustError> {
    let pick = |raw: &str| {
        let p = PathBuf::from(raw.trim());
        if p.is_absolute() { p } else { cwd.join(p) }
    };
    if let Some(d) = cli_dir.map(str::trim).filter(|s| !s.is_empty()) {
        return Ok(ResolvedWorkspace {
            dir: pick(d),
            source: WorkspaceSource::Explicit,
            legacy_repo_local: None,
        });
    }
    if let Some(d) = env_dir.map(str::trim).filter(|s| !s.is_empty()) {
        return Ok(ResolvedWorkspace {
            dir: pick(d),
            source: WorkspaceSource::Env,
            legacy_repo_local: None,
        });
    }
    let root = project_root(cwd);
    let legacy = [root.join(LEGACY_DIR), cwd.join(LEGACY_DIR)]
        .into_iter()
        .find(|p| p.is_dir());
    Ok(ResolvedWorkspace {
        dir: default_workspace_dir(cwd, roots),
        source: WorkspaceSource::Default,
        legacy_repo_local: legacy,
    })
}

fn require_absolute(dir: &Path, what: &str) -> Result<(), TrustError> {
    if dir.is_absolute() {
        Ok(())
    } else {
        Err(TrustError::Io(format!(
            "cannot determine a private {what} directory (set HOME or the XDG base directories)"
        )))
    }
}

fn private_dir(dir: &Path) -> Result<(), TrustError> {
    require_absolute(dir, "workspace")?;
    let mut b = std::fs::DirBuilder::new();
    b.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        b.mode(0o700);
    }
    b.create(dir).map_err(io_err)
}

fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), TrustError> {
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let _ = std::fs::remove_file(&tmp);
    let mut f = opts.open(&tmp).map_err(io_err)?;
    f.write_all(bytes).map_err(io_err)?;
    f.sync_all().map_err(io_err)?;
    drop(f);
    std::fs::rename(&tmp, path).map_err(io_err)
}

/// Loads the per-install key, creating it (32 bytes from the OS CSPRNG, mode `0600`) on first use.
pub fn install_key(roots: &WorkspaceRoots) -> Result<[u8; 32], TrustError> {
    require_absolute(&roots.config_dir, "config")?;
    let path = roots.config_dir.join(KEY_FILE);
    if let Ok(bytes) = std::fs::read(&path) {
        check_dir_security(&roots.config_dir)?;
        check_private_file(&path)?;
        return bytes
            .try_into()
            .map_err(|_| TrustError::Io("install.key is not 32 bytes".into()));
    }
    private_dir(&roots.config_dir)?;
    check_dir_security(&roots.config_dir)?;
    let mut key = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut key))
        .map_err(io_err)?;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    match opts.open(&path) {
        Ok(mut f) => {
            f.write_all(&key).map_err(io_err)?;
            f.sync_all().map_err(io_err)?;
            Ok(key)
        }
        // Another process won the creation race: use its key.
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            check_private_file(&path)?;
            let bytes = std::fs::read(&path).map_err(io_err)?;
            bytes
                .try_into()
                .map_err(|_| TrustError::Io("install.key is not 32 bytes".into()))
        }
        Err(e) => Err(io_err(e)),
    }
}

/// Key used to MAC every saved store. It binds a trusted workspace to icx-written content, so a
/// store replaced by `git pull` or a file copy is rejected even inside a sealed directory.
pub fn store_seal_key(roots: &WorkspaceRoots) -> Result<[u8; 32], TrustError> {
    derived_key(roots, "store-seal")
}

/// Derives a purpose-specific 32-byte key from the install key (e.g. `"provenance"`).
pub fn derived_key(roots: &WorkspaceRoots, purpose: &str) -> Result<[u8; 32], TrustError> {
    let key = install_key(roots)?;
    let mut ctx = Vec::with_capacity(32 + purpose.len());
    ctx.extend_from_slice(&key);
    ctx.extend_from_slice(purpose.as_bytes());
    Ok(blake3::derive_key("icx v1 install-derived key", &ctx))
}

#[cfg(unix)]
fn current_uid() -> u32 {
    rustix::process::geteuid().as_raw()
}

#[cfg(unix)]
fn check_private_file(path: &Path) -> Result<(), TrustError> {
    use std::os::unix::fs::MetadataExt;
    let md = std::fs::symlink_metadata(path).map_err(io_err)?;
    if md.uid() != current_uid() {
        return Err(TrustError::NotOwned);
    }
    if md.mode() & 0o077 != 0 {
        return Err(TrustError::PermissiveMode(md.mode() & 0o777));
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_private_file(_path: &Path) -> Result<(), TrustError> {
    Ok(())
}

fn check_dir_security(dir: &Path) -> Result<(), TrustError> {
    let md = std::fs::symlink_metadata(dir).map_err(io_err)?;
    if !md.is_dir() {
        return Err(TrustError::NotDirectory);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if md.uid() != current_uid() {
            return Err(TrustError::NotOwned);
        }
        if md.mode() & 0o077 != 0 {
            return Err(TrustError::PermissiveMode(md.mode() & 0o777));
        }
    }
    Ok(())
}

fn compute_seal(dir: &Path, roots: &WorkspaceRoots) -> Result<blake3::Hash, TrustError> {
    let key = install_key(roots)?;
    let canonical = std::fs::canonicalize(dir).map_err(io_err)?;
    let mut h = blake3::Hasher::new_keyed(&key);
    h.update(SEAL_DOMAIN);
    h.update(canonical.as_os_str().as_encoded_bytes());
    Ok(h.finalize())
}

fn write_seal_and_ignore(dir: &Path, roots: &WorkspaceRoots) -> Result<(), TrustError> {
    // Always (re)write: a repository could ship its own `.icx/.gitignore` with `!` exceptions.
    write_private_file(&dir.join(".gitignore"), b"*\n")?;
    let seal = compute_seal(dir, roots)?;
    write_private_file(&dir.join(SEAL_FILE), seal.to_hex().as_bytes())
}

/// Verifies ownership, permissions, and the owner seal of an existing workspace directory.
pub fn verify(dir: &Path, roots: &WorkspaceRoots) -> Result<(), TrustError> {
    check_dir_security(dir)?;
    let seal_path = dir.join(SEAL_FILE);
    let raw = match std::fs::read_to_string(&seal_path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(TrustError::MissingSeal),
        Err(e) => return Err(io_err(e)),
    };
    let stored = blake3::Hash::from_hex(raw.trim()).map_err(|_| TrustError::BadSeal)?;
    // `blake3::Hash` equality is constant-time.
    if stored == compute_seal(dir, roots)? {
        Ok(())
    } else {
        Err(TrustError::BadSeal)
    }
}

/// Ensures a usable workspace at `dir`: creates and seals it if missing, verifies it otherwise.
/// Never adopts an existing unsealed directory (see [`trust`]).
pub fn prepare(dir: &Path, roots: &WorkspaceRoots) -> Result<(), TrustError> {
    if dir.exists() {
        return verify(dir, roots);
    }
    private_dir(dir)?;
    write_seal_and_ignore(dir, roots)
}

/// Explicitly adopts an existing directory owned by the current user: tightens it to `0700`,
/// adds `.gitignore`, and writes a fresh owner seal. This is the user's opt-in for a directory
/// icx did not create (for example a v1.0.x repository-local `.icx/`).
pub fn trust(dir: &Path, roots: &WorkspaceRoots) -> Result<(), TrustError> {
    let md = std::fs::symlink_metadata(dir).map_err(io_err)?;
    if !md.is_dir() {
        return Err(TrustError::NotDirectory);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if md.uid() != current_uid() {
            return Err(TrustError::NotOwned);
        }
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).map_err(io_err)?;
        // A v1.0.x `.icx/` was created with default modes (0644 files). Tighten the regular
        // files this user owns; never follow symlinks or touch files owned by someone else.
        for entry in std::fs::read_dir(dir).map_err(io_err)?.flatten() {
            let Ok(emd) = std::fs::symlink_metadata(entry.path()) else {
                continue;
            };
            if emd.is_file() && emd.uid() == current_uid() && emd.mode() & 0o077 != 0 {
                std::fs::set_permissions(entry.path(), std::fs::Permissions::from_mode(0o600))
                    .map_err(io_err)?;
            }
        }
    }
    write_seal_and_ignore(dir, roots)
}

/// Unix-socket path for the resident daemon serving `dir`.
pub fn daemon_socket_path(dir: &Path, roots: &WorkspaceRoots) -> PathBuf {
    let canonical = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let digest = blake3::hash(canonical.as_os_str().as_encoded_bytes());
    roots
        .runtime_dir
        .join(format!("ws-{}.sock", &digest.to_hex()[..16]))
}

/// Creates the runtime directory (`0700`) that holds daemon sockets.
pub fn ensure_runtime_dir(roots: &WorkspaceRoots) -> Result<(), TrustError> {
    private_dir(&roots.runtime_dir)?;
    check_dir_security(&roots.runtime_dir)
}
