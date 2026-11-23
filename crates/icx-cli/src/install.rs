//! `icx install --agent <name>` (T-65-004) and MCP tool pre-approval (T-65-005).
//!
//! An install is a [`Plan`]: the new content of every file it would change, plus notes for steps
//! it does not take itself. Planning reads files and never writes; `--dry-run` prints the plan as
//! unified diffs and stops. Applying writes only after explicit confirmation, backs up every file
//! that existed, writes through a temporary file and a rename, and keeps the original mode.
//!
//! Edits are additive and idempotent: a hook is added only when no hook running `icx` (or the
//! `icx-hook` wrapper) for the same agent and event is already registered, and an MCP server or
//! permission entry only when it is missing. Unknown keys, and the order of keys, in JSON files
//! are kept; Codex's `config.toml` is edited with `toml_edit`, which keeps comments and layout. A
//! file that does not parse (JSON with comments, for example) is never rewritten: the plan prints
//! the manual step instead.

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Agents `icx install` knows, in the order `--agent all` installs them.
pub const AGENTS: &[&str] = &[
    "claude-code",
    "codex",
    "agy",
    "muse",
    "gemini-cli",
    "cursor",
    "opencode",
    "hermes",
];

/// Whether `agent` looks installed: its program is on `PATH` (or in `~/.local/bin`, or
/// `~/.opencode/bin` for OpenCode), or, for Hermes, its home directory exists. `--agent all`
/// installs only these, so it never writes config for an agent that is not there.
#[must_use]
pub fn is_installed(agent: &str, o: &Opts) -> bool {
    let program = match agent {
        "claude-code" => "claude",
        "codex" => "codex",
        "agy" => "agy",
        "muse" => "muse",
        "gemini-cli" => "gemini",
        "cursor" => "cursor-agent",
        "opencode" => "opencode",
        "hermes" => return crate::install_hermes::hermes_home(o).is_dir(),
        _ => return false,
    };
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    dirs.push(o.home.join(".local/bin"));
    dirs.push(o.home.join(".opencode/bin"));
    dirs.iter().any(|d| d.join(program).is_file())
}

/// Where agent configs live and which `icx` the hooks run.
#[derive(Clone, Debug)]
pub struct Opts {
    pub home: PathBuf,
    pub config_home: PathBuf,
    pub codex_home: PathBuf,
    pub claude_home: PathBuf,
    /// The `icx` program written into hook and MCP commands.
    pub icx: String,
    /// Also pre-approve icx's read-only `icx` tool where the agent supports it (T-65-005).
    pub approve: bool,
}

/// One file the plan would change.
#[derive(Clone, Debug)]
pub struct FileEdit {
    pub path: PathBuf,
    /// The current content, `None` when the file does not exist yet.
    pub before: Option<String>,
    pub after: String,
}

/// What an install would do.
#[derive(Clone, Debug, Default)]
pub struct Plan {
    pub edits: Vec<FileEdit>,
    /// Steps the installer leaves to the user, and why.
    pub notes: Vec<String>,
}

impl Plan {
    fn note(&mut self, s: impl Into<String>) {
        self.notes.push(s.into());
    }

    /// Records `after` for `path` when it differs from what is there (or from an earlier edit of
    /// the same file in this plan).
    fn set(&mut self, path: &Path, before: Option<String>, after: String) {
        if let Some(e) = self.edits.iter_mut().find(|e| e.path == path) {
            e.after = after;
        } else if before.as_deref() != Some(after.as_str()) {
            self.edits.push(FileEdit {
                path: path.to_path_buf(),
                before,
                after,
            });
        }
        self.edits
            .retain(|e| e.before.as_deref() != Some(e.after.as_str()));
    }

    /// [`Plan::set`] for another module of the installer.
    pub(crate) fn set_file(&mut self, path: &Path, before: Option<String>, after: String) {
        self.set(path, before, after);
    }

    /// The current content of `path` as this plan would leave it so far.
    fn current(&self, path: &Path) -> Option<String> {
        self.edits
            .iter()
            .find(|e| e.path == path)
            .map(|e| e.after.clone())
            .or_else(|| fs::read_to_string(path).ok())
    }
}

// ---------------------------------------------------------------------------------------------
// Order-preserving JSON
// ---------------------------------------------------------------------------------------------

/// A JSON value whose objects keep their key order, so an edit changes only what it must.
#[derive(Clone, Debug, PartialEq)]
pub enum J {
    Null,
    Bool(bool),
    Num(serde_json::Number),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    fn obj() -> Self {
        J::Obj(Vec::new())
    }

    fn get(&self, key: &str) -> Option<&J> {
        match self {
            J::Obj(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            J::Str(s) => Some(s),
            _ => None,
        }
    }

    /// The object at `key`, created when missing. Errors when `key` holds a non-object.
    fn obj_mut(&mut self, key: &str) -> Result<&mut J, String> {
        let J::Obj(m) = self else {
            return Err(format!("expected an object around `{key}`"));
        };
        if !m.iter().any(|(k, _)| k == key) {
            m.push((key.to_owned(), J::obj()));
        }
        let v = m
            .iter_mut()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
            .ok_or_else(|| format!("missing `{key}`"))?;
        if matches!(v, J::Obj(_)) {
            Ok(v)
        } else {
            Err(format!("`{key}` is not an object"))
        }
    }

    /// The array at `key`, created when missing. Errors when `key` holds a non-array.
    fn arr_mut(&mut self, key: &str) -> Result<&mut Vec<J>, String> {
        let J::Obj(m) = self else {
            return Err(format!("expected an object around `{key}`"));
        };
        if !m.iter().any(|(k, _)| k == key) {
            m.push((key.to_owned(), J::Arr(Vec::new())));
        }
        match m.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v) {
            Some(J::Arr(a)) => Ok(a),
            _ => Err(format!("`{key}` is not an array")),
        }
    }

    fn insert(&mut self, key: &str, value: J) {
        if let J::Obj(m) = self {
            if let Some(slot) = m.iter_mut().find(|(k, _)| k == key) {
                slot.1 = value;
            } else {
                m.push((key.to_owned(), value));
            }
        }
    }

    /// Every string in the value, depth first.
    fn strings(&self) -> Vec<&str> {
        match self {
            J::Str(s) => vec![s.as_str()],
            J::Arr(a) => a.iter().flat_map(J::strings).collect(),
            J::Obj(m) => m.iter().flat_map(|(_, v)| v.strings()).collect(),
            _ => Vec::new(),
        }
    }
}

/// Builds a [`J`] from a `serde_json::json!` literal (key order as written).
fn j(v: serde_json::Value) -> J {
    match v {
        serde_json::Value::Null => J::Null,
        serde_json::Value::Bool(b) => J::Bool(b),
        serde_json::Value::Number(n) => J::Num(n),
        serde_json::Value::String(s) => J::Str(s),
        serde_json::Value::Array(a) => J::Arr(a.into_iter().map(j).collect()),
        serde_json::Value::Object(m) => J::Obj(m.into_iter().map(|(k, v)| (k, j(v))).collect()),
    }
}

/// An object from key/value pairs, in this order (`json!` objects are sorted).
fn obj(pairs: Vec<(&str, J)>) -> J {
    J::Obj(pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}

fn s(v: &str) -> J {
    J::Str(v.to_owned())
}

impl<'de> Deserialize<'de> for J {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = J;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("JSON")
            }
            fn visit_unit<E>(self) -> Result<J, E> {
                Ok(J::Null)
            }
            fn visit_none<E>(self) -> Result<J, E> {
                Ok(J::Null)
            }
            fn visit_bool<E>(self, b: bool) -> Result<J, E> {
                Ok(J::Bool(b))
            }
            fn visit_i64<E>(self, n: i64) -> Result<J, E> {
                Ok(J::Num(n.into()))
            }
            fn visit_u64<E>(self, n: u64) -> Result<J, E> {
                Ok(J::Num(n.into()))
            }
            fn visit_f64<E: de::Error>(self, n: f64) -> Result<J, E> {
                serde_json::Number::from_f64(n)
                    .map(J::Num)
                    .ok_or_else(|| E::custom("non-finite number"))
            }
            fn visit_str<E>(self, v: &str) -> Result<J, E> {
                Ok(J::Str(v.to_owned()))
            }
            fn visit_string<E>(self, v: String) -> Result<J, E> {
                Ok(J::Str(v))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<J, A::Error> {
                let mut v = Vec::new();
                while let Some(x) = a.next_element()? {
                    v.push(x);
                }
                Ok(J::Arr(v))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<J, A::Error> {
                let mut m: Vec<(String, J)> = Vec::new();
                while let Some((k, v)) = a.next_entry::<String, J>()? {
                    // A repeated key: the last one wins, as in serde_json, at its first place.
                    if let Some(slot) = m.iter_mut().find(|(key, _)| *key == k) {
                        slot.1 = v;
                    } else {
                        m.push((k, v));
                    }
                }
                Ok(J::Obj(m))
            }
        }
        d.deserialize_any(V)
    }
}

impl Serialize for J {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        match self {
            J::Null => ser.serialize_unit(),
            J::Bool(b) => ser.serialize_bool(*b),
            J::Num(n) => n.serialize(ser),
            J::Str(s) => ser.serialize_str(s),
            J::Arr(a) => {
                let mut seq = ser.serialize_seq(Some(a.len()))?;
                for x in a {
                    seq.serialize_element(x)?;
                }
                seq.end()
            }
            J::Obj(m) => {
                let mut map = ser.serialize_map(Some(m.len()))?;
                for (k, v) in m {
                    map.serialize_entry(k, v)?;
                }
                map.end()
            }
        }
    }
}

/// Reads a JSON object file for editing: `Ok(None)` when it does not exist, an error when it
/// cannot be read or parsed (the installer then leaves it alone).
fn read_json(plan: &Plan, path: &Path) -> Result<(Option<String>, J), String> {
    match plan.current(path) {
        None => Ok((None, J::obj())),
        Some(text) if text.trim().is_empty() => Ok((Some(text), J::obj())),
        Some(text) => {
            let v: J = serde_json::from_str(&text)
                .map_err(|e| format!("{} is not plain JSON ({e})", path.display()))?;
            if matches!(v, J::Obj(_)) {
                let before = fs::read_to_string(path).ok();
                Ok((before, v))
            } else {
                Err(format!("{} does not hold a JSON object", path.display()))
            }
        }
    }
}

fn json_text(v: &J) -> String {
    let mut out = serde_json::to_string_pretty(v).unwrap_or_else(|_| "{}".to_owned());
    out.push('\n');
    out
}

/// Applies `edit` to the JSON object in `path`, adding the result to the plan. A file that cannot
/// be parsed is left alone, with `manual` noted instead.
fn edit_json(
    plan: &mut Plan,
    path: &Path,
    manual: &str,
    edit: impl FnOnce(&mut J) -> Result<(), String>,
) {
    let (before, mut v) = match read_json(plan, path) {
        Ok(x) => x,
        Err(e) => {
            plan.note(format!("not edited: {e}. {manual}"));
            return;
        }
    };
    let unchanged = v.clone();
    if let Err(e) = edit(&mut v) {
        plan.note(format!("not edited: {}: {e}. {manual}", path.display()));
        return;
    }
    let before = before.or_else(|| fs::read_to_string(path).ok());
    // Nothing to add: leave the file exactly as it is, formatting included.
    if v == unchanged && before.is_some() {
        return;
    }
    let mut text = json_text(&v);
    // Keep the file's own end: no trailing newline if it had none.
    if before
        .as_deref()
        .is_some_and(|b| !b.is_empty() && !b.ends_with('\n'))
    {
        text.pop();
    }
    plan.set(path, before, text);
}

// ---------------------------------------------------------------------------------------------
// Recognising icx hooks already registered
// ---------------------------------------------------------------------------------------------

/// Whether `command` runs icx (`icx`, or the `icx-hook` wrapper, at any path) for `agent` and
/// `event`, in any flag order.
pub fn is_icx_hook_command(command: &str, agent: &str, event: &str) -> bool {
    let owned = shell_words(command);
    let words: Vec<&str> = owned.iter().map(String::as_str).collect();
    let program = words.first().and_then(|w| w.rsplit('/').next());
    if !matches!(program, Some("icx" | "icx-hook")) {
        return false;
    }
    let value_of = |flag: &str| {
        words
            .windows(2)
            .find(|p| p[0] == flag)
            .map(|p| p[1])
            .or_else(|| {
                words
                    .iter()
                    .find_map(|w| w.strip_prefix(&format!("{flag}=")))
            })
    };
    value_of("--agent") == Some(agent)
        && value_of("--event").is_some_and(|e| e.eq_ignore_ascii_case(event))
}

/// Splits a command line into words, honouring single and double quotes, so a quoted program
/// path with a space stays one word.
fn shell_words(command: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    for c in command.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => cur.push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                in_word = true;
            }
            None if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut cur));
                    in_word = false;
                }
            }
            None => {
                cur.push(c);
                in_word = true;
            }
        }
    }
    if in_word {
        words.push(cur);
    }
    words
}

/// Whether any command string anywhere in `v` is an icx hook for `agent` and `event`.
fn has_icx_hook(v: &J, agent: &str, event: &str) -> bool {
    v.strings()
        .into_iter()
        .any(|c| is_icx_hook_command(c, agent, event))
}

fn hook_cmd(o: &Opts, agent: &str, event: &str) -> String {
    format!("{} hook --agent {agent} --event {event}", o.icx)
}

/// Adds Claude-Code-style hook groups (`hooks.<Event>: [{matcher?, hooks: [{type, command,
/// timeout}]}]`), skipping events that already have an icx hook. `events` pairs each event with
/// its matcher (`None` for events that take none).
fn add_group_hooks(
    root: &mut J,
    o: &Opts,
    agent: &str,
    events: &[(&str, Option<&str>)],
) -> Result<(), String> {
    let hooks = root.obj_mut("hooks")?;
    for (event, matcher) in events {
        if hooks
            .get(event)
            .is_some_and(|v| has_icx_hook(v, agent, event))
        {
            continue;
        }
        let handler = obj(vec![
            ("type", s("command")),
            ("command", s(&hook_cmd(o, agent, event))),
            ("timeout", J::Num(10.into())),
        ]);
        let mut group = Vec::new();
        if let Some(m) = matcher {
            group.push(("matcher", s(m)));
        }
        group.push(("hooks", J::Arr(vec![handler])));
        hooks.arr_mut(event)?.push(obj(group));
    }
    Ok(())
}

/// Adds `<key>.icx = entry` when no `icx` server is configured.
fn add_mcp_server(root: &mut J, key: &str, entry: J) -> Result<(), String> {
    let servers = root.obj_mut(key)?;
    if servers.get("icx").is_none() {
        servers.insert("icx", entry);
    }
    Ok(())
}

/// Adds `rule` to the string array at `path` (dot-separated) when it is not there.
fn add_to_list(root: &mut J, path: &[&str], rule: &str) -> Result<(), String> {
    let (last, parents) = path.split_last().ok_or("empty path")?;
    let mut cur = root;
    for p in parents {
        cur = cur.obj_mut(p)?;
    }
    let list = cur.arr_mut(last)?;
    if !list.iter().any(|v| v.as_str() == Some(rule)) {
        list.push(s(rule));
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Per-agent plans
// ---------------------------------------------------------------------------------------------

/// Builds the install plan for `agent`. Reads files; never writes.
pub fn plan(agent: &str, o: &Opts) -> Result<Plan, String> {
    let mut p = Plan::default();
    match agent {
        "claude-code" => claude(&mut p, o),
        "codex" => codex(&mut p, o),
        "agy" => agy(&mut p, o),
        "muse" => muse(&mut p, o),
        "gemini-cli" => gemini(&mut p, o),
        "cursor" => cursor(&mut p, o),
        "opencode" => opencode(&mut p, o),
        "hermes" => hermes(&mut p, o),
        other => {
            return Err(format!(
                "unknown agent `{other}`; expected one of {} or all",
                AGENTS.join(", ")
            ));
        }
    }
    Ok(p)
}

fn claude(p: &mut Plan, o: &Opts) {
    let path = o.claude_home.join("settings.json");
    let events: &[(&str, Option<&str>)] = &[
        ("SessionStart", Some("")),
        ("UserPromptSubmit", Some("")),
        // Bulk comes from shell, web and MCP output; icx's own tools are left alone.
        (
            "PostToolUse",
            Some("^(Bash|BashOutput|WebFetch|WebSearch|mcp__(?!icx__).+)$"),
        ),
        ("SubagentStart", Some("")),
        ("SubagentStop", Some("")),
        ("PreCompact", Some("")),
        ("PostCompact", Some("")),
        ("SessionEnd", Some("")),
        ("CwdChanged", None),
        ("PostModelSwitch", None),
    ];
    edit_json(
        p,
        &path,
        "Add the hooks from the deployment guide section 2.2 by hand.",
        |v| {
            add_group_hooks(v, o, "claude-code", events)?;
            // The read-only tool only: `icx_write` still asks. A rule allowing the whole icx
            // server already covers it.
            let allowed = v
                .get("permissions")
                .and_then(|p| p.get("allow"))
                .is_some_and(|a| a.strings().contains(&"mcp__icx"));
            if o.approve && !allowed {
                add_to_list(v, &["permissions", "allow"], "mcp__icx__icx")?;
            }
            Ok(())
        },
    );
    p.note(format!(
        "Claude Code keeps MCP servers in ~/.claude.json, which icx does not edit. Register icx with: claude mcp add --scope user icx -- {} mcp-serve",
        o.icx
    ));
}

fn codex(p: &mut Plan, o: &Opts) {
    let hooks = o.codex_home.join("hooks.json");
    let events: Vec<(&str, Option<&str>)> = [
        "SessionStart",
        "UserPromptSubmit",
        "PostToolUse",
        "SubagentStart",
        "SubagentStop",
        "PreCompact",
        "PostCompact",
        "SessionEnd",
    ]
    .iter()
    .map(|e| (*e, Some("")))
    .collect();
    edit_json(
        p,
        &hooks,
        "Add the hooks from the deployment guide section 2.3 by hand.",
        |v| add_group_hooks(v, o, "codex", &events),
    );

    let config = o.codex_home.join("config.toml");
    let before = p.current(&config);
    let mut doc: toml_edit::DocumentMut = match before.as_deref().unwrap_or("").parse() {
        Ok(d) => d,
        Err(e) => {
            p.note(format!(
                "not edited: {} does not parse ({e}). Add [mcp_servers.icx] by hand.",
                config.display()
            ));
            return;
        }
    };
    let servers = doc.entry("mcp_servers").or_insert_with(|| {
        let mut t = toml_edit::Table::new();
        t.set_implicit(true);
        toml_edit::Item::Table(t)
    });
    let Some(servers) = servers.as_table_mut() else {
        p.note(format!(
            "not edited: `mcp_servers` in {} is not a table.",
            config.display()
        ));
        return;
    };
    if !servers.contains_key("icx") {
        let mut t = toml_edit::Table::new();
        t["command"] = toml_edit::value(o.icx.as_str());
        let mut args = toml_edit::Array::new();
        args.push("mcp-serve");
        t["args"] = toml_edit::value(args);
        servers.insert("icx", toml_edit::Item::Table(t));
    }
    if o.approve {
        if let Some(icx) = servers
            .get_mut("icx")
            .and_then(toml_edit::Item::as_table_mut)
        {
            if icx
                .get("default_tools_approval_mode")
                .and_then(|v| v.as_str())
                == Some("approve")
            {
                p.notes.push(
                    "Codex: [mcp_servers.icx] default_tools_approval_mode = \"approve\" also approves icx_write; per-tool approval of the read-only tool is narrower.".to_owned(),
                );
            }
            let tools = icx.entry("tools").or_insert_with(|| {
                let mut t = toml_edit::Table::new();
                t.set_implicit(true);
                toml_edit::Item::Table(t)
            });
            if let Some(tools) = tools.as_table_mut() {
                let tool = tools
                    .entry("icx")
                    .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()));
                if let Some(tool) = tool.as_table_mut()
                    && !tool.contains_key("approval_mode")
                {
                    tool["approval_mode"] = toml_edit::value("approve");
                }
            }
        }
    }
    let disk = fs::read_to_string(&config).ok();
    p.set(&config, disk, doc.to_string());
}

fn agy(p: &mut Plan, o: &Opts) {
    let gemini = o.home.join(".gemini");
    let hooks = gemini.join("config/hooks.json");
    edit_json(
        p,
        &hooks,
        "Add the hooks from the deployment guide section 2.4 by hand.",
        |v| {
            // agy groups hooks by name; the tool event takes `matcher` + `hooks`, the others a
            // list of handlers. `icx hook --agent agy` finds the project from the payload, so no
            // wrapper is needed.
            let mut group = Vec::new();
            for event in [
                "SessionStart",
                "PreInvocation",
                "PostInvocation",
                "PostToolUse",
                "Stop",
            ] {
                if has_icx_hook(v, "agy", event) {
                    continue;
                }
                let handler = obj(vec![
                    ("type", s("command")),
                    ("command", s(&hook_cmd(o, "agy", event))),
                    ("timeout", J::Num(10.into())),
                ]);
                let entry = if event == "PostToolUse" {
                    J::Arr(vec![obj(vec![
                        ("matcher", s("")),
                        ("hooks", J::Arr(vec![handler])),
                    ])])
                } else {
                    J::Arr(vec![handler])
                };
                group.push((event, entry));
            }
            if !group.is_empty() {
                let g = v.obj_mut("infinity-context")?;
                for (event, entry) in group {
                    if g.get(event).is_none() {
                        g.insert(event, entry);
                    }
                }
            }
            Ok(())
        },
    );
    edit_json(
        p,
        &gemini.join("config/mcp_config.json"),
        "Add icx under mcpServers by hand.",
        |v| {
            add_mcp_server(
                v,
                "mcpServers",
                obj(vec![
                    ("command", s(&o.icx)),
                    ("args", j(serde_json::json!(["mcp-serve"]))),
                ]),
            )
        },
    );
    if o.approve {
        edit_json(
            p,
            &gemini.join("antigravity-cli/settings.json"),
            "Add \"mcp(icx/icx)\" to permissions.allow by hand.",
            |v| add_to_list(v, &["permissions", "allow"], "mcp(icx/icx)"),
        );
    }
}

fn muse(p: &mut Plan, o: &Opts) {
    let path = o.config_home.join("muse/settings.json");
    let events: Vec<(&str, Option<&str>)> = [
        "SessionStart",
        "UserPromptSubmit",
        "PostToolUse",
        "PostToolUseFailure",
        "SubagentStart",
        "SubagentStop",
        "PreCompact",
        "PostCompact",
        "SessionEnd",
    ]
    .iter()
    .map(|e| (*e, Some("")))
    .collect();
    edit_json(
        p,
        &path,
        "Add the hooks and the icx MCP server from the deployment guide section 2.6 by hand.",
        |v| {
            add_group_hooks(v, o, "muse", &events)?;
            add_mcp_server(
                v,
                "mcpServers",
                obj(vec![
                    ("type", s("stdio")),
                    ("command", s(&o.icx)),
                    ("args", j(serde_json::json!(["mcp-serve"]))),
                ]),
            )
        },
    );
    if o.approve {
        p.note("Muse has no pre-approval key; it runs a tool its server marks read-only without a prompt, which covers icx's read-only tool.");
    }
    p.note("Muse loads MCP servers only in a trusted workspace; trust is per workspace and icx never writes Muse's trust file.");
}

fn gemini(p: &mut Plan, o: &Opts) {
    let path = o.home.join(".gemini/settings.json");
    let events: Vec<(&str, Option<&str>)> = [
        "SessionStart",
        "BeforeAgent",
        "AfterTool",
        "PreCompress",
        "SessionEnd",
    ]
    .iter()
    .map(|e| (*e, Some("")))
    .collect();
    edit_json(
        p,
        &path,
        "Add the hooks and the icx MCP server from the deployment guide section 2.7 by hand.",
        |v| {
            add_group_hooks(v, o, "gemini-cli", &events)?;
            add_mcp_server(
                v,
                "mcpServers",
                obj(vec![
                    ("command", s(&o.icx)),
                    ("args", j(serde_json::json!(["mcp-serve"]))),
                ]),
            )
        },
    );
    if o.approve {
        p.note("Gemini CLI approves a whole MCP server at a time (`trust`), which would include icx_write, so icx does not pre-approve it.");
    }
}

fn cursor(p: &mut Plan, o: &Opts) {
    let dir = o.home.join(".cursor");
    edit_json(
        p,
        &dir.join("hooks.json"),
        "Add the hooks from the deployment guide section 2.8 by hand.",
        |v| {
            if v.get("version").is_none() {
                v.insert("version", J::Num(1.into()));
            }
            let hooks = v.obj_mut("hooks")?;
            for event in [
                "postToolUse",
                "postToolUseFailure",
                "preCompact",
                "sessionEnd",
            ] {
                if hooks
                    .get(event)
                    .is_some_and(|h| has_icx_hook(h, "cursor", event))
                {
                    continue;
                }
                hooks.arr_mut(event)?.push(obj(vec![
                    ("type", s("command")),
                    ("command", s(&hook_cmd(o, "cursor", event))),
                ]));
            }
            Ok(())
        },
    );
    edit_json(
        p,
        &dir.join("mcp.json"),
        "Add icx under mcpServers by hand.",
        |v| {
            add_mcp_server(
                v,
                "mcpServers",
                obj(vec![
                    ("command", s(&o.icx)),
                    ("args", j(serde_json::json!(["mcp-serve"]))),
                ]),
            )
        },
    );
    if o.approve {
        edit_json(
            p,
            &dir.join("cli-config.json"),
            "Add \"Mcp(icx:icx)\" to permissions.allow by hand.",
            |v| add_to_list(v, &["permissions", "allow"], "Mcp(icx:icx)"),
        );
    }
}

/// The OpenCode plugin source, shipped in the binary.
const OPENCODE_PLUGIN: &str = include_str!("../../../integrations/opencode/icx.ts");

fn opencode(p: &mut Plan, o: &Opts) {
    let dir = o.config_home.join("opencode");
    let plugin = dir.join("plugins/icx.ts");
    let before = fs::read_to_string(&plugin).ok();
    p.set(&plugin, before, OPENCODE_PLUGIN.to_owned());
    edit_json(
        p,
        &dir.join("opencode.json"),
        "Add icx under mcp.servers by hand.",
        |v| {
            let mcp = v.obj_mut("mcp")?;
            add_mcp_server(
                mcp,
                "servers",
                obj(vec![
                    ("type", s("local")),
                    ("command", j(serde_json::json!([o.icx, "mcp-serve"]))),
                ]),
            )
        },
    );
    if o.icx != "icx" {
        p.note(format!(
            "The OpenCode plugin runs `icx` from PATH; set ICX_BIN={} in OpenCode's environment if it is not on PATH.",
            o.icx
        ));
    }
    if o.approve {
        p.note("OpenCode tool pre-approval is not written: its permission format is not yet verified on the installed version.");
    }
}

fn hermes(p: &mut Plan, o: &Opts) {
    crate::install_hermes::plan(p, o);
}

// ---------------------------------------------------------------------------------------------
// Rendering and applying
// ---------------------------------------------------------------------------------------------

/// The plan as unified diffs, one per file, followed by its notes.
#[must_use]
pub fn render(plan: &Plan) -> String {
    let mut out = String::new();
    for e in &plan.edits {
        let before = e.before.as_deref().unwrap_or("");
        let label_a = if e.before.is_some() {
            e.path.display().to_string()
        } else {
            "/dev/null".to_owned()
        };
        let diff = similar::TextDiff::from_lines(before, &e.after);
        out.push_str(
            &diff
                .unified_diff()
                .context_radius(3)
                .header(&label_a, &e.path.display().to_string())
                .to_string(),
        );
    }
    for n in &plan.notes {
        out.push_str(&format!("note: {n}\n"));
    }
    out
}

/// Writes every edit. A file that existed is first copied to `<file>.icx-backup-<stamp>`, then
/// replaced through a temporary file and a rename. The temporary file and the backup are created
/// new (`O_EXCL`, so an existing path or a planted symlink is refused) with the original file's
/// mode, or `0600` for a new file, from the start, so a config holding tokens is never readable
/// by others, even briefly. A config that is a symlink (a dotfiles link) is written at its target,
/// so the link stays a link. Returns the backups made.
pub fn apply(plan: &Plan, stamp: u64) -> Result<Vec<PathBuf>, String> {
    let mut backups = Vec::new();
    for e in &plan.edits {
        // The file to write: the target of a symlinked config, or the path itself.
        let target = fs::canonicalize(&e.path).unwrap_or_else(|_| e.path.clone());
        let parent = target
            .parent()
            .ok_or_else(|| format!("{} has no parent directory", target.display()))?;
        fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
        let name = target
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mode = file_mode(&target);
        if target.exists() {
            let backup = target.with_file_name(format!("{name}.icx-backup-{stamp}"));
            let contents =
                fs::read(&target).map_err(|err| format!("reading {}: {err}", target.display()))?;
            write_new(&backup, &contents, mode).map_err(|err| {
                format!(
                    "backing up {} to {}: {err}",
                    target.display(),
                    backup.display()
                )
            })?;
            backups.push(backup);
        }
        let tmp = target.with_file_name(format!(".{name}.icx-tmp-{}", std::process::id()));
        let written =
            write_new(&tmp, e.after.as_bytes(), mode).and_then(|()| fs::rename(&tmp, &target));
        if let Err(err) = written {
            let _ = fs::remove_file(&tmp);
            return Err(format!("writing {}: {err}", target.display()));
        }
    }
    Ok(backups)
}

/// The permission bits of `path`, or `0o600` when it does not exist yet.
fn file_mode(path: &Path) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path).map_or(0o600, |m| m.permissions().mode() & 0o7777)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        0o600
    }
}

/// Creates `path`, which must not exist (a symlink there is refused, not followed), with `mode`
/// from the start, and writes `contents` to it.
fn write_new(path: &Path, contents: &[u8], mode: u32) -> std::io::Result<()> {
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(mode);
    }
    #[cfg(not(unix))]
    let _ = mode;
    let mut f = opts.open(path)?;
    f.write_all(contents)?;
    f.sync_all()?;
    // The umask may have cleared bits the original had; set the mode exactly.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_keeps_key_order_and_unknown_keys() {
        let text = r#"{"zeta": 1, "alpha": {"b": 2, "a": [1, 2.5, null, true]}, "x": "y"}"#;
        let v: J = serde_json::from_str(text).unwrap();
        let back: serde_json::Value = serde_json::from_str(&json_text(&v)).unwrap();
        assert_eq!(
            back,
            serde_json::from_str::<serde_json::Value>(text).unwrap()
        );
        let out = json_text(&v);
        assert!(out.find("zeta").unwrap() < out.find("alpha").unwrap());
        assert!(out.find("\"b\"").unwrap() < out.find("\"a\"").unwrap());
    }

    #[test]
    fn icx_hook_commands_are_recognised_in_any_form() {
        for c in [
            "icx hook --agent codex --event PostToolUse",
            "/home/u/.local/bin/icx-hook --agent codex --event PostToolUse",
            "\"/opt/x y/icx\" hook --event posttooluse --agent codex",
            "icx hook --agent=codex --event=PostToolUse",
        ] {
            assert!(is_icx_hook_command(c, "codex", "PostToolUse"), "{c}");
        }
        for c in [
            "icx hook --agent codex --event SessionStart",
            "icx hook --agent codex-x --event PostToolUse",
            "ai-memory hook --agent codex --event PostToolUse",
            "not-icx hook --agent codex --event PostToolUse",
        ] {
            assert!(!is_icx_hook_command(c, "codex", "PostToolUse"), "{c}");
        }
    }
}
