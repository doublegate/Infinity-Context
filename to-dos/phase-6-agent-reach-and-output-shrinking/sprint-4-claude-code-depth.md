# Sprint 4 — Claude Code Depth

**Phase:** Phase 6 — Agent Reach and Output Shrinking

**Release:** v1.4.0

**Sprint Goal:** Make every Claude Code replacement shape-exact and verified, adopt the newer
lifecycle events, respect the `additionalContext` budget, and stop needless permission prompts
for the icx MCP tool in plan mode.

**Estimated Duration:** 2 weeks

## Research gate

Re-check before the sprint starts:

- **External docs and versions:** the Claude Code hooks reference and changelog since 2.1.328
  (`updatedToolOutput`, `updatedMCPToolOutput`, `WorktreeCreate`, `WorktreeRemove`,
  `CwdChanged`, `PostToolBatch`, `PreModelSwitch`, `PostModelSwitch`, `additionalContext`
  limits); permission-rule syntax and plan-mode behaviour; the MCP specification for tool
  `annotations` (`readOnlyHint`).
- **Installed agent behaviour:** the installed Claude Code version; T-61-002 findings on
  per-tool result shaping; whether plan mode prompts for the icx tool with and without allow
  rules.
- **Project evidence:** `crates/icx-adapters/tests/claude_contract.rs`; the v1.3.2 worktree and
  MCP workspace-following code; issue #34; the field scan of permission prompts.

### Research notes

**2026-11-22, from Sprint 1** (`ref-docs/agent-internals/below-hook-findings-2026-11-22.md`, Claude
Code section and decision table; Claude Code 2.1.330):

- **Replacement validation:** a replacement is checked against the tool's output schema and then
  its renderer. On failure the original output is used and the user sees a hook-error notice.
  The required fields per built-in tool are tabled in the dossier (Bash, Read, Grep, Glob,
  WebFetch, Write, Edit). WebSearch and Agent are unresolved.
- **Precedence:** `updatedToolOutput` wins. `updatedMCPToolOutput` is used only when it is
  absent, and an empty value is ignored. MCP replacements are not schema-validated.
- **Plan mode:** a non-read-only MCP tool is held before allow rules are consulted. Hook and
  plugin allows do not lift the hold, so `readOnlyHint: true` is the only route. Expect the
  "rule only" configuration in T-64-007 to keep prompting.
- **`additionalContext`:** on the settings-file hook path icx uses, the cap is 8,000 characters
  and 200 lines, not 10,000. Larger context is spilled to a file. A `hookEventName` that does not
  match the event drops the context. The 10,000-character figure is the PreCompact stdout cap.
- **Payloads confirmed:** `WorktreeCreate` (name), `WorktreeRemove` (worktree_path), `CwdChanged`
  (old_cwd, new_cwd), `PostToolBatch` (`tool_calls[]`, may add context), and
  `PreModelSwitch`/`PostModelSwitch` (model ids, source, context tokens, prompt-cache warmth and
  TTL; Pre can allow, deny or ask).

**2026-11-22, research gate** (Sprint 3 exit re-scan; `anthropics/claude-code` `CHANGELOG.md`):

- **Version:** the installed Claude Code is 2.1.330, the latest published release, so nothing
  has landed since the Sprint 1 findings.
- **Code Mode exists in Claude Code** (2.1.328 and later). Tools can run inside a script, the
  mechanism that defeated the Codex stub route (T-62-001).
  - 2.1.328: Code Mode no longer runs `UserPromptSubmit` hooks on a script's output, and "to
    redact that output, use a `PostToolUse` hook with `updatedToolOutput`". That suggests
    replacement still reaches the script's tool calls.
  - T-64-001 therefore runs its live checks with Code Mode both off and on, and records what the
    model sees in each.
- **Refused calls (2.1.328):** managed `PostToolUse` hooks no longer fire for a call refused
  before it ran, while `PostToolBatch` receives most of them, not marked as refused. T-64-004
  must not count a batch entry as a completed result without its output.
- **Plan mode:** 2.1.328 to 2.1.330 change only which model plan mode uses. Nothing touches the
  MCP hold that T-64-007 tests.
- **Worktrees (2.1.330):** `ExitWorktree` no longer deletes a worktree another session is using.
  T-64-002 must not retire a mapping while another session still reports that worktree.

**2026-11-22, documented contracts** (code.claude.com/docs/en/hooks; the worktrees page could not
be read):

- **`WorktreeCreate` replaces Claude Code's own worktree creation.** The docs say:
  - "Configuring a WorktreeCreate hook replaces that default git behavior";
  - "A `WorktreeCreate` command hook can't return JSON, because Claude Code reads its stdout as
    the worktree path";
  - "Any non-zero exit code causes worktree creation to fail."

  The input is `name` (a slug), with no matcher. Registering it for observation would make icx
  responsible for creating every worktree. **T-64-002 is closed as rejected** (see the ticket).
- **`WorktreeRemove`** takes `worktree_path`. Its JSON output is discarded, and a non-zero exit
  fails the removal. The docs pair it with `WorktreeCreate` ("pair it with a WorktreeRemove hook
  to control cleanup"); whether registering it alone changes removal is not stated.
- **`CwdChanged`:** `old_cwd`, `new_cwd` plus the common fields, with no matcher and no decision
  control. Exit 2 shows stderr to the user only.
- **`PreModelSwitch`:** `from_model` and `to_model`. It can allow, deny or ask, `decision:
  "block"` cancels the switch, exit 2 blocks it, and a timeout also blocks it (one read). icx
  must therefore not register it: a slow or failing hook would block model switches.
  **`PostModelSwitch`** has the same fields, accepts `additionalContext` only, and exit 2 only
  shows stderr.
- **`PostToolBatch`:** common fields plus `tool_calls[]`, each with `tool_name`, `tool_input`,
  `tool_use_id` and `tool_response` (a string or content blocks), and no matcher. The only
  documented output is `additionalContext` ("`PostToolBatch` hooks can inject context for
  Claude"); `decision: "block"` and exit 2 stop the agentic loop. No replacement is documented.
- Field types and the version that added each event are not stated.

**2026-11-22, finding while building T-64-005:** the compaction and session-end events of every
adapter call `rollover_and_sync_dag_root`, which seals only when the epoch has reached its token
threshold (`maybe_rollover`). The docs say these events "seal the epoch" unconditionally, which
is true only of a full epoch. A model switch now forces the seal
(`force_rollover_and_sync_dag_root`). Whether compaction should force it too (history is
rewritten, so the old cache is gone) is an open decision, raised with the owner; until then the
docs and the code disagree on this point.

## Tickets

### T-64-001 — Shape-exact `updatedToolOutput` for every built-in tool

**Description:** For each Claude Code built-in tool whose output icx replaces, emit an
`updatedToolOutput` value whose shape matches that tool's native result, and add a test that
detects when Claude Code silently ignores the replacement.

**Research:** Hypothesis: Claude Code ignores a replacement whose shape does not match the
tool's native result (issues #86753, #67442). Confirmed by a live run per tool comparing the
model-visible output for a matching and a non-matching shape.

**Acceptance Criteria:**

- [ ] A shape table per built-in tool, from T-61-002 and live runs, with the Claude Code version.
- [ ] `claude_contract.rs` pins each shape.
- [ ] A silent-ignore detector: a live-run test fails if the model-visible output is unchanged.
- [ ] Tools where replacement does not work are listed as ingest-only in the adapters doc.
- [ ] For MCP tools, a regression test pins that `updatedToolOutput` is sent and that
  `updatedMCPToolOutput` is never empty.

**Dependencies:** T-61-002

**Reference:** Claude Code issues #86753 and #67442; `crates/icx-adapters/tests/claude_contract.rs`

**Estimated Complexity:** L

---

### T-64-002 — Handle `WorktreeCreate` and `WorktreeRemove`

**Description:** Use the worktree events to register and retire the worktree-to-main-root
mapping that v1.3.2 derives from git control files.

**Research:** Confirm payload fields on the installed version, and whether the events fire for
subagent worktrees.

**Outcome (2026-11-22): rejected.** A `WorktreeCreate` hook replaces Claude Code's own git
worktree creation and must print the new worktree's path, so icx cannot register it just to
observe. `WorktreeRemove` alone would not see creations, and the docs pair it with
`WorktreeCreate`. The v1.3.2 derivation from git control files stays the only mechanism; it
needs no hook and works on every version. The ticket closes with this finding and no feature.

**Acceptance Criteria (superseded by the outcome above):**

- [ ] ~~Both events parsed; mapping registered on create and retired on remove.~~
- [x] The v1.3.2 git-file derivation remains the mechanism.
- [ ] ~~Contract tests over recorded payloads.~~

**Dependencies:** None

**Reference:** Plan Phase 6 Sprint 4; v1.3.2 worktree stubs (PR #38)

**Estimated Complexity:** M

---

### T-64-003 — Handle `CwdChanged` as the workspace-switch trigger

**Description:** Use `CwdChanged` to switch the MCP server's workspace, replacing the heuristic
trigger behind issue #34.

**Research:** Confirm when `CwdChanged` fires (shell `cd`, worktree switch, subagent) on the
installed version.

**Acceptance Criteria:**

- [ ] Workspace follows `CwdChanged`; the v1.3.2 heuristic stays as fallback.
- [ ] Test covers a change into a different repository and into a worktree.
- [ ] Issue #34 notes updated with the new trigger.

**Dependencies:** None

**Reference:** Issue #34; v1.3.2 MCP workspace following (PR #38)

**Estimated Complexity:** S

---

### T-64-004 — Handle `PostToolBatch`

**Description:** Ingest parallel tool results once per batch, so the tail slot and the epoch
controller see one update per batch instead of one per tool.

**Research:** Confirm the payload and whether a batch reply can replace outputs, or only add
context.

**Acceptance Criteria:**

- [ ] Batch payload parsed; per-tool ingestion is not duplicated when both events fire.
- [ ] Contract test over a recorded batch.
- [ ] Effect on lock contention measured and published in `docs/numbers.md`.

**Dependencies:** None

**Reference:** Plan Phase 6 Sprint 4; field scan (8 s PostToolUse outliers under parallel
agents)

**Estimated Complexity:** M

---

### T-64-005 — Handle `PreModelSwitch` and `PostModelSwitch`

**Description:** Seal the cache epoch on a model switch, since a new model starts a new prompt
cache. Use `PostModelSwitch` only. `PreModelSwitch` blocks the switch on a non-zero exit and, by
one reading of the docs, on a timeout, so icx does not register it.

**Research:** Confirm payload fields and whether the switch invalidates the provider cache for
the installed version.

**Acceptance Criteria:**

- [x] Epoch sealed on switch (forced, `PostModelSwitch`). On the hook path icx injects no
  deterministic prefix, so there is nothing to re-emit; the proxy path starts a new epoch on its
  next request.
- [ ] Contract tests over recorded payloads. Tests use the documented fields
  (`tests/claude_model_switch.rs`); a live recording waits for T-64-001's live runs.

**Dependencies:** None

**Reference:** Plan Phase 6 Sprint 4; `docs/adr/0003-prefix-cache-epoch-pinning.md`

**Estimated Complexity:** S

---

### T-64-006 — Respect the `additionalContext` budget (8,000 characters, 200 lines)

**Description:** Keep every `additionalContext` reply within the cap Claude Code applies on the
settings-file hook path: 8,000 characters and 200 lines (Sprint 1; the 10,000-character figure
first planned is the PreCompact stdout cap). Use deterministic truncation that keeps the handles
and drops the lowest-ranked lines. Always set `hookEventName` to the exact event name.

**Research:** Re-confirm both caps and the spill-to-file behaviour on the installed version.

**Acceptance Criteria:**

- [x] Budget enforced in characters and lines, not bytes, at the adapter boundary
  (`claude_bounded_context`; characters as UTF-16 units, as JavaScript counts them).
- [x] Every reply carries the exact `hookEventName`; a test covers each emitting event
  (`SessionStart`, `UserPromptSubmit`, `SubagentStart`).
- [x] Truncation keeps every `#icx:` handle intact and is deterministic (whole lines only).
- [x] Property test: no reply exceeds the limit for any input (`tests/claude_context_budget.rs`).
- [ ] Re-confirm both caps and the spill-to-file behaviour live on 2.1.330 (with T-64-001's live
  runs).

**Dependencies:** None

**Reference:** Plan Phase 6 Sprint 4; Claude Code hooks reference (`additionalContext`)

**Estimated Complexity:** S

---

### T-64-007 — Stop plan-mode permission prompts for the icx tool

**Description:** Test on the installed Claude Code whether MCP `annotations.readOnlyHint` on the
icx tool, a whole-server allow rule (`mcp__icx`), or both stop the permission prompts that
appear in plan mode. Ship the annotations only if the test shows they work.

**Research:** Hypothesis: with `readOnlyHint: true`, plan mode runs the icx tool without a
prompt even when no allow rule exists. Sprint 1 found that plan mode holds non-read-only MCP
tools before consulting allow rules, so "rule only" is expected to keep prompting. Measured by
running plan-mode sessions in four
configurations (no rule and no hint; rule only; hint only; both) and recording whether a prompt
appears. A rejected hypothesis closes the annotation part with the finding, and the docs point
users to the allow rule.

**Acceptance Criteria:**

- [ ] Claude Code version and the result for each of the four configurations recorded.
- [ ] Annotations added to the tool definition only if "hint only" stops the prompt.
- [ ] Deployment guide states the working configuration, including the user-level
  `permissions.allow` entry for sessions and worktree subagents outside the project tree.

**Dependencies:** T-61-002

**Reference:** Plan "Immediate fixes" (repeated icx permission prompts); MCP specification tool
`annotations`

**Estimated Complexity:** S

---

### T-64-008 — Decide whether `fold` moves to a separate non-read-only tool

**Description:** `fold` and `expand` change state, while `deref`, `slice`, `grep`, `recall`,
`xagent` and `status` only read. Decide whether `fold` belongs in a separate tool, so the
read-only tool can carry `readOnlyHint` honestly.

**Research:** Depends on the T-64-007 result. Check how each supported agent displays and
approves multiple tools from one server, and the cost of a second tool definition in the prompt.

**Acceptance Criteria:**

- [ ] Decision recorded as an ADR (Context, Decision, Consequences).
- [ ] If split: the read-only tool carries `readOnlyHint`; the state-changing tool does not;
  old `op` values keep working for one release with a deprecation note.
- [ ] If not split: the tool carries no `readOnlyHint`, and the reason is documented.
- [ ] Phase 7 Sprint 3 (typed eviction through `fold`) updated to match.

**Dependencies:** T-64-007

**Reference:** Plan "Immediate fixes"; `docs/wire-protocol-and-shm-bus.md` (single-tool MCP
pager)

**Estimated Complexity:** M

## Sprint exit

- [ ] Gates green: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and
  coverage `>= 95%`.
- [ ] Docs and CHANGELOG updated for any user-visible change.
- [ ] Upstream re-scan done for Sprint 5 (every agent's release notes and config schema), and
  its tickets adjusted.
