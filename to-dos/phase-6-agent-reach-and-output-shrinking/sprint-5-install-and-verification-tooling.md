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

_None recorded yet._

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
- [ ] Live verification recorded; contract fixture added.
- [ ] Deployment guide Hermes section rewritten; ICX-AUD-006 moved to fixed.

**Dependencies:** T-61-005

**Reference:** ICX-AUD-006 (`docs/claude/findings.csv`, "no Hermes shim ships");
`docs/deployment-and-agent-integration-guide.md` (Hermes section)

**Estimated Complexity:** L

---

### T-65-003 — Verify agy `user_markdown` behaviour

**Description:** Confirm on the installed agy how `user_markdown` content is shown to the model,
and align `AntigravityAdapter` with it.

**Research:** Hypothesis: agy injects `user_markdown` into the model turn as user content.
Confirmed or rejected by a live session plus the T-61-004 findings.

**Acceptance Criteria:**

- [ ] Behaviour recorded with the agy version.
- [ ] Adapter output matches; contract fixture added.
- [ ] Adapters doc updated.

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
- Muse: the policy-plane key, still to be determined on the installed version.

The step runs in `--dry-run` by default: it prints the edit and never writes another agent's
permission config without explicit user confirmation.

**Research:** Re-verify each key on the installed version before coding. For Muse, use the
T-61-004 finding; if no key exists, document that and skip Muse.

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

## Sprint exit

- [ ] Gates green: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and
  coverage `>= 95%`.
- [ ] Docs and CHANGELOG updated for any user-visible change; v1.4.0 release notes drafted.
- [ ] Upstream re-scan done for Phase 7 Sprint 1 (ranking crates and comparable tools), and its
  tickets adjusted.
