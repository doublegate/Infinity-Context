//! T-65-006: the capability matrix in `docs/capability-matrix.md` is generated here, by running
//! every adapter, and this test fails when the committed file differs from what the adapters do.
//!
//! Regenerate after a behaviour change:
//! `ICX_UPDATE_CAPABILITY_MATRIX=1 cargo test -p icx-adapters --test capability_matrix`.
//!
//! Computed per agent: whether a large tool result is ingested, whether it is replaced by a stub
//! (by default, only with the agent's opt-in flag, or never), whether a context event injects
//! icx context, and whether compaction and session end seal the cache epoch. The verification
//! column is fixed per agent and cross-checked against the live fixture directories.

use icx_adapters::{
    AntigravityAdapter, ClaudeCodeAdapter, CodexAdapter, CursorAdapter, GeminiCliAdapter,
    HermesAdapter, InfinityContextMmu, MuseAdapter, OpenCodeAdapter,
};
use icx_core::AgentKind;
use serde_json::{Value, json};
use std::path::Path;

type Handler = fn(&str, &str, &mut InfinityContextMmu) -> Result<String, serde_json::Error>;
type StubHandler =
    fn(&str, &str, &mut InfinityContextMmu, bool) -> Result<String, serde_json::Error>;

struct Agent {
    name: &'static str,
    kind: AgentKind,
    handler: Handler,
    /// The adapter's entry point with the opt-in stub flag, and the flag's name.
    opt_in: Option<(StubHandler, &'static str)>,
    tool: (&'static str, Value),
    context: (&'static str, Value),
    /// Where the context goes.
    context_field: &'static str,
    compaction: Option<(&'static str, Value)>,
    session_end: Option<(&'static str, Value)>,
    /// How the adapter is verified, and the fixture directory a live claim rests on.
    verified: &'static str,
    fixtures: Option<&'static str>,
    /// What pre-approves the read-only `icx` tool (`icx install --approve-tools`).
    approval: &'static str,
    /// Qualifies a computed "yes" in the shrink column, when the route is itself opt-in.
    shrink_route: Option<&'static str>,
}

fn big() -> String {
    (1..=3000).map(|i| i.to_string() + "\n").collect()
}

fn fixture(path: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(Path::new("tests/fixtures").join(path)).unwrap())
        .unwrap()
}

fn agents() -> Vec<Agent> {
    let mut codex_tool = fixture("codex/0.171.0/PostToolUse.json");
    codex_tool["tool_response"] = json!(big());
    let mut muse_tool = fixture("muse/1.4.25/PostToolUse-3.json");
    muse_tool["tool_name"] = json!("Bash");
    muse_tool["tool_response"] = json!(big());
    vec![
        Agent {
            name: "Claude Code",
            kind: AgentKind::ClaudeCode,
            handler: ClaudeCodeAdapter::handle_hook,
            opt_in: None,
            tool: (
                "PostToolUse",
                json!({"tool_name": "Bash", "tool_response": {"stdout": big(), "stderr": ""}}),
            ),
            context: ("SessionStart", json!({"session_id": "s"})),
            context_field: "`additionalContext`",
            compaction: Some(("PreCompact", json!({}))),
            session_end: Some(("SessionEnd", json!({}))),
            verified: "live, 2.1.330 (`claude_builtin_shapes.rs`, `scripts/live-claude-replacement-check.sh`)",
            fixtures: None,
            shrink_route: None,
            approval: "`mcp__icx__icx` in `permissions.allow`",
        },
        Agent {
            name: "Codex CLI",
            kind: AgentKind::Codex,
            handler: CodexAdapter::handle_hook,
            opt_in: None,
            tool: ("PostToolUse", codex_tool),
            context: ("SessionStart", fixture("codex/0.171.0/SessionStart.json")),
            context_field: "`additionalContext`",
            compaction: Some(("PreCompact", json!({}))),
            session_end: Some(("SessionEnd", json!({}))),
            verified: "live fixtures, 0.171.0",
            fixtures: Some("codex/0.171.0"),
            shrink_route: None,
            approval: "`tools.icx.approval_mode = \"approve\"`",
        },
        Agent {
            name: "agy",
            kind: AgentKind::AntigravityCli,
            handler: AntigravityAdapter::handle_hook,
            opt_in: None,
            tool: ("PostToolUse", fixture("agy/1.5.14/PostToolUse.json")),
            context: ("PreInvocation", fixture("agy/1.5.14/PreInvocation.json")),
            context_field: "`injectSteps` (visible to the user)",
            compaction: None,
            // agy fires `Stop`, not `SessionEnd`; an ordinary stop does not seal (review of #91).
            session_end: Some(("Stop", fixture("agy/1.5.14/Stop.json"))),
            verified: "live fixtures, 1.5.14 (PostToolUse carries no result; `overwriteResult` ignored; `Stop` seals only on `max_steps_exceeded` or an error)",
            fixtures: Some("agy/1.5.14"),
            shrink_route: None,
            approval: "`mcp(icx/icx)` in `permissions.allow`",
        },
        Agent {
            name: "Muse Code",
            kind: AgentKind::Muse,
            handler: MuseAdapter::handle_hook,
            opt_in: None,
            tool: ("PostToolUse", muse_tool),
            context: ("SessionStart", fixture("muse/1.4.25/SessionStart.json")),
            context_field: "`additionalContext`",
            compaction: Some(("PreCompact", json!({}))),
            session_end: Some(("SessionEnd", json!({}))),
            verified: "live fixtures, 1.4.25",
            fixtures: Some("muse/1.4.25"),
            shrink_route: None,
            approval: "none needed: `readOnlyHint` runs without a prompt",
        },
        Agent {
            name: "OpenCode",
            kind: AgentKind::OpenCode,
            handler: OpenCodeAdapter::handle_hook,
            opt_in: Some((
                OpenCodeAdapter::handle_hook_with_stub_mode,
                "ICX_OPENCODE_STUB",
            )),
            tool: (
                "execute.after",
                json!({"tool": "bash", "status": "completed", "output": big()}),
            ),
            context: (
                "prompt",
                json!({"sessionID": "s", "prompt": "hi", "legend": true}),
            ),
            context_field: "appended to the user turn",
            compaction: Some(("compaction", json!({"sessionID": "s"}))),
            session_end: Some(("session.end", json!({"sessionID": "s"}))),
            verified: "live, v2.2.7 (Sprint 3)",
            fixtures: None,
            shrink_route: None,
            approval: "not written (format unverified on the installed version)",
        },
        Agent {
            name: "Hermes",
            kind: AgentKind::Hermes,
            handler: HermesAdapter::handle_hook,
            opt_in: None,
            tool: (
                "transform_terminal_output",
                json!({"command": "seq 1 3000", "output": big(), "returncode": 0}),
            ),
            context: ("SessionStart", json!({})),
            context_field: "user turn, via `select_context` (plugin `icx`)",
            // The shipped `icx` engine checkpoints through `on_pre_compress` (review of #91).
            compaction: Some((
                "on_pre_compress",
                json!({"messages": [{"role": "user", "content": "hi"}]}),
            )),
            session_end: None,
            verified: "documented contract, stand-in tests; live deferred (#89)",
            fixtures: None,
            shrink_route: Some("plugin `icx-terminal`; live check deferred, #89"),
            approval: "MCP is optional; no rule written",
        },
        Agent {
            name: "Gemini CLI",
            kind: AgentKind::GeminiCli,
            handler: GeminiCliAdapter::handle_hook,
            opt_in: Some((
                GeminiCliAdapter::handle_hook_with_stub_mode,
                "ICX_GEMINI_STUB",
            )),
            tool: (
                "AfterTool",
                json!({"tool_name": "run_shell_command", "tool_response": {"llmContent": big()}}),
            ),
            context: ("SessionStart", json!({})),
            context_field: "`additionalContext`",
            compaction: Some(("PreCompress", json!({}))),
            session_end: Some(("SessionEnd", json!({}))),
            verified: "documented contract; live deferred (#89)",
            fixtures: None,
            shrink_route: None,
            approval: "none: approval is per server and would include `icx_write`",
        },
        Agent {
            name: "Cursor",
            kind: AgentKind::Cursor,
            handler: CursorAdapter::handle_hook,
            opt_in: Some((CursorAdapter::handle_hook_with_stub_mode, "ICX_CURSOR_STUB")),
            tool: (
                "postToolUse",
                json!({"tool_name": "MCP:svc", "tool_output":
                    json!({"content": [{"type": "text", "text": big()}]}).to_string()}),
            ),
            context: ("sessionStart", json!({})),
            context_field: "`additional_context`",
            compaction: Some(("preCompact", json!({}))),
            session_end: Some(("sessionEnd", json!({}))),
            verified: "session events live (2026.09.08); tool calls documented contract, live deferred (#89)",
            fixtures: None,
            shrink_route: None,
            approval: "`Mcp(icx:icx)` in `permissions.allow`",
        },
    ]
}

fn reply(h: Handler, event: &str, payload: &Value, mmu: &mut InfinityContextMmu) -> String {
    h(event, &payload.to_string(), mmu).unwrap_or_default()
}

fn seals(a: &Agent, event: &str, payload: &Value) -> bool {
    let mut mmu = InfinityContextMmu::default();
    mmu.ingest_payload(a.kind, "work", "a small amount of work\n", false);
    let before = mmu.epochs.sealed_epoch_count();
    let _ = reply(a.handler, event, payload, &mut mmu);
    mmu.epochs.sealed_epoch_count() > before
}

fn yes_no(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}

fn row(a: &Agent) -> String {
    let (event, payload) = &a.tool;
    let mut mmu = InfinityContextMmu::default();
    let before = mmu.cas.artifact_count();
    let plain = reply(a.handler, event, payload, &mut mmu);
    let ingests = mmu.cas.artifact_count() > before;
    let shrinks = if plain.contains("ICX:STUB") {
        a.shrink_route
            .map_or_else(|| "yes".to_owned(), |r| format!("opt-in ({r})"))
    } else if let Some((h, flag)) = a.opt_in {
        let mut mmu = InfinityContextMmu::default();
        let on = h(event, &payload.to_string(), &mut mmu, true).unwrap_or_default();
        if on.contains("ICX:STUB") {
            format!("opt-in (`{flag}=1`)")
        } else {
            "no".to_owned()
        }
    } else {
        "no".to_owned()
    };
    let (cevent, cpayload) = &a.context;
    let mut mmu = InfinityContextMmu::default();
    let injects = reply(a.handler, cevent, cpayload, &mut mmu).contains("ICX:");
    let context = if injects {
        format!("yes, {}", a.context_field)
    } else {
        "no".to_owned()
    };
    let compaction = a
        .compaction
        .as_ref()
        .map_or("no event".to_owned(), |(e, p)| {
            format!("{} (`{e}`)", yes_no(seals(a, e, p)))
        });
    let end = a
        .session_end
        .as_ref()
        .map_or("no event".to_owned(), |(e, p)| {
            format!("{} (`{e}`)", yes_no(seals(a, e, p)))
        });
    format!(
        "| {} | {} | {} | {} | {} | {} | {} | {} |",
        a.name,
        yes_no(ingests),
        shrinks,
        context,
        compaction,
        end,
        a.verified,
        a.approval
    )
}

fn generate() -> String {
    let mut out = String::from(
        "<!-- Generated by crates/icx-adapters/tests/capability_matrix.rs; do not edit by hand.\n     \
         Regenerate: ICX_UPDATE_CAPABILITY_MATRIX=1 cargo test -p icx-adapters --test capability_matrix -->\n\n\
         # Agent Capability Matrix\n\n\
         What icx does with each agent, computed by running each adapter: a large tool result \
         (3,000 lines), a context event, compaction and session end. \"Shrinks\" means the \
         agent receives an icx stub in place of the result; \"opt-in\" means only with the \
         named environment variable set for the icx hook. The verification column states how \
         the agent's side of the contract was confirmed; agents without a live run follow \
         their documented contract (deferred to v1.5.0, #89).\n\n\
         | Agent | Ingests tool output | Shrinks tool output | Injects context | Seals at compaction | Seals at session end | Verified | Pre-approval of `icx` |\n\
         | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |\n",
    );
    for a in agents() {
        out.push_str(&row(&a));
        out.push('\n');
    }
    out.push_str(
        "\nThe one \"yes\" in the shrink column, Claude Code, is backed by a test that fails \
         when the agent ignores the replacement (`scripts/live-claude-replacement-check.sh`, run \
         live by hand). Of the opt-in routes, OpenCode's was checked live in Sprint 3; Hermes, \
         Gemini CLI and Cursor follow their documented contracts until the live checks in #89. \
         Codex, agy and Muse were checked live and ignore or refuse replacement, so icx only \
         ingests there.\n",
    );
    out
}

#[test]
fn the_committed_matrix_matches_the_adapters() {
    // A "live fixtures" claim rests on a fixture directory that exists.
    for a in agents() {
        if let Some(dir) = a.fixtures {
            assert!(
                Path::new("tests/fixtures").join(dir).is_dir(),
                "{}: missing {dir}",
                a.name
            );
        }
    }
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/capability-matrix.md");
    let generated = generate();
    if std::env::var_os("ICX_UPDATE_CAPABILITY_MATRIX").is_some() {
        std::fs::write(&path, &generated).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == generated,
        "docs/capability-matrix.md is out of date; regenerate it with \
         ICX_UPDATE_CAPABILITY_MATRIX=1 cargo test -p icx-adapters --test capability_matrix\n\
         generated:\n{generated}"
    );
}
