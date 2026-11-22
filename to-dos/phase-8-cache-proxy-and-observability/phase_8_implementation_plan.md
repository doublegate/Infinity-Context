# Phase 8 (v1.6.0) Implementation Plan — Prompt-Cache Proxy and Observability

## Goal Description

Deliver v1.6.0 in four sprint pull requests. Each sprint opens with its research gate, because
every provider and protocol fact this phase depends on changes faster than the plan does.

## Ordered Approach

1. **Sprint 8.1 — Provider caching (T-81-001 to T-81-004).**
   - Re-read the current OpenAI and Anthropic caching docs and record what changed.
   - Add OpenAI explicit cache breakpoints and the `prompt_cache_options.ttl` 12h option to the
     OpenAI transform; add Anthropic automatic caching, mid-conversation system messages and cache
     diagnostics to the Anthropic transform.
   - Record the provider counters per request so cache behaviour is measured, not inferred.
   - Relabel the `>= 94%` byte-reuse figure in README, CLAUDE.md summary and docs as a byte-reuse
     simulation figure, not a provider cache hit rate.
2. **Sprint 8.2 — Offline test harness (T-82-001 to T-82-003).**
   - Build a fake provider (HTTP, SSE, cancellation) first, so the incremental SSE parser and the
     HTTP/2 upstream are developed against it rather than against paid endpoints.
3. **Sprint 8.3 — Observability (T-83-001 to T-83-005).**
   - Trace model first (one trace per hook, spans, token counts), then bus timing per hook as the
     #32 baseline, then `icx stats` reading those traces, then the generated numbers table and the
     dataset schema that feeds it. The optional dashboard is last and may slip.
4. **Sprint 8.4 — MCP 2026-07-28 alignment (T-84-001 to T-84-006).**
   - Finish `server/discover` and per-request `_meta`, add `outputSchema` / `structuredContent`
     with `ttlMs` / `cacheScope`, fix the tool order, remove any dependence on roots or sampling,
     and optionally expose stubs as resources.

## Risks

- **Provider API drift.** Breakpoint, TTL and diagnostics fields may change between planning and
  execution. Mitigation: the research gate, and transforms keyed on recorded API versions with
  fallback to pass-through when a field is unknown.
- **Counter availability.** Not every provider or account tier returns cache counters. Mitigation:
  record absence explicitly; never substitute a byte-reuse estimate for a missing counter.
- **Trace overhead on the hook path.** Mitigation: measure hook latency with and without tracing
  and publish both in `docs/numbers.md`; tracing must be cheap to disable.
- **HTTP/2 and SSE regressions.** Mitigation: the fake provider covers cancellation and partial
  frames before either change merges.
- **MCP client support lag.** Clients may not yet read `structuredContent` or `server/discover`.
  Mitigation: keep the text content block alongside structured output, and test against each
  installed client.

## Verification

- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` and `cargo llvm-cov ... --fail-under-lines 95` on every sprint PR.
- Fake-provider integration tests for every transform, the SSE parser and the HTTP/2 path.
- Provider-counter capture runs only where separately authorized (paid calls), with manifests
  recorded; otherwise the fake provider supplies recorded counter fixtures.
- Every new figure measured and published in `docs/numbers.md` from the generated table.
- MCP conformance tests against the 2026-07-28 schema plus manual checks on each installed client.
