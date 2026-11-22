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

_None recorded yet._

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

**Dependencies:** T-61-002

**Reference:** Claude Code issues #86753 and #67442; `crates/icx-adapters/tests/claude_contract.rs`

**Estimated Complexity:** L

---

### T-64-002 — Handle `WorktreeCreate` and `WorktreeRemove`

**Description:** Use the worktree events to register and retire the worktree-to-main-root
mapping that v1.3.2 derives from git control files.

**Research:** Confirm payload fields on the installed version, and whether the events fire for
subagent worktrees.

**Acceptance Criteria:**

- [ ] Both events parsed; mapping registered on create and retired on remove.
- [ ] The v1.3.2 git-file derivation remains the fallback for older versions.
- [ ] Contract tests over recorded payloads.

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
cache.

**Research:** Confirm payload fields and whether the switch invalidates the provider cache for
the installed version.

**Acceptance Criteria:**

- [ ] Epoch sealed on switch; the deterministic prefix is re-emitted after.
- [ ] Contract tests over recorded payloads.

**Dependencies:** None

**Reference:** Plan Phase 6 Sprint 4; `docs/adr/0003-prefix-cache-epoch-pinning.md`

**Estimated Complexity:** S

---

### T-64-006 — Respect the 10,000-character `additionalContext` budget

**Description:** Keep every `additionalContext` reply within the documented 10,000-character
limit, with deterministic truncation that keeps the handles and drops the lowest-ranked lines.

**Research:** Confirm the current limit and what Claude Code does when it is exceeded
(truncation, rejection, or spill to a file).

**Acceptance Criteria:**

- [ ] Budget enforced in characters, not bytes, at the adapter boundary.
- [ ] Truncation keeps every `#icx:` handle intact and is deterministic.
- [ ] Property test: no reply exceeds the limit for any input.

**Dependencies:** None

**Reference:** Plan Phase 6 Sprint 4; Claude Code hooks reference (`additionalContext`)

**Estimated Complexity:** S

---

### T-64-007 — Stop plan-mode permission prompts for the icx tool

**Description:** Test on the installed Claude Code whether MCP `annotations.readOnlyHint` on the
icx tool, a whole-server allow rule (`mcp__icx`), or both stop the permission prompts that
appear in plan mode. Ship the annotations only if the test shows they work.

**Research:** Hypothesis: with `readOnlyHint: true`, plan mode runs the icx tool without a
prompt even when no allow rule exists. Measured by running plan-mode sessions in four
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
