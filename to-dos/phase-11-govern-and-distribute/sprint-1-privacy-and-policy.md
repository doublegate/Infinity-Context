# Sprint 1 — Privacy and Policy

**Phase:** Phase 11 — Govern and Distribute  
**Release:** v1.9.0  
**Sprint Goal:** Let users decide what `icx` never stores and how long it keeps the rest, through
`<private>` spans, a per-project `.icx/policy.toml`, and `icx forget`/`icx purge` with dry-run.
This sprint lands before the viewer and the export.  
**Estimated Duration:** 1.5 weeks

## Research gate

Before any ticket starts:

- Re-read the current store write paths (`crates/icx-core/src/cas.rs`, the daemon ingest path in
  `crates/icx-adapters`) and the Phase 9 storage changes, so the policy check sits at the single
  point every write passes through.
- Re-check how each supported agent (Claude Code, Codex, `agy`, Hermes, Muse) delivers tool input
  and output to hooks on the locally installed version, so `<private>` spans are recognised in
  every payload shape.
- Review comparable privacy controls: claude-mem `<private>` tags, basic-memory and context-mode
  ignore rules, for naming and semantics users already know.
- Re-run `cargo test --workspace` and record the baseline.

### Research notes

_None recorded yet._

## Tickets

### T-11-1-001 — Never store `<private>…</private>` spans

**Description:** Strip every `<private>…</private>` span from tool output, prompts and recall
payloads before chunking, hashing or indexing, so the content never reaches the CAS, the HDC
index, the graph or the bus. The emitted stub states that a private span was removed, without its
length or hash.

**Research:** Confirm how each agent's hook payload encodes the tags (raw text, JSON string,
escaped) on the installed version, and whether nested or unterminated tags occur in practice.
Decide the unterminated-tag rule (fail closed: drop to end of input) and record it.

**Acceptance Criteria:**

- [ ] Content inside `<private>` spans never appears in the CAS, index, graph, bus or logs (tested
  per agent payload shape).
- [ ] An unterminated `<private>` tag drops everything after it (fail closed).
- [ ] The removal is noted in the stub without leaking length or digest.
- [ ] Documented in `docs/security-and-threat-model.md` and `docs/interfaces.md`.

**Dependencies:** None  
**Reference:** Plan Phase 11 Sprint 1; claude-mem `<private>` tag convention;
`docs/security-and-threat-model.md`  
**Estimated Complexity:** M

---

### T-11-1-002 — Per-project policy file `.icx/policy.toml`

**Description:** Define and enforce a per-project policy: denied paths (globs) and tools whose
output is never stored, a skip-store mode (stub-free pass-through), retention and TTL per kind
of artifact. One policy-checked accessor in the store is the only write path; later consumers
(viewer, timeline, export) read through the same layer.

**Research:** Survey ignore-file semantics users know (`.gitignore` globbing, Claude Code
permission rules) and pick the glob crate after checking its current API via Context7. Confirm the
file is read from the trusted workspace only (ADR-0005 trust store).

**Acceptance Criteria:**

- [ ] Schema documented, versioned (`version = 1`), and validated with clear errors on unknown
  keys.
- [ ] Denied paths and tools never reach the CAS (tested).
- [ ] Retention and TTL expire artifacts on schedule (tested with an injected clock).
- [ ] Policy is ignored, with a logged reason, in an untrusted workspace.

**Dependencies:** None  
**Reference:** Plan Phase 11 Sprint 1; `docs/adr/0005-v1-1-0-workspace-trust-store-and-daemon.md`  
**Estimated Complexity:** L

---

### T-11-1-003 — `icx forget` and `icx purge` with dry-run

**Description:** `icx forget <hash|path|query>` removes matching artifacts and their index and
graph entries; `icx purge` removes everything for a project or past a date. Both default to a
dry-run report and act only with an explicit flag, writing an audit entry first.

**Research:** Confirm which structures reference an artifact (CAS, DAG nodes, HDC postings, graph
edges, bus history, fold summaries) so removal leaves no dangling reference; check the Merkle
integrity check's behaviour on a removed leaf.

**Acceptance Criteria:**

- [ ] Dry-run is the default and lists exactly what would be removed.
- [ ] After a real run, `icx deref` on a forgotten hash fails cleanly and recall never returns it.
- [ ] `icx doctor` integrity verification passes after a forget.
- [ ] An audit entry (who, what, when, scope) is written before deletion.

**Dependencies:** T-11-1-002  
**Reference:** Plan Phase 11 Sprint 1; module 60 (destructive operations need confirmation and an
audit entry)  
**Estimated Complexity:** M

---

### T-11-1-004 — Config-file groundwork for env-only settings

**Description:** Introduce the config-file schema and loader for settings that are env-only today,
with precedence env > file > default. Sprint 6 adds the `icx config` CLI and full coverage on top.

**Research:** Inventory every `ICX_*` environment variable read in the workspace (about 27 today)
and its default, and confirm which are security-relevant and must stay env-only.

**Acceptance Criteria:**

- [ ] Loader reads `$ICX_CONFIG_HOME/config.toml` with documented precedence (tested).
- [ ] Security-relevant settings that must not come from a file are listed and enforced.
- [ ] Inventory of env vars committed to `docs/interfaces.md`.

**Dependencies:** None  
**Reference:** Plan Phase 11 Sprints 1 and 6  
**Estimated Complexity:** S

---

### T-11-1-005 — No default telemetry, with a network-call inventory

**Description:** State the telemetry policy: none by default, and any future telemetry opt-in
only. Publish a documented list of every network call `icx` can make (proxy upstreams, registry
or update checks if any), following the context-mode practice.

**Research:** Grep the workspace for every outbound connection and confirm the list is complete;
recheck it at each later sprint that adds a dependency.

**Acceptance Criteria:**

- [ ] `docs/security-and-threat-model.md` lists every network call with its trigger.
- [ ] A test or CI check fails if a new outbound client is added without updating the list.

**Dependencies:** None  
**Reference:** Plan Phase 11 "Optional telemetry"; context-mode telemetry practice  
**Estimated Complexity:** S

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Coverage >= 95% (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Docs and `CHANGELOG.md` (`[Unreleased]`) updated in the same change.
- [ ] Upstream re-scan for Sprint 2 done, its tickets adjusted, and findings recorded in its
  Research notes.
