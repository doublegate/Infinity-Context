# Sprint 1 — Pre-Execution Command Rewriter

**Phase:** Phase 10 — Prevent and Prove  
**Release:** v1.8.0  
**Sprint Goal:** Rewrite known shell commands in PreToolUse to filtered forms so bulky output never
reaches the agent, fail-open and never longer than the original, with a bypass and a handle to the
full text.  
**Estimated Duration:** 2 weeks

## Research gate

Re-check before any ticket starts:

- **External docs and versions:** rtk's current filter set, how it rewrites commands, and its
  documented failure modes and issues; each agent's PreToolUse documentation for input rewriting
  (Claude Code `updatedInput`, Codex, agy, Gemini CLI, Cursor, OpenCode, Hermes, Muse).
- **Installed agent behaviour:** on each installed agent version, whether a PreToolUse hook can
  rewrite a shell command's input and whether the agent shows the rewritten command to the model
  and the user. Confirm with a live hook and REA where the docs are silent.
- **Project evidence:** the Phase 6 Sprint 2 Codex shrinking result (where post-hoc replacement is
  impossible); field session logs for the most frequent bulky commands; the existing stub path
  that issues `#icx` handles.

### Research notes

_None recorded yet._

## Tickets

### T-10-1-001 — Fail-open PreToolUse command rewriter

**Description:** Add a PreToolUse hook that matches known shell commands and rewrites them to run
through `icx` filters (the rtk pattern). It is fail-open: when a filter fails, the raw output and
the original exit code pass through, and filtered output is never longer than the original.

**Research:** Confirm per agent that the rewrite is applied and visible as described above.
Hypothesis: rewriting the most frequent bulky commands reduces tokens returned to the agent in
field sessions. Measurement: replay the field command corpus with and without rewriting.

**Acceptance Criteria:**

- [ ] The exit code of every rewritten command equals the original's (property test).
- [ ] Filtered output length is never greater than the original (property test).
- [ ] Any filter error passes the raw output through unchanged (fault-injection test).
- [ ] Rewriting is opt-in per agent and documented.
- [ ] Token reduction on the field corpus is measured and published in `docs/numbers.md`.

**Dependencies:** None (builds on T-62-002)  
**Reference:** rtk repository (pinned URL and commit recorded in the research notes); plan
Phase 10 Sprint 1; Claude Code hooks reference
(<https://docs.anthropic.com/en/docs/claude-code/hooks>)  
**Estimated Complexity:** L

---

### T-10-1-002 — `icx bypass` and an `#icx` handle on truncated output

**Description:** Add `icx bypass` (per command or for a session) to disable rewriting, and attach
an `#icx` handle to every truncated output so the full text stays retrievable through `deref`.

**Research:** Confirm how a bypass can be signalled per agent (environment, command prefix, or
session flag) without the model being able to disable redaction.

**Acceptance Criteria:**

- [ ] A bypassed command runs exactly as typed.
- [ ] Every truncated output carries a handle whose `deref` returns the full, redacted output.
- [ ] Bypass does not disable secret redaction.

**Dependencies:** T-10-1-001  
**Reference:** plan Phase 10 Sprint 1  
**Estimated Complexity:** S

---

### T-10-1-003 — Per-command filters with golden tests

**Description:** Implement filters for git, cargo, npm/pnpm, pytest, docker and ls/find, each with
golden tests built from recorded outputs, including failure outputs that must survive filtering.

**Research:** For each command, confirm its output formats across current versions and which lines
carry errors. Compare with rtk's filters and note where `icx` differs and why.

**Acceptance Criteria:**

- [ ] Each filter has golden tests for success, failure and unusual output.
- [ ] Error lines, failing test names and file paths with line numbers are never dropped.
- [ ] Unknown output formats fall back to pass-through.
- [ ] Per-filter reduction on the field corpus is measured and published in `docs/numbers.md`.

**Dependencies:** T-10-1-001  
**Reference:** plan Phase 10 Sprint 1; rtk filter set  
**Estimated Complexity:** L

---

### T-10-1-004 — Extend Codex shrinking to pre-execution

**Description:** Use the rewriter for Codex where Phase 6 found post-hoc output replacement
impossible, so Codex sessions also receive filtered output.

**Research:** Confirm on the installed Codex version that PreToolUse input rewriting works for
shell commands and how it interacts with the Phase 6 `decision:"block"` path.

**Acceptance Criteria:**

- [ ] A Codex contract test proves the rewritten command runs and its filtered output reaches the
  model.
- [ ] Behaviour without rewriting support falls back to the Phase 6 path.

**Dependencies:** T-10-1-001, T-10-1-003  
**Reference:** plan Phase 6 Sprint 2; plan Phase 10 Sprint 1; Codex hooks documentation  
**Estimated Complexity:** M

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Line coverage is `>= 95%` (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Affected docs and `CHANGELOG.md` `[Unreleased]` are updated.
- [ ] Upstream re-scan for Sprint 2 is done and its tickets are adjusted.
