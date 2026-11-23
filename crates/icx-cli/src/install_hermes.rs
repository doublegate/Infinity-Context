//! `icx install --agent hermes` (T-65-004): installs the Hermes plugins shipped in
//! `integrations/hermes/`, the `icx` context engine (T-65-002) and the optional `icx-terminal`
//! shell-output plugin (T-65-009), into `$HERMES_HOME/plugins/` (default `~/.hermes/plugins/`).
//!
//! Hermes keeps its settings in YAML, which icx does not rewrite (it edits only formats it can
//! change without losing comments), so the two `config.yaml` lines that turn the plugins on are
//! printed instead.

use crate::install::{Opts, Plan};
use std::path::PathBuf;

const ENGINE_INIT: &str = include_str!("../../../integrations/hermes/icx/__init__.py");
const ENGINE_MANIFEST: &str = include_str!("../../../integrations/hermes/icx/plugin.yaml");
const TERMINAL_INIT: &str = include_str!("../../../integrations/hermes/icx-terminal/__init__.py");
const TERMINAL_MANIFEST: &str =
    include_str!("../../../integrations/hermes/icx-terminal/plugin.yaml");

/// Hermes's home: `$HERMES_HOME`, or `~/.hermes`.
pub(crate) fn hermes_home(o: &Opts) -> PathBuf {
    std::env::var_os("HERMES_HOME")
        .filter(|v| !v.is_empty())
        .map_or_else(|| o.home.join(".hermes"), PathBuf::from)
}

pub(crate) fn plan(p: &mut Plan, o: &Opts) {
    let plugins = hermes_home(o).join("plugins");
    for (dir, name, text) in [
        ("icx", "__init__.py", ENGINE_INIT),
        ("icx", "plugin.yaml", ENGINE_MANIFEST),
        ("icx-terminal", "__init__.py", TERMINAL_INIT),
        ("icx-terminal", "plugin.yaml", TERMINAL_MANIFEST),
    ] {
        let path = plugins.join(dir).join(name);
        match crate::install::read_existing(&path) {
            Ok(before) => p.set_file(&path, before, text.to_owned()),
            Err(e) => p.notes.push(format!(
                "not edited: {e}. Copy integrations/hermes/{dir} by hand."
            )),
        }
    }
    p.notes.push(
        "Hermes: select the engine in config.yaml with `context:` / `engine: icx`; the optional \
         shell-output stubbing also needs `plugins:` / `enabled: [icx-terminal]`. icx does not \
         edit Hermes's YAML. Restart Hermes to load the plugins."
            .to_owned(),
    );
    if o.icx != "icx" {
        p.notes.push(format!(
            "Hermes: the plugins run `icx` from PATH or ~/.local/bin; set ICX_BIN={} in Hermes's \
             environment if neither has it.",
            o.icx
        ));
    }
}
