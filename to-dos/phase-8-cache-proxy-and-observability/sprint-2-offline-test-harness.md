# Sprint 2 — Offline Test Harness

**Phase:** Phase 8 — Prompt-Cache Proxy and Observability  
**Release:** v1.6.0  
**Sprint Goal:** Test the proxy end to end without paid endpoints, then replace whole-body SSE
handling with an incremental parser and add HTTP/2 to the upstream.  
**Estimated Duration:** 1.5 weeks

## Research gate

Re-check before any ticket starts:

- **External docs and versions:** current SSE event formats for OpenAI (Responses and Chat
  Completions) and Anthropic Messages streaming, including error and ping events; the HTTP/2
  support status of the upstream client in use (curl today; the rustls client planned in Phase 10
  Sprint 7) and of `hyper`/`h2` if adopted (Context7).
- **Installed agent behaviour:** whether each agent that uses the proxy cancels streams mid-response
  and how (connection close, RST_STREAM).
- **Project evidence:** the current relay in `crates/icx-proxy/src/wire.rs`
  (`relay_upstream_response_with_eof`), its tests, and the Phase 6 Sprint 5 fixture layout.

### Research notes

_None recorded yet._

## Tickets

### T-82-001 — Fake provider for HTTP, SSE and cancellation

**Description:** Add a test-only fake provider that serves recorded and synthetic responses for
both providers: plain JSON, SSE streams split at arbitrary byte boundaries, slow streams, upstream
errors, and client cancellation mid-stream.

**Research:** Confirm which response fixtures can be redistributed (recorded from the owner's own
runs, redacted) and reuse the Phase 6 contract-fixture layout.

**Acceptance Criteria:**

- [ ] The fake provider runs in-process in `cargo test` with no network access.
- [ ] Scenarios cover JSON, SSE with split frames, errors, timeouts and cancellation.
- [ ] Cancellation tests prove the proxy closes the upstream and releases resources.
- [ ] Fixtures carry no secrets (checked by the redactor and gitleaks).

**Dependencies:** None  
**Reference:** `docs/codex/11-remediation-roadmap.md:32-36`; plan Phase 8 Sprint 2  
**Estimated Complexity:** M

---

### T-82-002 — Incremental SSE parser

**Description:** Replace buffered SSE handling with an incremental parser that transforms and
forwards events as they arrive, preserving byte-exact pass-through for events it does not change.

**Research:** Confirm the SSE line rules (CRLF, comments, multi-line `data:`) against the WHATWG
specification and both providers' streams. Hypothesis: incremental parsing lowers time to first
byte and peak memory for long streams. Measurement: fake-provider runs before and after, published
in `docs/numbers.md`.

**Acceptance Criteria:**

- [ ] The parser handles frames split at every byte offset in a property test.
- [ ] Unchanged events are forwarded byte-identical.
- [ ] Peak memory no longer grows with stream length (measured on the fake provider).
- [ ] Time to first byte and peak memory before and after are measured and published in
  `docs/numbers.md`.
- [ ] A fuzz target for the parser is added (shared with T-10-6-009).

**Dependencies:** T-82-001  
**Reference:** `docs/claude/11-enhancements-and-optimizations.md` §C; WHATWG HTML server-sent
events (<https://html.spec.whatwg.org/multipage/server-sent-events.html>)  
**Estimated Complexity:** L

---

### T-82-003 — HTTP/2 to the upstream

**Description:** Use HTTP/2 between the proxy and the provider where the provider supports it, with
HTTP/1.1 fallback, so concurrent agent requests share connections.

**Research:** Confirm provider HTTP/2 support and the chosen client's ALPN behaviour. Hypothesis:
connection reuse lowers per-request latency under concurrent agents. Measurement: fake-provider
runs with concurrent clients, HTTP/1.1 against HTTP/2. A rejected hypothesis closes the ticket with
the finding.

**Acceptance Criteria:**

- [ ] ALPN negotiates HTTP/2 when available and falls back to HTTP/1.1 otherwise.
- [ ] Cancellation of one stream does not affect others on the same connection.
- [ ] Latency under concurrency is measured and published in `docs/numbers.md`.

**Dependencies:** T-82-001, T-82-002  
**Reference:** `ref-docs/research-report.md:7`; plan Phase 8 Sprint 2  
**Estimated Complexity:** M

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Line coverage is `>= 95%` (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Affected docs and `CHANGELOG.md` `[Unreleased]` are updated.
- [ ] Upstream re-scan for Sprint 3 is done and its tickets are adjusted.
