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

### T-65-001 — Contract fixtures from live runs for Codex, agy, Hermes and Muse

**Description:** Capture hook input and expected output for every handled event from live runs
of each agent, and replace hand-written test inputs with these fixtures.

**Research:** Record each agent version at capture time. Redact the captures before commit.

**Acceptance Criteria:**

- [ ] One fixture set per agent under `crates/icx-adapters/tests/fixtures/<agent>/<version>/`.
- [ ] Contract tests run against the fixtures; gitleaks clean.
- [ ] A script re-captures fixtures for a new agent version.
- [ ] Adapters doc line 13 updated to state which agents are pinned by live fixtures.

**Dependencies:** T-62-002, T-63-004, T-64-001

**Reference:** `docs/agent-adapters-and-interception.md` line 13

**Estimated Complexity:** L

---

### T-65-002 — Ship the Hermes `ContextEngine` shim

**Description:** Ship a Hermes plugin that implements the `ContextEngine` interface found in
T-61-005 and calls icx, and verify it in a live Hermes session.

**Research:** Hypothesis: the shim loads on the installed Hermes commit and its context reaches
the model. Confirmed by a live session log showing the shim's output in the assembled context.

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
- [ ] Live verification recorded; contract fixture added.
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

- [ ] Behaviour of `user_markdown` and `renderInfo.hidden` recorded with the agy version, per UI.
- [ ] The "completely invisible" claim removed from the adapter's module docs, the adapters doc
  and the deployment guide; they state that injected steps are visible.
- [ ] `renderInfo.hidden` emitted only if the live test shows it hides the step.
- [ ] Contract fixture added.

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

- [ ] Supported agents: Claude Code, Codex, agy, Muse, Hermes, Gemini CLI, Cursor, OpenCode.
- [ ] `--dry-run` output is a diff per file; no file is written in dry-run mode (tested).
- [ ] A real install backs up each edited file and is idempotent (second run makes no change).
- [ ] Unknown keys and comments in agent configs are preserved.

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

- [ ] The step prints the exact edit per agent and writes only after an explicit confirmation
  (interactive prompt or a dedicated flag); a test proves that nothing is written otherwise.
- [ ] The approval covers only the icx server, never a wildcard over all servers or tools.
- [ ] Each edited file is backed up; the edit is idempotent.
- [ ] Each key is verified on a live run and recorded with the agent version.
- [ ] Deployment guide documents the approvals and how to remove them.

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

- [ ] agy version, hook config and the model-visible result per tool kind recorded.
- [ ] If confirmed: opt-in flag, off by default, enabled only on verified agy versions; default
  output unchanged.
- [ ] A live-run test detects a silent ignore; contract fixture added.
- [ ] Adapters doc and capability matrix updated.

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

- [ ] Both name forms recorded from a live session with the agy version.
- [ ] The pager exemption holds for both forms (contract tests over recorded payloads).
- [ ] `icx init` and `icx install` write matchers that cover both forms.

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
- [ ] Live verification recorded; contract fixture added.
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
