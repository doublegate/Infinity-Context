# Sprint 4 — MCP 2026-07-28 Alignment

**Phase:** Phase 8 — Prompt-Cache Proxy and Observability  
**Release:** v1.6.0  
**Sprint Goal:** Align the single-tool `icx` MCP pager with the 2026-07-28 MCP specification:
discovery, structured output with cache hints, a stable tool order, and no dependence on optional
client features.  
**Estimated Duration:** 1 week

## Research gate

Re-check before any ticket starts:

- **External docs and versions:** the MCP 2026-07-28 specification and changelog
  (`server/discover`, `_meta`, `outputSchema`, `structuredContent`, `ttlMs`, `cacheScope`,
  resources), and any later draft that changes them.
- **Installed agent behaviour:** for each installed client (Claude Code, Codex, agy, Hermes, Muse),
  whether it calls `server/discover`, reads `structuredContent`, honours `ttlMs` / `cacheScope`,
  lists resources, and whether tool order affects its prompt cache. Use REA where docs are silent.
- **Project evidence:** the current `McpPagerServer` and what of `server/discover` and `_meta` is
  already implemented; MCP conformance tests in `icx-adapters`.

### Research notes

_None recorded yet._

## Tickets

### T-84-001 — Complete `server/discover` and per-request `_meta`

**Description:** Finish the partly done `server/discover` response and carry per-request `_meta`
(provenance, trust, exactness) on every tool result.

**Research:** Diff the current implementation against the specification and list the missing
fields.

**Acceptance Criteria:**

- [ ] `server/discover` returns every field the specification requires.
- [ ] Every tool result carries `_meta` with provenance and trust markers.
- [ ] Conformance tests cover both, against the published schema.

**Dependencies:** None  
**Reference:** MCP specification 2026-07-28 (<https://modelcontextprotocol.io/specification>);
plan Phase 8 Sprint 4  
**Estimated Complexity:** S

---

### T-84-002 — `outputSchema` and `structuredContent` for deref, slice, grep and recall

**Description:** Declare an `outputSchema` per operation and return `structuredContent` for
`deref`, `slice`, `grep` and `recall`, keeping the text content block for clients that do not read
structured output.

**Research:** Confirm per client whether structured output is consumed, and how a single
polymorphic tool should declare per-operation schemas.

**Acceptance Criteria:**

- [ ] Each operation's structured result validates against its declared schema in tests.
- [ ] The text block remains and matches the structured content.
- [ ] Clients that ignore structured output behave as before (manual check per installed client).

**Dependencies:** T-84-001  
**Reference:** MCP specification 2026-07-28, tools section  
**Estimated Complexity:** M

---

### T-84-003 — `ttlMs` and `cacheScope` hints

**Description:** Set `ttlMs` and `cacheScope` on results whose content is immutable (a dereference
of a content hash) or session-scoped (recall), so clients can cache them correctly.

**Research:** Confirm the hint semantics and which installed clients honour them.

**Acceptance Criteria:**

- [ ] Content-addressed results are marked cacheable; recall results are marked session-scoped.
- [ ] Hints never mark mutable results as long-lived (test per operation).

**Dependencies:** T-84-002  
**Reference:** MCP specification 2026-07-28  
**Estimated Complexity:** S

---

### T-84-004 — Deterministic tool order

**Description:** Return tools, schemas and properties in a fixed order so the tool list is
byte-stable across runs and does not break the client's prompt-cache prefix.

**Research:** Check whether any serialization path uses unordered maps.

**Acceptance Criteria:**

- [ ] `tools/list` output is byte-identical across runs and restarts (test).
- [ ] Schema property order is fixed.

**Dependencies:** None  
**Reference:** plan Phase 8 Sprint 4  
**Estimated Complexity:** S

---

### T-84-005 — No dependence on roots or sampling

**Description:** Ensure the server works fully when the client offers neither roots nor sampling;
workspace selection keeps using its existing rules.

**Research:** Confirm which installed clients offer roots and sampling, and audit code paths that
read either.

**Acceptance Criteria:**

- [ ] A test client without roots or sampling exercises every operation successfully.
- [ ] No code path requests sampling.

**Dependencies:** None  
**Reference:** plan Phase 8 Sprint 4  
**Estimated Complexity:** S

---

### T-84-006 — Optionally expose stubs as resources

**Description:** Optionally list stored artifacts referenced by stubs as MCP resources
(`icx://<hash>`), off by default, so clients with resource support can fetch them without a tool
call.

**Research:** Hypothesis: resource access lowers tool-call count for clients that support it.
Measurement: count tool calls on a replayed session with the option on and off. A rejected
hypothesis closes the ticket with the finding.

**Acceptance Criteria:**

- [ ] The option is off by default and documented.
- [ ] Resource reads return the same bytes and provenance as `deref`.
- [ ] Any tool-call difference is measured and published in `docs/numbers.md`.

**Dependencies:** T-84-001  
**Reference:** MCP specification 2026-07-28, resources section  
**Estimated Complexity:** M

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Line coverage is `>= 95%` (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Affected docs and `CHANGELOG.md` `[Unreleased]` are updated.
- [ ] Upstream re-scan for Phase 9 Sprint 1 is done and its tickets are adjusted.
