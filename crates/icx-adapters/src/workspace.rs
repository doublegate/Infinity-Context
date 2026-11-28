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

/// Nearest ancestor of `cwd` that is the top of a git work tree, else `cwd`. A linked git
/// worktree (`.git` is a file naming `<main>/.git/worktrees/<name>`) resolves to its main
/// repository, so an agent working in a worktree shares the project's workspace (v1.3.2, #37).
///
/// The search follows git's own discovery (#120), so one stray `.git` in a shared parent such
/// as `/tmp` cannot merge every project below it: a `.git` directory git would reject (empty, or
/// missing `HEAD`, `objects` or `refs`) is skipped and the search goes on upward, and a `.git`
/// file naming no repository ends it, as git stops there with "not a git repository", leaving
/// `cwd` its own project.
pub fn project_root(cwd: &Path) -> PathBuf {
    let start = std::fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    for dir in start.ancestors() {
        match dot_git(dir) {
            DotGit::Repository => {
                return worktree_main_root(dir).unwrap_or_else(|| dir.to_path_buf());
            }
            DotGit::InvalidFile => break,
            DotGit::None => {}
        }
    }
    start
}

/// Whether `dir/.git` is a repository git would accept: a git directory, or a file whose
/// `gitdir:` line names one (a linked worktree or a submodule).
pub fn has_git_repository(dir: &Path) -> bool {
    matches!(dot_git(dir), DotGit::Repository)
}

/// What `dir/.git` is to git's repository discovery.
enum DotGit {
    /// A repository: discovery stops here.
    Repository,
    /// Absent, or a directory git rejects: discovery goes on to the parent.
    None,
    /// A file that names no repository: git reports an error and stops.
    InvalidFile,
}

/// The largest git control file (`.git` file, `HEAD`, `commondir`, `gitdir`) icx reads.
const MAX_CONTROL_FILE: u64 = 4096;

/// A small git control file, or `None`. It must be a regular file, not a symlink (ICX-AUD-018),
/// of at most [`MAX_CONTROL_FILE`] bytes. The open is non-blocking and no-follow and the handle is
/// re-checked, so a FIFO or device planted in any ancestor of the cwd cannot block a hook and a
/// large file is never read whole (`project_root` runs on every hook).
fn read_control_file(path: &Path) -> Option<String> {
    if !std::fs::symlink_metadata(path).ok()?.file_type().is_file() {
        return None;
    }
    let mut opts = std::fs::OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let flags = rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::NOFOLLOW;
        opts.custom_flags(flags.bits().cast_signed());
    }
    let file = opts.open(path).ok()?;
    let meta = file.metadata().ok()?;
    if !meta.file_type().is_file() || meta.len() > MAX_CONTROL_FILE {
        return None;
    }
    let mut text = String::new();
    file.take(MAX_CONTROL_FILE).read_to_string(&mut text).ok()?;
    Some(text)
}

fn dot_git(dir: &Path) -> DotGit {
    let dot_git = dir.join(".git");
    // A `.git` directory may be a symlink (git follows it); a `.git` file may not.
    let Ok(meta) = std::fs::metadata(&dot_git) else {
        return DotGit::None;
    };
    if meta.is_dir() {
        return if is_git_directory(&dot_git) {
            DotGit::Repository
        } else {
            DotGit::None
        };
    }
    if !meta.is_file() {
        return DotGit::None;
    }
    let names_repository = read_control_file(&dot_git).is_some_and(|text| {
        text.lines()
            .next()
            .and_then(|l| l.strip_prefix("gitdir:"))
            .map(str::trim)
            .is_some_and(|g| !g.is_empty() && is_git_directory(&dir.join(g)))
    });
    if names_repository {
        DotGit::Repository
    } else {
        DotGit::InvalidFile
    }
}

/// git's `is_git_directory`: a valid `HEAD`, plus `objects` and `refs` directories in the common
/// directory (`commondir` names it for a linked worktree's git directory).
fn is_git_directory(gitdir: &Path) -> bool {
    let head_ok = read_control_file(&gitdir.join("HEAD")).is_some_and(|h| {
        let h = h.trim_end();
        h.starts_with("ref: refs/")
            || ((h.len() == 40 || h.len() == 64) && h.bytes().all(|b| b.is_ascii_hexdigit()))
    });
    if !head_ok {
        return false;
    }
    let common = match read_control_file(&gitdir.join("commondir")) {
        Some(rel) if !rel.trim().is_empty() => gitdir.join(rel.trim()),
        _ => gitdir.to_path_buf(),
    };
    common.join("objects").is_dir() && common.join("refs").is_dir()
}

/// The main repository of the linked worktree at `root`, or `None` when `root` is not one.
///
/// Accepted only when both links agree: `root/.git` names `<common>/worktrees/<name>`, that
/// directory's `commondir` names `<common>`, `<common>` is the `.git` directory of its parent,
/// and git's own back-link `<common>/worktrees/<name>/gitdir` points at `root/.git`. A stray or
/// planted `.git` file therefore cannot redirect a directory into another project's workspace.
/// Submodules (`gitdir` under `.git/modules/`) have no `commondir` and stay their own project.
fn worktree_main_root(root: &Path) -> Option<PathBuf> {
    let dot_git = root.join(".git");
    if !std::fs::symlink_metadata(&dot_git).ok()?.is_file() {
        return None;
    }
    let read_small = read_control_file;
    let gitdir_line = read_small(&dot_git)?;
    let gitdir = gitdir_line.trim().strip_prefix("gitdir:")?.trim();
    let gitdir = std::fs::canonicalize(root.join(gitdir)).ok()?;
    let common_rel = read_small(&gitdir.join("commondir"))?;
    let common = std::fs::canonicalize(gitdir.join(common_rel.trim())).ok()?;
    if common.file_name()? != ".git" || gitdir.parent()? != common.join("worktrees") {
        return None;
    }
    let back = read_small(&gitdir.join("gitdir"))?;
    // A relative back-link (`git worktree add --relative-paths`) is relative to `gitdir`.
    let back = std::fs::canonicalize(gitdir.join(back.trim())).ok()?;
    if back != std::fs::canonicalize(&dot_git).ok()? {
        return None;
    }
    let main = common.parent()?.to_path_buf();
    std::fs::symlink_metadata(main.join(".git"))
        .ok()?
        .is_dir()
        .then_some(main)
}

/// Linux: `(parent pid, start time in clock ticks, comm)` of `pid` from `/proc/<pid>/stat`. The
/// start time makes a record specific to one process, so a reused PID never matches an old record.
#[cfg(target_os = "linux")]
fn proc_info(pid: u32) -> Option<(u32, u64, String)> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // `comm` (field 2) is parenthesised and may contain spaces: split around its last `)`.
    let close = stat.rfind(')')?;
    let comm = stat.get(stat.find('(')? + 1..close)?.to_owned();
    let fields: Vec<&str> = stat.get(close + 1..)?.split_whitespace().collect();
    // After `comm`: state(3) ppid(4) ... starttime(22) -> indices 0, 1, 19.
    Some((
        fields.get(1)?.parse().ok()?,
        fields.get(19)?.parse().ok()?,
        comm,
    ))
}

/// Shells and launchers that sit between an agent and the `icx` it runs; never the agent itself.
#[cfg(target_os = "linux")]
const LAUNCHERS: &[&str] = &[
    "sh", "bash", "dash", "zsh", "fish", "ksh", "busybox", "env", "timeout", "nice", "nohup",
    "setsid", "stdbuf", "flock", "icx-hook", "bwrap", "unshare", "firejail", "direnv", "mise",
    "asdf", "npx", "uvx", "pnpm", "bunx",
];

/// The agent process this `icx` runs under, as `(pid, start time)`: the nearest ancestor that is
/// not a shell or launcher, looking at most a few levels up. Recording and lookup both use only
/// this process, so a record never lands on a terminal, multiplexer or session manager shared
/// with other sessions, and a child agent never overwrites its parent's record (review of #38).
#[cfg(target_os = "linux")]
fn agent_of_self() -> Option<(u32, u64)> {
    let (mut pid, _, _) = proc_info(std::process::id())?;
    for _ in 0..AGENT_SEARCH_DEPTH {
        if pid <= 1 {
            return None;
        }
        let (parent, start, comm) = proc_info(pid)?;
        if !LAUNCHERS.contains(&comm.as_str()) {
            return Some((pid, start));
        }
        pid = parent;
    }
    None
}

/// How many ancestors may be launchers before the agent is given up on.
#[cfg(target_os = "linux")]
const AGENT_SEARCH_DEPTH: usize = 5;

fn agent_registry_dir(roots: &WorkspaceRoots) -> PathBuf {
    roots.runtime_dir.join("agents")
}

/// Records `ws_dir` as the workspace this process's agent is using (v1.3.2, #34), in
/// `$XDG_RUNTIME_DIR/icx/agents/<pid>-<start>.ws` (`0700`, file `0600`, replaced atomically).
/// Best effort; a failure only means the MCP server keeps its launch workspace. No-op off Linux.
pub fn record_agent_workspace(ws_dir: &Path, roots: &WorkspaceRoots) {
    #[cfg(target_os = "linux")]
    {
        let Ok(canonical) = std::fs::canonicalize(ws_dir) else {
            return;
        };
        let Some((pid, start)) = agent_of_self() else {
            return;
        };
        if ensure_runtime_dir(roots).is_err() {
            return;
        }
        let dir = agent_registry_dir(roots);
        if private_dir(&dir).is_err() || check_dir_security(&dir).is_err() {
            return;
        }
        let text = canonical.to_string_lossy().into_owned();
        let path = dir.join(format!("{pid}-{start}.ws"));
        if std::fs::read_to_string(&path).is_ok_and(|t| t == text) {
            return;
        }
        let tmp = dir.join(format!(".{pid}-{start}.{}.tmp", std::process::id()));
        // `create_new` opens with O_CREAT|O_EXCL, which refuses any existing path, a planted
        // symlink included.
        let written = {
            use std::os::unix::fs::OpenOptionsExt;
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&tmp)
                .and_then(|mut f| f.write_all(text.as_bytes()))
        };
        if written.is_err() || std::fs::rename(&tmp, &path).is_err() {
            let _ = std::fs::remove_file(&tmp);
            return;
        }
        prune_agent_records(&dir);
    }
    #[cfg(not(target_os = "linux"))]
    let _ = (ws_dir, roots);
}

/// Removes records whose agent process has exited (or whose PID was reused). Runs only after a
/// new record is written, so a long session pays nothing per hook (review of #38).
#[cfg(target_os = "linux")]
fn prune_agent_records(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(stem) = name.to_str().and_then(|n| n.strip_suffix(".ws")) else {
            continue;
        };
        let Some((pid, start)) = stem.split_once('-') else {
            continue;
        };
        let (Ok(pid), Ok(start)) = (pid.parse::<u32>(), start.parse::<u64>()) else {
            continue;
        };
        if proc_info(pid).is_none_or(|(_, s, _)| s != start) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// The workspace this MCP server's own agent's hooks are using, if recorded (v1.3.2, #34). Only
/// a default per-project workspace under `<state>/workspaces/` is accepted, so a record can never
/// point the MCP server outside icx's own workspaces; trust checks still run on use.
pub fn agent_workspace(roots: &WorkspaceRoots) -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        let (pid, start) = agent_of_self()?;
        let dir = agent_registry_dir(roots);
        // The registry must be private, and the record owned by the same user and not group- or
        // world-accessible, so another account cannot steer this server (review of #38).
        check_dir_security(&dir).ok()?;
        let owner = std::fs::symlink_metadata(&dir).ok()?.uid();
        let path = dir.join(format!("{pid}-{start}.ws"));
        let meta = std::fs::symlink_metadata(&path).ok()?;
        if !meta.is_file() || meta.len() > 4096 || meta.uid() != owner || meta.mode() & 0o077 != 0 {
            return None;
        }
        let text = std::fs::read_to_string(&path).ok()?;
        let base = std::fs::canonicalize(roots.state_dir.join("workspaces")).ok()?;
        let candidate = PathBuf::from(text.trim());
        (candidate.is_absolute() && candidate.parent() == Some(base.as_path())).then_some(candidate)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = roots;
        None
    }
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

/// Per-process sequence for unique temp file names (`write_private_file`, `install_key`).
static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub(crate) fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), TrustError> {
    // Unique per call (PR #20 review): concurrent writers in one process must not share a temp.
    let tmp = path.with_extension(format!(
        "tmp.{}.{}",
        std::process::id(),
        TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&tmp).map_err(io_err)?;
    let written = f.write_all(bytes).and_then(|()| f.sync_all());
    drop(f);
    let result = written.and_then(|()| std::fs::rename(&tmp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result.map_err(io_err)
}

/// Loads the per-install key, creating it (32 bytes from the OS CSPRNG, mode `0600`) on first use.
pub fn install_key(roots: &WorkspaceRoots) -> Result<[u8; 32], TrustError> {
    require_absolute(&roots.config_dir, "config")?;
    let path = roots.config_dir.join(KEY_FILE);
    // Validate metadata before opening (PR #20 review): a FIFO or other special file must be
    // refused, not read (reading one blocks), and only `NotFound` leads to key creation.
    match std::fs::symlink_metadata(&path) {
        Ok(_) => {
            check_dir_security(&roots.config_dir)?;
            check_private_file(&path)?;
            let bytes = std::fs::read(&path).map_err(io_err)?;
            return bytes
                .try_into()
                .map_err(|_| TrustError::Io("install.key is not 32 bytes".into()));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(io_err(e)),
    }
    private_dir(&roots.config_dir)?;
    check_dir_security(&roots.config_dir)?;
    let mut key = [0u8; 32];
    #[cfg(unix)]
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut key))
        .map_err(io_err)?;
    #[cfg(not(unix))]
    return Err(TrustError::Io(
        "creating an install key needs a Unix OS RNG (/dev/urandom); supported platforms are \
         Linux and macOS"
            .into(),
    ));
    // Publish atomically: write a private temp file, then hard-link it into place. `link` fails
    // if the key already exists, so a concurrent first use can never observe a partially written
    // key (it either wins the link or reads the winner's complete file).
    // The per-process counter makes the name unique between threads of one process even when
    // the clock is coarse (two threads with the same pid and nanos collided on CI); the pid makes
    // it unique between processes.
    let tmp = roots.config_dir.join(format!(
        ".{KEY_FILE}.{}.{}.{}",
        std::process::id(),
        TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let written = opts.open(&tmp).and_then(|mut f| {
        f.write_all(&key)?;
        f.sync_all()
    });
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(io_err(e));
    }
    let linked = std::fs::hard_link(&tmp, &path);
    let _ = std::fs::remove_file(&tmp);
    match linked {
        Ok(()) => Ok(key),
        // Another process won the creation race: use its (complete) key.
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
    if !md.file_type().is_file() {
        return Err(TrustError::Io(format!(
            "{} is not a regular file",
            path.display()
        )));
    }
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
        return verify_awaiting_creator(dir, roots);
    }
    require_absolute(dir, "workspace")?;
    if let Some(parent) = dir.parent() {
        private_dir(parent)?;
    }
    // Creating the leaf (non-recursively) is the claim: exactly one concurrent caller creates the
    // directory and seals it. A caller that loses the race waits briefly for that seal instead of
    // seeing an unsealed directory (PR #20 review).
    let mut b = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        b.mode(0o700);
    }
    match b.create(dir) {
        Ok(()) => write_seal_and_ignore(dir, roots),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            verify_awaiting_creator(dir, roots)
        }
        Err(e) => Err(io_err(e)),
    }
}

/// `verify`, but while the directory holds nothing beyond what `prepare` writes before the seal
/// (`.gitignore` and temp files), waits up to 2 s for a concurrent creator to finish sealing it.
/// An unsealed directory with any other content is refused immediately.
fn verify_awaiting_creator(dir: &Path, roots: &WorkspaceRoots) -> Result<(), TrustError> {
    let being_created = |dir: &Path| {
        std::fs::read_dir(dir).is_ok_and(|entries| {
            entries.flatten().all(|e| {
                let name = e.file_name();
                let name = name.to_string_lossy();
                name == ".gitignore" || name.contains(".tmp.")
            })
        })
    };
    for _ in 0..200 {
        match verify(dir, roots) {
            // Re-check when the seal appeared after the failed read (the creator just finished).
            Err(TrustError::MissingSeal) if being_created(dir) || dir.join(SEAL_FILE).exists() => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            other => return other,
        }
    }
    verify(dir, roots)
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
