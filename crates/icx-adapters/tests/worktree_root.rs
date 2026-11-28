//! v1.3.2 (#37): a linked git worktree belongs to its main repository's project; a planted
//! `.git` file or a submodule does not redirect anything.
use icx_adapters::workspace::{has_git_repository, project_root};
use std::path::{Path, PathBuf};
use std::process::Command;

fn base_dir(name: &str) -> PathBuf {
    let base = std::env::temp_dir()
        .join(format!(
            "icx-tests-{}",
            std::env::var("USER").unwrap_or_default()
        ))
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    std::fs::canonicalize(&base).unwrap()
}

fn git(cwd: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(cwd)
        // The scratch repository must not depend on the user's git config (commit signing
        // through an agent that may not be running).
        .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
        .args(["-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false"])
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {out:?}");
}

#[test]
fn worktrees_resolve_to_the_main_repository_and_forgeries_do_not() {
    let base = base_dir("worktree-root");
    let main = base.join("main");
    std::fs::create_dir_all(main.join("src")).unwrap();
    git(&main, &["init", "-q"]);
    std::fs::write(main.join("README"), "x\n").unwrap();
    git(&main, &["add", "README"]);
    git(&main, &["commit", "-q", "-m", "init"]);
    git(&main, &["worktree", "add", "-q", "../wt", "-b", "agent"]);
    let wt = base.join("wt");
    std::fs::create_dir_all(wt.join("deep/er")).unwrap();

    assert_eq!(project_root(&main.join("src")), main);
    assert_eq!(project_root(&wt), main);
    assert_eq!(project_root(&wt.join("deep/er")), main);

    // A planted `.git` file naming the main repository's worktree dir without git's back-link.
    let forged = base.join("forged");
    std::fs::create_dir_all(&forged).unwrap();
    let real_gitdir = std::fs::read_to_string(wt.join(".git")).unwrap();
    std::fs::write(forged.join(".git"), real_gitdir).unwrap();
    assert_eq!(project_root(&forged), forged);

    // A gitdir without `commondir` (submodule layout) stays its own project.
    let sub = base.join("sub");
    let modules = main.join(".git/modules/sub");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::create_dir_all(&modules).unwrap();
    std::fs::write(sub.join(".git"), format!("gitdir: {}\n", modules.display())).unwrap();
    assert_eq!(project_root(&sub), sub);

    // Garbage in `.git` is ignored.
    let junk = base.join("junk");
    std::fs::create_dir_all(&junk).unwrap();
    std::fs::write(junk.join(".git"), "not a gitdir line\n").unwrap();
    assert_eq!(project_root(&junk), junk);

    // A relative back-link (`git worktree add --relative-paths`) resolves against the gitdir.
    let back = main.join(".git/worktrees/wt/gitdir");
    let absolute = std::fs::read_to_string(&back).unwrap();
    std::fs::write(&back, "../../../../wt/.git\n").unwrap();
    assert_eq!(project_root(&wt), main, "relative back-link");

    // A symlinked control file is refused (review of #38; ICX-AUD-018).
    let copy = base.join("gitdir-copy");
    std::fs::write(&copy, &absolute).unwrap();
    std::fs::remove_file(&back).unwrap();
    std::os::unix::fs::symlink(&copy, &back).unwrap();
    assert_eq!(project_root(&wt), wt, "symlinked back-link");
    let _ = std::fs::remove_dir_all(&base);
}

/// git's own answer for `cwd`: the work tree top, or `None` outside any repository. Discovery
/// stops at `base`'s parent, so a repository above the test's temp directory cannot answer.
fn git_toplevel(base: &Path, cwd: &Path) -> Option<PathBuf> {
    let out = Command::new("git")
        .current_dir(cwd)
        .env("GIT_CEILING_DIRECTORIES", base.parent().unwrap_or(base))
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .unwrap();
    out.status
        .success()
        .then(|| std::fs::canonicalize(String::from_utf8_lossy(&out.stdout).trim()).unwrap())
}

#[test]
fn a_git_directory_git_rejects_does_not_make_a_project_root() {
    // #120: a stray empty `/tmp/.git` captured every project under /tmp into one workspace.
    let base = base_dir("stray-dot-git");

    // An empty `.git` above a plain directory: neither git nor icx sees a repository.
    let plain = base.join("outer/plain");
    std::fs::create_dir_all(&plain).unwrap();
    std::fs::create_dir_all(base.join("outer/.git")).unwrap();
    assert_eq!(git_toplevel(&base, &plain), None);
    assert_eq!(project_root(&plain), plain);
    assert!(!has_git_repository(&base.join("outer")));

    // An empty, then a partial (`HEAD` only), `.git` between a real repository and the cwd:
    // the search skips it and finds the repository, as git does.
    let repo = base.join("repo");
    let mid = repo.join("mid");
    let deep = mid.join("x");
    std::fs::create_dir_all(&deep).unwrap();
    git(&repo, &["init", "-q"]);
    assert!(has_git_repository(&repo), "a repository git created");
    std::fs::create_dir_all(mid.join(".git")).unwrap();
    assert_eq!(git_toplevel(&base, &deep), Some(repo.clone()));
    assert_eq!(project_root(&deep), repo);
    std::fs::write(mid.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
    assert_eq!(git_toplevel(&base, &deep), Some(repo.clone()));
    assert_eq!(project_root(&deep), repo, "HEAD without objects or refs");

    // A `.git` file naming a directory that is not a repository ends the search: git reports
    // "not a git repository" there rather than looking further up, so the cwd is its own project.
    std::fs::remove_dir_all(mid.join(".git")).unwrap();
    std::fs::create_dir_all(base.join("not-a-repo")).unwrap();
    std::fs::write(
        mid.join(".git"),
        format!("gitdir: {}\n", base.join("not-a-repo").display()),
    )
    .unwrap();
    assert_eq!(git_toplevel(&base, &deep), None);
    assert_eq!(project_root(&deep), deep, ".git file naming no repository");

    // The smallest directory git accepts (`HEAD`, `objects`, `refs`) is a repository for both.
    std::fs::remove_file(mid.join(".git")).unwrap();
    for d in ["objects", "refs"] {
        std::fs::create_dir_all(mid.join(".git").join(d)).unwrap();
    }
    std::fs::write(mid.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
    assert_eq!(git_toplevel(&base, &deep), Some(mid.clone()));
    assert_eq!(project_root(&deep), mid);

    // git checks only how `HEAD` starts, so a second line, `ref:` without a space and a bare
    // object name are all valid; a `ref:` outside `refs/` is not.
    for head in [
        "ref: refs/heads/main\nextra\n",
        "ref:refs/heads/main\n",
        "0123456789abcdef0123456789abcdef01234567\n",
    ] {
        std::fs::write(mid.join(".git/HEAD"), head).unwrap();
        assert_eq!(git_toplevel(&base, &deep), Some(mid.clone()), "{head:?}");
        assert_eq!(project_root(&deep), mid, "{head:?}");
    }
    std::fs::write(mid.join(".git/HEAD"), "ref: heads/main\n").unwrap();
    assert_eq!(git_toplevel(&base, &deep), Some(repo.clone()));
    assert_eq!(project_root(&deep), repo, "ref outside refs/");

    // A `.git` file must be exactly one `gitdir:` line, even when it names a real repository.
    std::fs::remove_dir_all(mid.join(".git")).unwrap();
    std::fs::write(
        mid.join(".git"),
        format!("gitdir: {}\nextra\n", repo.join(".git").display()),
    )
    .unwrap();
    assert_eq!(git_toplevel(&base, &deep), None);
    assert_eq!(project_root(&deep), deep, "two-line .git file");
    let _ = std::fs::remove_dir_all(&base);
}

#[cfg(unix)]
#[test]
fn planted_control_files_cannot_block_or_bloat_the_lookup() {
    // `project_root` runs on every hook: a FIFO named `HEAD` in an ancestor must not block it,
    // and an oversized `HEAD` is not read (security review of #119).
    let base = base_dir("planted-control-files");
    let cwd = base.join("fifo/work");
    std::fs::create_dir_all(&cwd).unwrap();
    let gitdir = base.join("fifo/.git");
    for d in ["objects", "refs"] {
        std::fs::create_dir_all(gitdir.join(d)).unwrap();
    }
    let made = Command::new("mkfifo")
        .arg(gitdir.join("HEAD"))
        .status()
        .unwrap();
    assert!(made.success());
    let (tx, rx) = std::sync::mpsc::channel();
    let probe = cwd.clone();
    std::thread::spawn(move || tx.send(project_root(&probe)).unwrap());
    let found = rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("project_root blocked on a FIFO HEAD");
    assert_eq!(found, cwd, "a FIFO is not a valid HEAD");

    // A `.git` that is neither a file nor a directory (here a FIFO) is no repository either.
    let odd = base.join("odd");
    std::fs::create_dir_all(odd.join("work")).unwrap();
    assert!(
        Command::new("mkfifo")
            .arg(odd.join(".git"))
            .status()
            .unwrap()
            .success()
    );
    assert!(!has_git_repository(&odd));
    assert_eq!(project_root(&odd.join("work")), odd.join("work"));

    let big = base.join("big");
    std::fs::create_dir_all(big.join("work")).unwrap();
    for d in ["objects", "refs"] {
        std::fs::create_dir_all(big.join(".git").join(d)).unwrap();
    }
    let mut head = "ref: refs/heads/main\n".to_owned();
    head.push_str(&" ".repeat(8192));
    std::fs::write(big.join(".git/HEAD"), head).unwrap();
    assert_eq!(project_root(&big.join("work")), big.join("work"));
    let _ = std::fs::remove_dir_all(&base);
}
