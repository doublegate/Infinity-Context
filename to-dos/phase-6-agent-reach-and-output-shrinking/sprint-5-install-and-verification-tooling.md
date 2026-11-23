# Sprint 5 — Install and Verification Tooling

**Phase:** Phase 6 — Agent Reach and Output Shrinking

**Release:** v1.4.0

**Sprint Goal:** Prove each adapter against the installed agent with live-run fixtures, ship
the Hermes shim, and give users an install command that shows every config edit before it
makes one.

**Estimated Duration:** 2 weeks

## Research gate

Re-check before the sprint starts:

- **External docs and versions:** the config schema of each agent: Claude Code
  `settings.json`, Codex `~/.codex/config.toml` (`[mcp_servers.<name>]` keys), agy
  `~/.gemini/antigravity-cli/settings.json`, Muse settings, and the Hermes plugin interface.
- **Installed agent behaviour:** the installed version of each agent; that Codex 0.171.0 honours
  `default_tools_approval_mode = "approve"` under `[mcp_servers.icx]`; that agy honours
  `"mcp(icx/*)"` in `permissions.allow`; Muse's policy-plane key (T-61-004).
- **Project evidence:** the adapters doc statement that only Claude Code is pinned by live
  fixtures (`docs/agent-adapters-and-interception.md` line 13); ICX-AUD-006; the current
  `icx init` behaviour and backups.

### Research notes

**2026-11-22, from Sprint 1** (`ref-docs/agent-internals/below-hook-findings-2026-11-22.md`, decision
table):

- **Approvals:** Codex `default_tools_approval_mode = "approve"` and agy `"mcp(icx/*)"` both
  verified live. Muse has no pre-approval settings key; its documented route is the tool's
  `readOnlyHint` (T-64-007).
- **Muse trust:** trust is per workspace root, in a user-level file with an undocumented format.
  The installer must print guidance and never write that file.
- **agy:**
  - `metadata.user_markdown: ""` hides nothing, so injected steps are visible to the user, and
    the adapter's "invisible" claim is wrong (T-65-003).
  - A PostToolUse `overwriteResult` may replace the result; whether that works for command hooks
    is unverified (T-65-007).
  - MCP tools reach hook matchers as `call_mcp_tool` or `mcp_<server>_<tool>` (T-65-008).
- **Hermes (`caafe5389ae`):**
  - the current hand-installed plugin has four defects (T-65-002);
  - `transform_terminal_output` is the only shell-output replacement seam, and only a general
    plugin can use it (T-65-009).

## Tickets

### T-65-001 — Contract fixtures from live runs for Codex, agy and Muse

**Description:** Capture hook input and expected output for every handled event from live runs
of each agent, and replace hand-written test inputs with these fixtures.

**Research:** Record each agent version at capture time. Redact the captures before commit.

**Acceptance Criteria:**

**Live captures (2026-11-23).** Each agent ran in a scratch project with a capture hook that
recorded every payload before forwarding it to icx, and with scratch `ICX_*` state, so the
owner's global icx hooks also wrote only to the scratch store.

- **Codex 0.171.0:** hooks were passed inline (`-c hooks.<Event>=...`, with the documented
  `--dangerously-bypass-hook-trust`), because Codex ignores an untrusted project's
  `.codex/hooks.json`. Codex ran its own memory-consolidation task in the same process; those
  events and their output, the owner's private memories, were excluded by session and deleted.
- **agy 1.5.14:** project `.agents/hooks.json`, narrow `--settings` permissions
  (`mcp(icx/*)`). `--dangerously-skip-permissions` was refused by the session's safety classifier
  and is not used.
- **Muse 1.4.25:** project `.muse/hooks.json` with `--trust-workspace` (this run only). Muse
  refuses to override a configured server name per run, so the new build ran as server `icxtest`.

- [x] One fixture set per agent under `crates/icx-adapters/tests/fixtures/<agent>/<version>/`
  for Codex, agy and Muse. Hermes is deferred to v1.5.0 (#89): the gateway stays stopped.
- [x] Contract tests run against the fixtures (`codex_fixtures.rs`, `agy_fixtures.rs`,
  `muse_fixtures.rs`); gitleaks clean.
- [x] A script turns captures into fixtures for a new agent version:
  `scripts/sanitize-hook-fixtures.py` (paths, UUIDs and e-mail addresses replaced; fails if a real
  path survives). The capture setup per agent is recorded above.
- [x] Adapters doc updated to state which agents are pinned by live fixtures.

**Dependencies:** T-62-002, T-63-004, T-64-001

**Reference:** `docs/agent-adapters-and-interception.md` line 13

**Estimated Complexity:** L

---

### T-65-002 — Ship the Hermes `ContextEngine` shim

**Description:** Ship a Hermes plugin that implements the `ContextEngine` interface found in
T-61-005 and calls icx, and verify it in a live Hermes session.

**Research:** Hypothesis: the shim loads on the installed Hermes and its context reaches the
model. The live session that would confirm it is deferred to v1.5.0 (#89); v1.4.0 tests the shim
against the documented `ContextEngine` interface.

**Acceptance Criteria:**

- [ ] Shim source in the repository, pinned to a Hermes commit range.
- [ ] Fails open: if icx is unavailable, Hermes runs unchanged.
- [ ] The four defects found in the hand-installed plugin (Sprint 1) are fixed:
  - the manifest uses a valid kind (`exclusive`, or no kind field) instead of
    `context_engine`;
  - `on_turn_complete` sends only the delta since the last ingested turn, on one bounded
    worker instead of one thread per turn;
  - `on_session_start` does not block Hermes for up to 10 s at init and after each compaction;
  - the user's `compression:` settings are read rather than silently ignored.
- [ ] Verified by stand-in tests that drive the shim through the documented `ContextEngine`
  interface. Live verification and a live fixture are deferred to v1.5.0 (#89): the Hermes
  gateway stays stopped (owner decision, 2026-11-23).
- [ ] Deployment guide Hermes section rewritten; ICX-AUD-006 moved to fixed.

**Dependencies:** T-61-005

**Reference:** ICX-AUD-006 (`docs/claude/findings.csv`, "no Hermes shim ships");
`docs/deployment-and-agent-integration-guide.md` (Hermes section)

**Estimated Complexity:** L

---

### T-65-003 — Verify agy `user_markdown` behaviour

**Description:** Sprint 1 found that `metadata.user_markdown: ""` hides nothing: the terminal UI
never reads it, the web UI treats an empty value as unset, and the model always receives the
text. Correct `AntigravityAdapter` and its docs, which claim the injected frames are invisible,
and test whether `renderInfo.hidden: true` hides an injected step.

**Research:** Hypothesis: `renderInfo.hidden: true` does not hide hook-injected steps in the agy
terminal UI. Sprint 1 rated it unlikely. Confirmed
or rejected by a live session in the terminal UI and the web UI.

**Acceptance Criteria:**

**Outcome (2026-11-23):** the claim is corrected everywhere. `renderInfo.hidden` stays
unemitted: a headless `agy -p` run cannot show what the terminal UI renders, so the UI check is
not done.

- [x] Behaviour of `user_markdown` recorded with the agy version (Sprint 1: hides nothing in
  either UI). `renderInfo.hidden` not tested: it needs an interactive session.
- [x] The "completely invisible" claim removed from the adapter's module docs, the adapters doc,
  the deployment guide and the architecture doc; they state that injected steps are visible.
- [x] `renderInfo.hidden` emitted only if the live test shows it hides the step: not emitted.
- [x] Contract fixture added (`tests/fixtures/agy/1.5.14/`, `tests/agy_fixtures.rs`).

**Dependencies:** T-61-004

**Reference:** Plan Phase 6 Sprint 5; `docs/agent-adapters-and-interception.md` (agy section)

**Estimated Complexity:** S

---

### T-65-004 — `icx install --agent <name> [--dry-run]`

**Description:** Add an install command per agent that registers hooks and the MCP server.
`--dry-run` prints each file and the exact edit (as a diff) and writes nothing. A real install
backs up every file before editing it.

**Research:** Verify each agent's config path and format on the installed version; check that
`icx init` behaviour is kept or folded in.

**Acceptance Criteria:**

**Outcome (2026-11-23):** shipped (`crates/icx-cli/src/install.rs`, `tests/install_cli.rs`).
Config paths and formats were checked on the installed versions (Claude Code, Codex, agy, Muse,
Cursor and OpenCode configs on this machine); Gemini CLI's follow its docs. `icx init` is
unchanged: it creates the workspace, and `icx install` registers the agents. A dry run against
the owner's real configs recognised every existing `icx-hook` registration and planned only the
missing pieces.

- [x] Supported agents: Claude Code, Codex, agy, Muse, Hermes, Gemini CLI, Cursor, OpenCode.
  Hermes installs its plugins once T-65-002 ships them, and prints its one YAML change.
- [x] `--dry-run` output is a diff per file; no file is written in dry-run mode (tested).
- [x] A real install backs up each edited file and is idempotent (second run makes no change).
- [x] Unknown keys and comments in agent configs are preserved (JSON keys and order; TOML
  comments). A file that does not parse, such as JSON with comments, is left alone with a note.

**Dependencies:** T-63-004

**Reference:** Plan Phase 6 Sprint 5; `docs/deployment-and-agent-integration-guide.md`

**Estimated Complexity:** L

---

### T-65-005 — Pre-approve the icx MCP tool per agent

**Description:** Add an install step that pre-approves the icx MCP tool where the agent
supports it, so users do not get repeated prompts:

- Claude Code: `"mcp__icx"` in `permissions.allow` of `~/.claude/settings.json`;
- Codex: `default_tools_approval_mode = "approve"` under `[mcp_servers.icx]` in
  `~/.codex/config.toml` (verified on codex 0.171.0);
- agy: `"mcp(icx/*)"` in `permissions.allow` of `~/.gemini/antigravity-cli/settings.json`;
- Muse: no settings key exists on 1.4.25 (Sprint 1). Muse auto-approves a tool the server
  declares read-only, so Muse is covered by `readOnlyHint` (T-64-007) and needs no config edit.

The step runs in `--dry-run` by default: it prints the edit and never writes another agent's
permission config without explicit user confirmation.

**Research:** Re-verify each key on the installed version before coding. For Muse, re-check
whether a pre-approval key has been documented since 1.4.25. If none has, the step prints the
`readOnlyHint` note for Muse and skips it.

**Acceptance Criteria:**

**Live checks (2026-11-23):** `codex exec` (0.171.0) ran `icx_write` with
`default_tools_approval_mode` set to `writes` and to `prompt` alike, so a non-interactive run
cannot tell approval modes apart. Muse 1.4.25's default LLM approval judge also let `icx_write`
run. Since the ADR-0009 split, the per-tool keys approve only the read-only tool.

- [x] The step prints the exact edit per agent and writes only after an explicit confirmation
  (interactive prompt or `--yes`); a test proves that nothing is written otherwise.
- [x] The approval covers only icx's read-only tool (`mcp__icx__icx`, Codex
  `tools.icx.approval_mode`, `mcp(icx/icx)`, `Mcp(icx:icx)`), never a wildcard.
- [x] Each edited file is backed up; the edit is idempotent.
- [ ] Each key is verified on a live run and recorded with the agent version. Partly: agy's form
  live (Sprint 1); Claude Code's by its documented syntax; Codex's cannot be shown by `codex
  exec`; Cursor's deferred with #89.
- [x] Deployment guide documents the approvals and how to remove them (section 2.11).

**Dependencies:** T-65-004, T-61-004, T-64-007

**Reference:** Plan "Immediate fixes" (Codex, agy, Muse and Hermes approval settings) and
Phase 11 Sprint 5 (install-time permission setup)

**Estimated Complexity:** M

---

### T-65-006 — Capability matrix generated from the contract tests

**Description:** Generate a capability matrix (agent, version, event, ingests, shrinks, injects
context, seals epoch) from the contract tests, and publish it in the adapters doc.

**Research:** Check the matrix layout proposed in `docs/claude/11-enhancements-and-optimizations.md` §E against the current adapters.

**Acceptance Criteria:**

- [ ] Matrix generated by a test or script from the contract-test results, not hand-written.
- [ ] CI fails when the committed matrix differs from the generated one.
- [ ] Every "shrinks" cell is backed by a silent-ignore test.

**Dependencies:** T-65-001

**Reference:** `docs/claude/11-enhancements-and-optimizations.md` §E

**Estimated Complexity:** M

---

### T-65-007 — Verify agy PostToolUse `overwriteResult`, then stub behind a flag

**Description:** Sprint 1 found that a non-empty PostToolUse `overwriteResult` may replace the
tool result text in agy. The documented reply is `{}`. Verify live whether it works for
`hooks.json` command hooks; if it does, add an opt-in, version-gated stub replacement to
`AntigravityAdapter`.

**Research:** Hypothesis: on agy 1.5.14, a command hook returning `overwriteResult` replaces the
result the model sees. Confirmed by a live session comparing the model-visible result with and
without the field, for a shell tool and an MCP tool. Also record whether the model is told the
result was changed. A rejected hypothesis closes the ticket with the finding, and agy stays
ingest-only.

**Acceptance Criteria:**

**Outcome (2026-11-23): rejected; agy stays ingest-only.** On agy 1.5.14, a project
`.agents/hooks.json` command hook answered PostToolUse with `{"overwriteResult": "<marker>"}` for
a shell call (`run_command`) and an MCP call (`call_mcp_tool`). The model quoted the original
results (`1`, and the icx status line), not the marker. The public hooks docs list no such field
either. Caveat: the owner's global hooks (ai-memory, icx) also answered PostToolUse with `{}`, and
how agy merges several replies is not documented.

- [x] agy version, hook config and the model-visible result per tool kind recorded.
- [x] ~~If confirmed: opt-in flag~~: not confirmed, so nothing ships.
- [x] Contract fixture added; no silent-ignore test is needed, since agy is ingest-only.
- [ ] Adapters doc and capability matrix updated (with T-65-006).

**Dependencies:** T-61-004

**Reference:** `ref-docs/agent-internals/below-hook-findings-2026-11-22.md` (agy hooks;
decision table)

**Estimated Complexity:** M

---

### T-65-008 — Match both agy MCP tool-name forms in hooks

**Description:** agy presents MCP tools to hook matchers as `call_mcp_tool` when they are loaded
lazily and as `mcp_<server>_<tool>` when they are loaded eagerly. Make the installed hook matchers
and `AntigravityAdapter` handle both forms. In particular, the pager exemption must recognise
icx's own tool under either name, so icx never stubs its own output.

**Research:** Confirm both forms on the installed agy, and what `call_mcp_tool` payloads carry to
identify the server and tool.

**Acceptance Criteria:**

**Live result (2026-11-23, agy 1.5.14):** a lazily loaded icx call arrived as
`toolCall.name: "call_mcp_tool"` with `args.ServerName: "icx"`, `args.ToolName: "icx"` and
`args.Arguments`; the shell tool is `run_command`. The adapter now exempts icx's own tools in that
form too: before, `call_mcp_tool` was ingested, icx's own replies included.

- [x] Both name forms recorded: `call_mcp_tool` live; `mcp_<server>_<tool>` from Sprint 1 and the
  public MCP docs.
- [x] The pager exemption holds for both forms and for `icx_write` (`tests/agy_fixtures.rs`).
- [ ] `icx init` and `icx install` write matchers that cover both forms: the recommended matcher
  is `""` (every tool), which covers both (with T-65-004).

**Dependencies:** T-61-004

**Reference:** `ref-docs/agent-internals/below-hook-findings-2026-11-22.md` (agy permissions)

**Estimated Complexity:** S

---

### T-65-009 — Hermes shell-output stubbing through `transform_terminal_output`

**Description:** Hermes's only seam for replacing shell output is the general-plugin hook
`transform_terminal_output` (`tools/terminal_tool_result.py:147-158` at `caafe5389ae`). The first
string it returns replaces the output, and the output limit is applied afterwards. A context
engine cannot register it. Ship an optional general plugin, listed in `plugins.enabled`, that
returns the icx stub for bulky shell output.

**Research:** Hypothesis: the plugin's stub reaches the model in place of the raw output, and the
handle dereferences losslessly. Confirmed by a live Hermes session. Check the plugin timeout
(about 30 s) against icx's ingest latency.

**Acceptance Criteria:**

- [ ] Separate general plugin, off by default, pinned to a Hermes commit range.
- [ ] Fails open: on error or timeout the raw output passes through unchanged.
- [ ] Verified by stand-in tests; live verification deferred to v1.5.0 (#89).
- [ ] Deployment guide Hermes section documents enabling it.

**Dependencies:** T-65-002

**Reference:** `ref-docs/agent-internals/below-hook-findings-2026-11-22.md` (Hermes hooks;
decision table)

**Estimated Complexity:** M

## Sprint exit

- [ ] Gates green: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and
  coverage `>= 95%`.
- [ ] Docs and CHANGELOG updated for any user-visible change; v1.4.0 release notes drafted.
- [ ] Upstream re-scan done for Phase 7 Sprint 1 (ranking crates and comparable tools), and its
  tickets adjusted.
