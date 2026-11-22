# Phase 6 — Agent Reach and Output Shrinking (`v1.4.0`)

## Goal

Extend where icx can act and how much tool output it can actually shrink. The phase looks below
the documented hooks of the installed agents (REA/Ghidra, interoperability only), closes the
Codex gap where hooks cannot replace tool output, adds adapters for Gemini CLI, Cursor and
OpenCode, deepens the Claude Code integration, and ships install and verification tooling that
proves each adapter against the agent the user has installed.

## Exit Criterion

- [ ] Every supported agent has a contract fixture captured from a live run on a recorded agent
  version, and the capability matrix is generated from those contract tests.
- [ ] For each agent, the matrix states whether icx shrinks tool output, and that statement is
  backed by a test that fails when the agent silently ignores the replacement.
- [ ] `icx install --agent <name> --dry-run` prints every config edit, including MCP tool
  pre-approval, and writes nothing without explicit user confirmation.
- [ ] Gates green: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and
  coverage `>= 95%`.

## Sprints

- [Sprint 1 — Agent internals investigation (REA/Ghidra)](sprint-1-agent-internals-investigation.md)
- [Sprint 2 — Codex output shrinking](sprint-2-codex-output-shrinking.md)
- [Sprint 3 — New adapters: Gemini CLI, Cursor and OpenCode](sprint-3-new-adapters.md)
- [Sprint 4 — Claude Code depth](sprint-4-claude-code-depth.md)
- [Sprint 5 — Install and verification tooling](sprint-5-install-and-verification-tooling.md)

Implementation approach, risks and verification:
[phase_6_implementation_plan.md](phase_6_implementation_plan.md).

## Dependencies on Earlier Phases

- **v1.3.2 shipped** (PR #38): worktree stubs, MCP workspace following and the pager fixes are
  the starting point. Issues #33, #34, #36 and #37 are closed there.
- **Phase 4** adapters (`ClaudeCodeAdapter`, `CodexAdapter`, `AntigravityAdapter`,
  `HermesAdapter`) and the v1.3.0 Muse adapter are the base every sprint extends.
- **ADR-0006** (v1.2.0 context and compatibility) governs how new hook fields are added without
  breaking older agent versions.
- **`ref-docs/agent-internals/`** (`rea-binary-and-sqlite-forensics.md`,
  `cross-agent-hook-and-wire-matrix.md`) is the Sprint 1 baseline. That corpus is immutable;
  new findings go in a new dated file.

## Standing Sources to Watch

Re-check these at every sprint start and close, and record changes in the sprint's research
notes:

- Claude Code changelog and hook reference (hooks, `updatedToolOutput`, `updatedMCPToolOutput`,
  permission rules, plan mode); issues #86753 and #67442.
- OpenAI Codex CLI releases and hooks documentation (`decision:"block"`, `updatedMCPToolOutput`,
  `[mcp_servers]` approval keys, rollout format).
- Google Antigravity CLI (`agy`) release notes and `~/.gemini/antigravity-cli/settings.json`
  schema.
- Gemini CLI hooks documentation and releases (`AfterTool`, `BeforeModel`, `PreCompress`).
- Cursor hooks documentation (`postToolUse`, `updated_mcp_tool_output`).
- OpenCode plugin API (`tool.execute.after`, `experimental.session.compacting`).
- Hermes Agent source (`ContextEngine`, `select_context`) and Meta Muse Code release notes,
  including its policy plane.
- MCP specification changelog (2026-07-28 revision and later), in particular tool
  `annotations`.
- Comparable tools: context-mode and rtk repositories, for hook usage and failure modes.
