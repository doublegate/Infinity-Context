# Phase 4 — Cross-Agent Low-Level Interception, MCP Pager & Wire Proxy

## Goal

Implement the unified `InfinityContextMmu` coordinator and concrete low-level adapters in `crates/icx-adapters` and `crates/icx-proxy` for **Claude Code (`2.1.326+`)**, **OpenAI Codex CLI (`0.169.0+`)**, **Google Antigravity CLI (`agy`)**, **Hermes Agent**, the **Single-Tool Polymorphic MCP Pager (`icx`)**, and the **Cache-Epoch HTTP/SSE Wire Proxy**.

## Exit Criteria

- [x] `ClaudeCodeAdapter` rewrites bulky tool outputs via `hookSpecificOutput.updatedToolOutput` and handles `SessionStart`, `UserPromptSubmit`, `PreCompact`, and `PostCompact`.
- [x] `CodexAdapter` emits schema-valid JSON matching Codex's embedded `*.command.output` schemas.
- [x] `AntigravityAdapter` injects UI-silent `systemMessage` steps (`metadata.user_markdown: ""`) on `PreInvocation`/`SessionStart` and continues across `MAX_TOKEN_BUDGET_EXCEEDED` on `Stop`.
- [x] `HermesAdapter` implements the `select_context` / `prune_tool_results_only` JSON-RPC bridge.
- [x] `McpPagerServer` handles `deref`, `slice`, `grep`, `recall`, `fold`, `xagent`, and `status` through a single `< 180 token` MCP tool definition.

## Sprints

- [Sprint 1 — Claude Code & OpenAI Codex Hook & WAL Adapters](sprint-1-claude-and-codex-adapters.md)
- [Sprint 2 — Antigravity CLI (`agy`) & Hermes ContextEngine Adapters](sprint-2-agy-and-hermes-adapters.md)
- [Sprint 3 — Single-Tool Polymorphic MCP Pager & Cache-Epoch Wire Proxy](sprint-3-mcp-pager-and-wire-proxy.md)

## Reference Docs

- [docs/agent-adapters-and-interception.md](../../docs/agent-adapters-and-interception.md)
- [docs/wire-protocol-and-shm-bus.md](../../docs/wire-protocol-and-shm-bus.md)
