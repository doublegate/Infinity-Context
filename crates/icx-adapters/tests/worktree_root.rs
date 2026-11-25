//! v1.3.2 (#37): a linked git worktree belongs to its main repository's project; a planted
//! `.git` file or a submodule does not redirect anything.
use icx_adapters::workspace::project_root;
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
