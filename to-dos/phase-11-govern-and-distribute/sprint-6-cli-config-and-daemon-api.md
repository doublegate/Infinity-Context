# Sprint 6 — CLI, Config and Daemon API

**Phase:** Phase 11 — Govern and Distribute  
**Release:** v1.9.0  
**Sprint Goal:** Close the CLI and configuration gaps (clap derive, `config.toml` with
`icx config`, a deeper `icx doctor`, `icx gc`) and add a versioned daemon JSON-RPC API that the
proxy and pager share and that Phase 12 uses as the `icx-shm` socket-routing comparison.  
**Estimated Duration:** 2 weeks

## Research gate

Before any ticket starts:

- Check the current `clap`, `clap_complete` and `clap_mangen` versions and derive API via
  Context7.
- Inventory every flag form the hand-written parser accepts today, including the v1.3.1
  flag-value fixes and `set_option_once` behaviour, as the compatibility baseline.
- Re-read claude/11 §A.1 (shared index between proxy and pager) and ADR-0005 (daemon).
- Re-read ADR-0007's "route through the daemon's Unix socket" alternative and issue #32, so the
  API carries what the comparison needs.
- Re-measure the daemon hook median and record it.

### Research notes

_None recorded yet._

## Tickets

### T-11-6-001 — Move the CLI to clap derive

**Description:** Replace the hand-written parser and its `set_option_once` helpers with clap
derive, gaining `--version`, per-subcommand `--help`, shell completions and a man page.

**Research:** Confirm clap reproduces every accepted flag form from the inventory, including
repeated-flag errors.

**Acceptance Criteria:**

- [ ] A compatibility test passes for every flag form in the inventory.
- [ ] `icx --version`, `icx <cmd> --help`, `icx completions <shell>` and a generated man page work.
- [ ] Hook invocation latency does not regress: measured and published in `docs/numbers.md`.

**Dependencies:** None  
**Reference:** Plan Phase 11 Sprint 6; `crates/icx-cli/src/main.rs`  
**Estimated Complexity:** L

---

### T-11-6-002 — `config.toml` and `icx config get/set/show`

**Description:** Extend the Sprint 1 loader to cover every env var, with precedence
env > file > default, and add `icx config get/set/show` (show reports each value's source).

**Research:** Re-confirm the env-var inventory from T-11-1-004 against the current code.

**Acceptance Criteria:**

- [ ] Every env var has a config key, except the documented env-only security settings.
- [ ] `icx config show` reports value and source for each key (tested).
- [ ] Configuration reference generated for Sprint 8.

**Dependencies:** T-11-1-004, T-11-6-001  
**Reference:** Plan Phase 11 Sprint 6  
**Estimated Complexity:** M

---

### T-11-6-003 — Deeper `icx doctor` with `--json`

**Description:** Extend `icx doctor` to check that the daemon and socket are alive, the proxy is
reachable, hook commands point at this `icx`, the MCP server is registered and tool approval is
present for each agent, and emit `--json` output.

**Research:** Confirm where each agent stores hooks, MCP registration and approvals on the
installed versions.

**Acceptance Criteria:**

- [ ] Each check has a pass and a fail test.
- [ ] `--json` output has a documented, versioned schema.

**Dependencies:** T-11-6-001, T-11-5-003  
**Reference:** Plan Phase 11 Sprint 6  
**Estimated Complexity:** M

---

### T-11-6-004 — `icx gc`: CAS retention and size cap

**Description:** Garbage-collect unreferenced CAS objects and enforce a configurable size cap,
honouring policy retention and never removing a pinned or referenced object. Dry-run by default.

**Research:** Confirm reachability from DAG roots, fold summaries and the bus covers every live
reference.

**Acceptance Criteria:**

- [ ] Referenced objects survive gc; unreferenced ones are removed (tested).
- [ ] The size cap is enforced oldest-first within policy retention.
- [ ] `icx doctor` integrity check passes after gc.

**Dependencies:** T-11-1-002  
**Reference:** Plan Phase 11 Sprint 6  
**Estimated Complexity:** M

---

### T-11-6-005 — Versioned daemon JSON-RPC API

**Description:** Expose a versioned JSON-RPC API on the daemon's Unix socket (deref, status,
recall, MCP proxying), so the proxy and pager share one index (claude/11 §A.1). The socket keeps
owner-only permissions.

**Research:** Pick the framing (newline-delimited JSON-RPC 2.0 or length-prefixed) after checking
what the existing daemon protocol uses; confirm concurrent-client behaviour.

**Acceptance Criteria:**

- [ ] API version negotiated on connect; unknown versions refused cleanly.
- [ ] Proxy and pager use the shared index through the API (integration test).
- [ ] Schema documented in `docs/interfaces.md`.

**Dependencies:** None  
**Reference:** Plan Phase 11 Sprint 6; claude/11 §A.1; ADR-0005  
**Estimated Complexity:** L

---

### T-11-6-006 — Event-routing method for the `icx-shm` comparison

**Description:** Add publish and subscribe methods for cross-agent events to the JSON-RPC API,
so Phase 12 can measure ADR-0007's daemon-socket alternative side by side with the shm bus.

**Research:** Confirm the event shape matches `TelepathyEvent` and that loss reporting can be
expressed (`dropped_publishes`, lapped polls).

**Acceptance Criteria:**

- [ ] Events published over the API are received by a second client in order (tested).
- [ ] A benchmark entry point exists for Phase 12 Sprint 4; no figure is claimed here.

**Dependencies:** T-11-6-005  
**Reference:** Plan Phase 11 Sprint 6; issue #32; `docs/adr/0007-opt-in-icx-shm-bus.md`
(Alternatives considered)  
**Estimated Complexity:** M

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Coverage >= 95% (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Docs and `CHANGELOG.md` (`[Unreleased]`) updated in the same change.
- [ ] Upstream re-scan for Sprint 7 done, its tickets adjusted, and findings recorded in its
  Research notes.
