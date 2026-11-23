//! `icx install` end to end (T-65-004, T-65-005), in a throwaway HOME: dry runs write nothing,
//! a real install backs up and is idempotent, existing icx hooks are not duplicated, unknown keys
//! and comments survive, and an unparsable file is never rewritten.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const ICX: &str = "/opt/icx/bin/icx";

fn home(tag: &str) -> PathBuf {
    let h = std::env::temp_dir().join(format!("icx-install-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&h);
    fs::create_dir_all(&h).unwrap();
    h
}

fn run(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_icx"))
        .arg("install")
        .args(args)
        .args(["--icx", ICX])
        .env("HOME", home)
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("CODEX_HOME")
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("HERMES_HOME")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn out(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

/// Every file under `dir`, with its content.
fn snapshot(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut v = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                v.push((p.clone(), fs::read_to_string(&p).unwrap_or_default()));
            }
        }
    }
    v.sort();
    v
}

#[test]
fn a_dry_run_writes_nothing_and_shows_diffs() {
    let h = home("dry");
    fs::create_dir_all(h.join(".codex")).unwrap();
    fs::write(h.join(".codex/config.toml"), "# mine\nmodel = \"x\"\n").unwrap();
    let before = snapshot(&h);
    let o = run(
        &h,
        &[
            "--agent",
            "claude-code",
            "--agent",
            "codex",
            "--agent",
            "agy",
            "--agent",
            "muse",
            "--agent",
            "gemini-cli",
            "--agent",
            "cursor",
            "--agent",
            "opencode",
            "--agent",
            "hermes",
            "--dry-run",
            "--approve-tools",
        ],
    );
    assert!(o.status.success(), "{}", out(&o));
    assert_eq!(snapshot(&h), before, "a dry run must not write");
    let text = out(&o);
    assert!(
        text.contains("+++ ") && text.contains("dry run: nothing was written"),
        "{text}"
    );
    assert!(text.contains(&format!("{ICX} hook --agent codex --event PostToolUse")));
}

#[test]
fn without_yes_and_without_a_terminal_nothing_is_written() {
    let h = home("noyes");
    let o = run(&h, &["--agent", "codex"]);
    assert!(!o.status.success());
    assert!(out(&o).contains("re-run with --yes"), "{}", out(&o));
    assert!(
        snapshot(&h).is_empty(),
        "nothing written without confirmation"
    );
}

#[test]
fn a_real_install_backs_up_keeps_other_content_and_is_idempotent() {
    let h = home("real");
    fs::create_dir_all(h.join(".codex")).unwrap();
    let toml = "# my settings, keep this comment\nmodel = \"gpt\"   # inline comment\n\n[profiles.fast]\nmodel = \"mini\"\n";
    fs::write(h.join(".codex/config.toml"), toml).unwrap();
    fs::create_dir_all(h.join(".claude")).unwrap();
    fs::write(
        h.join(".claude/settings.json"),
        r#"{"theme": "dark", "zeta": {"b": 1, "a": 2}, "hooks": {"Stop": [{"hooks": [{"type": "command", "command": "other-tool"}]}]}}"#,
    )
    .unwrap();

    let o = run(
        &h,
        &[
            "--agent",
            "codex",
            "--agent",
            "claude-code",
            "--yes",
            "--approve-tools",
        ],
    );
    assert!(o.status.success(), "{}", out(&o));

    let config = fs::read_to_string(h.join(".codex/config.toml")).unwrap();
    assert!(
        config.starts_with("# my settings, keep this comment\nmodel = \"gpt\"   # inline comment")
    );
    assert!(config.contains("[profiles.fast]"));
    assert!(config.contains(&format!("command = \"{ICX}\"")));
    assert!(
        config.contains("[mcp_servers.icx.tools.icx]")
            && config.contains("approval_mode = \"approve\"")
    );
    assert!(
        !config.contains("icx_write"),
        "only the read-only tool is approved"
    );

    let settings: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(h.join(".claude/settings.json")).unwrap())
            .unwrap();
    assert_eq!(settings["theme"], "dark");
    assert_eq!(settings["zeta"]["a"], 2);
    assert_eq!(
        settings["hooks"]["Stop"][0]["hooks"][0]["command"],
        "other-tool"
    );
    assert_eq!(
        settings["permissions"]["allow"],
        serde_json::json!(["mcp__icx__icx"])
    );
    let text = fs::read_to_string(h.join(".claude/settings.json")).unwrap();
    assert!(
        text.find("\"theme\"").unwrap() < text.find("\"zeta\"").unwrap(),
        "key order kept"
    );
    assert!(text.find("\"b\"").unwrap() < text.find("\"a\"").unwrap());
    assert!(text.contains(&format!(
        "{ICX} hook --agent claude-code --event CwdChanged"
    )));

    let backups: Vec<_> = snapshot(&h)
        .into_iter()
        .filter(|(p, _)| p.to_string_lossy().contains(".icx-backup-"))
        .collect();
    assert_eq!(backups.len(), 2, "both existing files were backed up");
    assert!(backups.iter().any(|(_, c)| c == toml));

    let again = run(
        &h,
        &[
            "--agent",
            "codex",
            "--agent",
            "claude-code",
            "--yes",
            "--approve-tools",
        ],
    );
    assert!(again.status.success(), "{}", out(&again));
    assert!(out(&again).contains("no file changes"), "{}", out(&again));
    assert!(
        !out(&again).contains("wrote"),
        "a second run changes nothing"
    );
}

#[test]
fn an_existing_icx_hook_is_not_duplicated() {
    let h = home("existing");
    fs::create_dir_all(h.join(".codex")).unwrap();
    fs::write(
        h.join(".codex/hooks.json"),
        r#"{"hooks": {"PostToolUse": [{"matcher": "", "hooks": [{"type": "command", "command": "/home/u/.local/bin/icx-hook --agent codex --event PostToolUse"}]}]}}"#,
    )
    .unwrap();
    let o = run(&h, &["--agent", "codex", "--yes"]);
    assert!(o.status.success(), "{}", out(&o));
    let v: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(h.join(".codex/hooks.json")).unwrap()).unwrap();
    assert_eq!(
        v["hooks"]["PostToolUse"].as_array().unwrap().len(),
        1,
        "not added twice"
    );
    assert!(
        v["hooks"]["SessionStart"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .starts_with(ICX)
    );
}

#[test]
fn an_unparsable_file_is_left_alone_with_a_note() {
    let h = home("jsonc");
    fs::create_dir_all(h.join(".cursor")).unwrap();
    let jsonc = "{\n  // my comment\n  \"version\": 1\n}\n";
    fs::write(h.join(".cursor/hooks.json"), jsonc).unwrap();
    let o = run(&h, &["--agent", "cursor", "--yes"]);
    assert!(o.status.success(), "{}", out(&o));
    assert_eq!(
        fs::read_to_string(h.join(".cursor/hooks.json")).unwrap(),
        jsonc
    );
    assert!(
        out(&o).contains("not edited") && out(&o).contains("by hand"),
        "{}",
        out(&o)
    );
    assert!(
        h.join(".cursor/mcp.json").exists(),
        "the other Cursor file is still written"
    );
}

#[test]
fn every_agent_plans_without_error_and_agy_needs_no_wrapper() {
    let h = home("all");
    let o = run(
        &h,
        &[
            "--agent",
            "claude-code",
            "--agent",
            "codex",
            "--agent",
            "agy",
            "--agent",
            "muse",
            "--agent",
            "gemini-cli",
            "--agent",
            "cursor",
            "--agent",
            "opencode",
            "--agent",
            "hermes",
            "--yes",
            "--approve-tools",
        ],
    );
    assert!(o.status.success(), "{}", out(&o));
    let hooks = fs::read_to_string(h.join(".gemini/config/hooks.json")).unwrap();
    assert!(hooks.contains(&format!("{ICX} hook --agent agy --event PreInvocation")));
    let plugin = fs::read_to_string(h.join(".config/opencode/plugins/icx.ts")).unwrap();
    assert!(plugin.contains("id: \"icx\""));
    let oc: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(h.join(".config/opencode/opencode.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(oc["mcp"]["servers"]["icx"]["command"][1], "mcp-serve");
    let agy_perm = fs::read_to_string(h.join(".gemini/antigravity-cli/settings.json")).unwrap();
    assert!(agy_perm.contains("mcp(icx/icx)"));
    assert!(
        fs::read_to_string(h.join(".cursor/cli-config.json"))
            .unwrap()
            .contains("Mcp(icx:icx)")
    );
    assert!(out(&o).contains("claude mcp add --scope user icx"));
    let _ = fs::remove_dir_all(&h);
}

#[test]
fn an_unknown_agent_is_an_error() {
    let h = home("unknown");
    let o = run(&h, &["--agent", "nope", "--dry-run"]);
    assert!(!o.status.success());
    assert!(out(&o).contains("unknown agent"), "{}", out(&o));
}

#[test]
fn all_installs_only_the_agents_that_are_installed() {
    let h = home("allmissing");
    let empty = h.join("empty-path");
    fs::create_dir_all(&empty).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_icx"))
        .args(["install", "--agent", "all", "--dry-run", "--icx", ICX])
        .env("HOME", &h)
        .env("PATH", &empty)
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("HERMES_HOME")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", out(&o));
    assert!(
        out(&o).contains("skipped (not installed): claude-code, codex, agy"),
        "{}",
        out(&o)
    );
    assert!(
        !out(&o).contains("+++ "),
        "nothing is planned for missing agents"
    );
}

#[test]
fn a_file_with_nothing_to_add_is_left_byte_for_byte() {
    let h = home("unchanged");
    fs::create_dir_all(h.join(".cursor")).unwrap();
    // Already registered, odd formatting, no trailing newline.
    let mcp = "{\"mcpServers\":{\"icx\":{\"command\":\"/x/icx\",\"args\":[\"mcp-serve\"]}}}";
    fs::write(h.join(".cursor/mcp.json"), mcp).unwrap();
    let o = run(&h, &["--agent", "cursor", "--yes"]);
    assert!(o.status.success(), "{}", out(&o));
    assert_eq!(fs::read_to_string(h.join(".cursor/mcp.json")).unwrap(), mcp);
}

#[test]
fn a_server_wide_allow_rule_makes_the_tool_rule_redundant() {
    let h = home("allowed");
    fs::create_dir_all(h.join(".claude")).unwrap();
    fs::write(
        h.join(".claude/settings.json"),
        r#"{"permissions": {"allow": ["mcp__icx"]}}"#,
    )
    .unwrap();
    let o = run(&h, &["--agent", "claude-code", "--yes", "--approve-tools"]);
    assert!(o.status.success(), "{}", out(&o));
    let v: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(h.join(".claude/settings.json")).unwrap())
            .unwrap();
    assert_eq!(v["permissions"]["allow"], serde_json::json!(["mcp__icx"]));
}

#[cfg(unix)]
#[test]
fn modes_are_kept_new_files_are_private_and_symlinks_stay_links() {
    use std::os::unix::fs::PermissionsExt;
    let h = home("modes");
    fs::create_dir_all(h.join(".codex")).unwrap();
    fs::create_dir_all(h.join("dotfiles")).unwrap();
    // A private config, and a settings file that is a dotfiles symlink.
    fs::write(h.join(".codex/config.toml"), "model = \"x\"\n").unwrap();
    fs::set_permissions(
        h.join(".codex/config.toml"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    fs::write(h.join("dotfiles/hooks.json"), "{}").unwrap();
    std::os::unix::fs::symlink(h.join("dotfiles/hooks.json"), h.join(".codex/hooks.json")).unwrap();

    let o = run(&h, &["--agent", "codex", "--yes"]);
    assert!(o.status.success(), "{}", out(&o));
    let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(
        mode(&h.join(".codex/config.toml")),
        0o600,
        "the mode is kept"
    );
    let backups: Vec<_> = snapshot(&h)
        .into_iter()
        .map(|(p, _)| p)
        .filter(|p| p.to_string_lossy().contains(".icx-backup-"))
        .collect();
    assert_eq!(backups.len(), 2, "{backups:?}");
    for b in &backups {
        if b.to_string_lossy().contains("config.toml") {
            assert_eq!(mode(b), 0o600, "the backup is as private as the original");
        }
    }
    let link = h.join(".codex/hooks.json");
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink(),
        "still a link"
    );
    assert!(
        fs::read_to_string(h.join("dotfiles/hooks.json"))
            .unwrap()
            .contains("--agent codex")
    );

    // A file the installer creates is private.
    let h2 = home("newfile");
    let o = run(&h2, &["--agent", "cursor", "--yes"]);
    assert!(o.status.success(), "{}", out(&o));
    assert_eq!(mode(&h2.join(".cursor/mcp.json")), 0o600);
}

#[test]
fn hermes_gets_both_plugins_and_its_yaml_is_left_alone() {
    let h = home("hermes");
    fs::create_dir_all(h.join(".hermes")).unwrap();
    let yaml = "# mine\ncontext:\n  engine: compressor\n";
    fs::write(h.join(".hermes/config.yaml"), yaml).unwrap();
    let o = run(&h, &["--agent", "hermes", "--yes"]);
    assert!(o.status.success(), "{}", out(&o));
    let engine = fs::read_to_string(h.join(".hermes/plugins/icx/__init__.py")).unwrap();
    assert!(engine.contains("class IcxContextEngine"));
    let manifest = fs::read_to_string(h.join(".hermes/plugins/icx/plugin.yaml")).unwrap();
    assert!(!manifest.contains("kind:"), "no invalid manifest kind");
    assert!(h.join(".hermes/plugins/icx-terminal/__init__.py").exists());
    assert_eq!(
        fs::read_to_string(h.join(".hermes/config.yaml")).unwrap(),
        yaml
    );
    assert!(out(&o).contains("engine: icx") && out(&o).contains("icx-terminal"));
    let again = run(&h, &["--agent", "hermes", "--yes"]);
    assert!(out(&again).contains("no file changes"), "{}", out(&again));
}

#[test]
fn argument_errors_are_reported() {
    let h = home("args");
    let bin = env!("CARGO_BIN_EXE_icx");
    let run_raw = |args: &[&str], with_home: bool| {
        let mut c = Command::new(bin);
        c.arg("install").args(args).stdin(Stdio::null());
        if with_home {
            c.env("HOME", &h);
        } else {
            c.env_remove("HOME");
        }
        c.env_remove("XDG_CONFIG_HOME")
            .env_remove("HERMES_HOME")
            .output()
            .unwrap()
    };
    let o = run_raw(&["--bogus"], true);
    assert!(
        !o.status.success() && out(&o).contains("unexpected argument"),
        "{}",
        out(&o)
    );
    let o = run_raw(&["--dry-run"], true);
    assert!(
        !o.status.success() && out(&o).contains("--agent is required"),
        "{}",
        out(&o)
    );
    let o = run_raw(&["--agent", "codex", "--dry-run"], false);
    assert!(
        !o.status.success() && out(&o).contains("HOME is not set"),
        "{}",
        out(&o)
    );
    // Without --icx the hooks run the binary that did the install, by absolute path.
    let o = run_raw(&["--agent", "codex", "--dry-run"], true);
    assert!(o.status.success(), "{}", out(&o));
    let exe = std::fs::canonicalize(bin).unwrap();
    assert!(
        out(&o).contains(&format!("{} hook --agent codex", exe.display())),
        "{}",
        out(&o)
    );
    assert!(snapshot(&h).is_empty(), "dry runs write nothing");
}

fn run_with_icx(home: &Path, icx: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_icx"))
        .arg("install")
        .args(args)
        .args(["--icx", icx])
        .env("HOME", home)
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("CODEX_HOME")
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("HERMES_HOME")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

#[test]
fn a_file_icx_cannot_read_as_text_is_never_replaced() {
    let h = home("binary");
    fs::create_dir_all(h.join(".codex")).unwrap();
    let bytes = b"# caf\xe9 (latin-1 comment)\nmodel = \"x\"\n".to_vec();
    fs::write(h.join(".codex/config.toml"), &bytes).unwrap();
    let o = run(&h, &["--agent", "codex", "--yes"]);
    assert!(o.status.success(), "{}", out(&o));
    assert_eq!(fs::read(h.join(".codex/config.toml")).unwrap(), bytes);
    assert!(out(&o).contains("not UTF-8"), "{}", out(&o));
}

#[test]
fn an_icx_path_with_spaces_is_quoted_and_recognised_on_the_next_run() {
    let h = home("spaces");
    let icx = "/opt/my tools/icx";
    let o = run_with_icx(&h, icx, &["--agent", "codex", "--yes"]);
    assert!(o.status.success(), "{}", out(&o));
    let hooks = fs::read_to_string(h.join(".codex/hooks.json")).unwrap();
    assert!(
        hooks.contains("'/opt/my tools/icx' hook --agent codex"),
        "{hooks}"
    );
    let again = run_with_icx(&h, icx, &["--agent", "codex", "--yes"]);
    assert!(out(&again).contains("no file changes"), "{}", out(&again));
}

#[test]
fn env_prefixed_hooks_are_not_duplicated() {
    let h = home("envprefix");
    fs::create_dir_all(h.join(".cursor")).unwrap();
    fs::write(
        h.join(".cursor/hooks.json"),
        r#"{"version": 1, "hooks": {"postToolUse": [{"command": "ICX_CURSOR_STUB=1 icx hook --agent cursor --event postToolUse"}]}}"#,
    )
    .unwrap();
    let o = run(&h, &["--agent", "cursor", "--yes"]);
    assert!(o.status.success(), "{}", out(&o));
    let v: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(h.join(".cursor/hooks.json")).unwrap()).unwrap();
    assert_eq!(v["hooks"]["postToolUse"].as_array().unwrap().len(), 1);
}

#[cfg(unix)]
#[test]
fn a_dangling_dotfiles_link_stays_a_link() {
    let h = home("dangling");
    fs::create_dir_all(h.join(".cursor")).unwrap();
    fs::create_dir_all(h.join("dotfiles")).unwrap();
    std::os::unix::fs::symlink(h.join("dotfiles/mcp.json"), h.join(".cursor/mcp.json")).unwrap();
    let o = run(&h, &["--agent", "cursor", "--yes"]);
    assert!(o.status.success(), "{}", out(&o));
    let link = h.join(".cursor/mcp.json");
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(
        fs::read_to_string(h.join("dotfiles/mcp.json"))
            .unwrap()
            .contains("mcp-serve")
    );
}

#[test]
fn a_repeated_agent_is_planned_once() {
    let h = home("repeat");
    fs::create_dir_all(h.join(".codex")).unwrap();
    fs::write(h.join(".codex/config.toml"), "model = \"x\"\n").unwrap();
    let o = run(&h, &["--agent", "codex", "--agent", "codex", "--yes"]);
    assert!(o.status.success(), "{}", out(&o));
    assert_eq!(out(&o).matches("=== codex").count(), 1, "{}", out(&o));
}
