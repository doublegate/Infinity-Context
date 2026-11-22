# Phase 8 — Prompt-Cache Proxy and Observability

**Release:** v1.6.0

## Goal

Bring the `icx-proxy` wire transformer in line with the current provider prompt-caching APIs
(OpenAI explicit breakpoints and TTL, Anthropic automatic caching and diagnostics), record the
provider's own cache counters instead of inferring cache behaviour from byte reuse, test the proxy
offline against a fake provider, and make every hook observable: one trace per hook, measured bus
timing, an `icx stats` command and a generated numbers table. Close the phase by aligning the MCP
pager with the 2026-07-28 MCP specification.

## Exit Criterion

- [ ] The proxy emits provider-correct cache controls for OpenAI and Anthropic, verified against
  the fake provider and recorded provider counters, and the byte-reuse figure is labelled as such
  everywhere it appears.
- [ ] Every hook invocation produces one trace with spans and before/after token counts, and bus
  publish/poll timing per hook is measured and published in `docs/numbers.md` as the `icx-shm`
  baseline (#32).
- [ ] `docs/numbers.md` tables are generated from a versioned benchmark dataset schema, not edited
  by hand.
- [ ] The MCP server passes conformance checks for `server/discover`, `outputSchema` /
  `structuredContent`, `ttlMs` / `cacheScope` and a deterministic tool order, with no dependence on
  roots or sampling.

## Sprints

- [Sprint 1 — Provider caching](sprint-1-provider-caching.md)
- [Sprint 2 — Offline test harness](sprint-2-offline-test-harness.md)
- [Sprint 3 — Observability](sprint-3-observability.md)
- [Sprint 4 — MCP 2026-07-28 alignment](sprint-4-mcp-2026-07-28-alignment.md)

## Dependencies on Earlier Phases

- Phase 4 (`icx-proxy` `WireProxyTransformer`, `McpPagerServer`) and Phase 5 (`icx benchmark`,
  `icx doctor`, `docs/numbers.md`).
- Phase 6 Sprint 5 contract fixtures: the fake provider in Sprint 2 reuses the fixture layout.
- Phase 7 Sprint 2 evaluation set: the `icx stats` retrieval panels report its metrics when present.
- Issue #32 (`icx-shm` design): Sprint 3 supplies its measured baseline; Phase 9 Sprint 3 supplies
  its aarch64 runner.

## Standing Sources to Watch

- OpenAI prompt caching guide and API changelog (explicit breakpoints,
  `prompt_cache_options.ttl`, usage fields).
- Anthropic prompt caching documentation and release notes (automatic caching, cache diagnostics,
  `cache_read_input_tokens` / `cache_creation_input_tokens`).
- MCP specification changelog (2026-07-28 revision and any later drafts).
- Claude Code, Codex CLI, Antigravity CLI, Hermes and Muse changelogs (hook payload and timing
  changes that affect per-hook traces).
- OpenTelemetry GenAI semantic conventions.
- Comparable proxies and caching tools (LiteLLM, Helicone, claude-code-router) for counter and
  trace conventions.

## Reference Docs

- [docs/claude/11-enhancements-and-optimizations.md](../../docs/claude/11-enhancements-and-optimizations.md)
- [docs/codex/09-context-budget-and-enhancements.md](../../docs/codex/09-context-budget-and-enhancements.md)
- [docs/codex/11-remediation-roadmap.md](../../docs/codex/11-remediation-roadmap.md)
- [docs/wire-protocol-and-shm-bus.md](../../docs/wire-protocol-and-shm-bus.md)
- [docs/numbers.md](../../docs/numbers.md)
