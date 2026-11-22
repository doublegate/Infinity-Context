# Sprint 1 — Claude Code & OpenAI Codex Hook & WAL Adapters

**Phase:** Phase 4 — Cross-Agent Interception  
**Sprint Goal:** Implement `ClaudeCodeAdapter` and `CodexAdapter` in `crates/icx-adapters`.  
**Estimated Duration:** 1 week

## Tickets

### T-41-001 — Implement `ClaudeCodeAdapter` (`updatedToolOutput`, `additionalContext`, `PreCompact`)

**Description:** Implement `ClaudeCodeAdapter` handling `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse` (emitting `hookSpecificOutput.updatedToolOutput` with an AHTL stub + `#icx:<hash>` when output exceeds threshold), `PreCompact`, and `PostCompact`.

**Acceptance Criteria:**

- [x] `PostToolUse` with a 50 KB `Bash` output returns a compact `< 400 byte` `updatedToolOutput` AHTL stub and commits the raw 50 KB payload to `CasStore`.
- [x] `UserPromptSubmit` injects recalled AHTL packets and `/dev/shm` cross-agent telepathy updates into `additionalContext`.

**Dependencies:** T-11-004, T-22-002, T-33-002  
**Reference:** `docs/agent-adapters-and-interception.md` §2  
**Estimated Complexity:** M

---

### T-41-002 — Implement `CodexAdapter` Matching Codex `0.169.0` Embedded Wire Schemas

**Description:** Implement `CodexAdapter` handling `session-start`, `user-prompt-submit`, `post-tool-use`, `pre-compact`, and `post-compact` conforming strictly to Codex's `additionalProperties: false` JSON schemas.

**Acceptance Criteria:**

- [x] Emitted JSON contains only fields permitted by Codex `0.169.0`'s embedded `*.command.output` schemas.
- [x] Unit tests verify schema conformance for all supported hook events.

**Dependencies:** T-41-001  
**Reference:** `docs/agent-adapters-and-interception.md` §3  
**Estimated Complexity:** M
