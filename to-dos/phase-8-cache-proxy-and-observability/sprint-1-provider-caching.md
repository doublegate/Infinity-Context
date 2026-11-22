# Sprint 1 — Provider Caching

**Phase:** Phase 8 — Prompt-Cache Proxy and Observability  
**Release:** v1.6.0  
**Sprint Goal:** Make the proxy emit current provider cache controls, record the provider's own
cache counters, and stop presenting byte reuse as a provider cache hit rate.  
**Estimated Duration:** 1 week

## Research gate

Re-check before any ticket starts:

- **External docs and versions:** the OpenAI prompt caching guide (explicit breakpoints, the
  `prompt_cache_options.ttl` values, usage fields for cached and written tokens) and the Anthropic
  prompt caching docs (automatic caching, system messages after the first turn, cache diagnostics,
  `cache_read_input_tokens` / `cache_creation_input_tokens`). Record the API versions and model
  families each feature applies to.
- **Installed agent behaviour:** which installed agents route through `icx proxy` today and which
  request shapes they send (inspect a recorded request per agent; REA where undocumented).
- **Project evidence:** the current transforms in `crates/icx-proxy/src/wire.rs`, the
  `CacheEpochController` in `icx-core`, and every place the `>= 94%` figure appears
  (`rg -n '94' README.md CLAUDE.md docs/`).

### Research notes

_None recorded yet._

## Tickets

### T-81-001 — OpenAI explicit cache breakpoints and 12h TTL

**Description:** Extend the OpenAI transform to place explicit cache breakpoints at epoch
boundaries chosen by the `CacheEpochController`, and to set `prompt_cache_options.ttl` to the 12h
option when configured. Unknown or unsupported models pass through unchanged.

**Research:** Confirm the current field names, allowed TTL values, breakpoint limits and model
support in the OpenAI docs; confirm whether breakpoints and automatic prefix caching interact.
Hypothesis: epoch-aligned breakpoints raise provider-reported cached tokens over automatic caching
alone. Measurement: paired requests through the fake provider replaying recorded responses, and,
where authorized, a small live run comparing reported cached tokens. A rejected hypothesis ships
only the TTL option and records the finding.

**Acceptance Criteria:**

- [ ] Breakpoints are placed only at epoch boundaries and never exceed the documented limit.
- [ ] `prompt_cache_options.ttl` is set only when configured and only for supported models.
- [ ] Requests for unsupported models are byte-identical to the input.
- [ ] Golden request tests cover each placement case.
- [ ] Any cached-token difference is measured and published in `docs/numbers.md` with its run
  manifest.

**Dependencies:** None  
**Reference:** OpenAI prompt caching guide
(<https://platform.openai.com/docs/guides/prompt-caching>); plan Phase 8 Sprint 1  
**Estimated Complexity:** M

---

### T-81-002 — Anthropic automatic caching, mid-conversation system messages and diagnostics

**Description:** Update the Anthropic transform for automatic caching, keep `cache_control`
breakpoints valid when system messages appear mid-conversation, and surface the provider's cache
diagnostics in the proxy log.

**Research:** Confirm the current automatic-caching semantics, breakpoint count limit, minimum
cacheable length per model, how mid-conversation system messages affect the cached prefix, and the
shape of the diagnostics. Verify on the installed Claude Code version whether it already sets
`cache_control`, so the proxy does not double-place breakpoints.

**Acceptance Criteria:**

- [ ] The transform never emits more breakpoints than the documented limit, including when the
  client already set some.
- [ ] Mid-conversation system messages do not invalidate the epoch prefix in the golden tests.
- [ ] Provider diagnostics are logged per request, without request content or secrets.
- [ ] Requests that already use automatic caching pass through unchanged.

**Dependencies:** None  
**Reference:** Anthropic prompt caching docs
(<https://docs.anthropic.com/en/docs/build-with-claude/prompt-caching>); plan Phase 8 Sprint 1  
**Estimated Complexity:** M

---

### T-81-003 — Record provider cache counters

**Description:** Parse and store the provider-reported cache counters per proxied request
(`cache_read_input_tokens` and cache-write counters for Anthropic; cached-token usage fields for
OpenAI) alongside the epoch id, so cache behaviour is measured from the provider and not inferred
from byte reuse.

**Research:** Confirm the counter field names in both APIs, in both streaming and non-streaming
responses, and which tiers omit them.

**Acceptance Criteria:**

- [ ] Counters are captured from JSON and SSE responses.
- [ ] A missing counter is recorded as absent, never as zero and never estimated.
- [ ] Counters are available to `icx stats` (T-83-003) and the trace (T-83-001).
- [ ] Fixture tests cover present, absent and malformed counter fields.

**Dependencies:** None  
**Reference:** OpenAI and Anthropic usage objects (URLs above); plan Phase 8 Sprint 1  
**Estimated Complexity:** S

---

### T-81-004 — Relabel the `>= 94%` byte-reuse figure

**Description:** The `>= 94%` figure is a byte-reuse result from the prefix-cache simulation, not a
provider cache hit rate. Relabel it in README, docs and release notes, and propose the same change
for the `CLAUDE.md` project summary (a managed block, changed through its own mechanism, as in
T-H-002) and `GEMINI.md`. Point each occurrence at `docs/numbers.md`. Do not reuse it as a provider metric.

**Research:** List every occurrence with `rg`, including badges, alt text and diagrams; check which
are generated (diagram sources) so the source is edited rather than the output.

**Acceptance Criteria:**

- [ ] Every occurrence says "byte reuse (simulation)" or links to the row that defines it.
- [ ] No document presents the figure as a provider cache hit rate.
- [ ] Generated diagrams are regenerated from their edited sources.
- [ ] `AGENTS.md` is changed only by its owner; if it carries the figure, the PR lists it for the
  owner.

**Dependencies:** None  
**Reference:** `CLAUDE.md:25`; `docs/numbers.md`; plan Phase 8 Sprint 1 and Conventions
("Disowned figures")  
**Estimated Complexity:** S

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Line coverage is `>= 95%` (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Affected docs and `CHANGELOG.md` `[Unreleased]` are updated.
- [ ] Upstream re-scan for Sprint 2 is done and its tickets are adjusted.
