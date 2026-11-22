# Sprint 3 — New Adapters: Gemini CLI, Cursor and OpenCode

**Phase:** Phase 6 — Agent Reach and Output Shrinking

**Release:** v1.4.0

**Sprint Goal:** Add hook adapters for Gemini CLI, Cursor and OpenCode, each verified on an
installed version, with contract tests and a deployment-guide section.

**Estimated Duration:** 2 weeks

## Research gate

Re-check before the sprint starts:

- **External docs and versions:** Gemini CLI hooks reference (`AfterTool`, `BeforeModel`,
  `PreCompress`); Cursor hooks reference (`postToolUse`, `updated_mcp_tool_output`); OpenCode
  plugin API (`tool.execute.after`, `experimental.session.compacting`); the latest release of
  each.
- **Installed agent behaviour:** install each agent locally, record its version, and capture a
  raw hook payload for every event the adapter will handle.
- **Project evidence:** the `AgentHookAdapter` trait and existing adapters in
  `crates/icx-adapters`; ADR-0006 compatibility rules; the conformance tests in
  `crates/icx-adapters/src/lib.rs`.

### Research notes

_None recorded yet._

## Tickets

### T-63-001 — Gemini CLI adapter

**Description:** Implement a Gemini CLI adapter: `AfterTool` deny with `tailToolCallRequest`
for output shrinking, `BeforeModel` for context injection, and `PreCompress` for epoch sealing.

**Research:** Hypothesis: `AfterTool` deny plus `tailToolCallRequest` lets icx replace a large
tool result with a stub that the model treats as the result. Confirmed by a live session on the
installed Gemini CLI showing the stub in the model-visible history; rejected otherwise, and the
adapter ships ingest-only for that event.

**Acceptance Criteria:**

- [ ] Installed version and raw payloads recorded as fixtures.
- [ ] Adapter implements the three events; unknown fields are tolerated.
- [ ] Contract tests pin input parsing and reply shapes.
- [ ] Capability (shrinks or ingest-only) stated per event and tested.

**Dependencies:** T-61-007

**Reference:** Plan Phase 6 Sprint 3; Gemini CLI hooks documentation

**Estimated Complexity:** L

---

### T-63-002 — Cursor adapter

**Description:** Implement a Cursor adapter on `postToolUse`, returning
`updated_mcp_tool_output` for MCP tool results over the stub threshold.

**Research:** Hypothesis: Cursor applies `updated_mcp_tool_output` to the model-visible result.
Confirmed by a live Cursor session where the model quotes the stub, not the raw output;
rejected otherwise, and the adapter ships ingest-only.

**Acceptance Criteria:**

- [ ] Installed version and raw payloads recorded as fixtures.
- [ ] Contract tests pin the reply shape; a silent-ignore test exists.
- [ ] Non-MCP tool results are ingested without replacement unless the research shows support.

**Dependencies:** T-61-007

**Reference:** Plan Phase 6 Sprint 3; Cursor hooks documentation

**Estimated Complexity:** M

---

### T-63-003 — OpenCode plugin adapter

**Description:** Ship an OpenCode plugin that calls `icx hook` from `tool.execute.after` to
shrink output, and from `experimental.session.compacting` to seal the epoch.

**Research:** Confirm on the installed OpenCode that `tool.execute.after` can mutate the result
and that `experimental.session.compacting` is still present (it is marked experimental).
Record the plugin API version.

**Acceptance Criteria:**

- [ ] Plugin source in the repository with a pinned API version.
- [ ] The plugin fails open: if `icx` is missing or errors, the original output passes through.
- [ ] Contract test over recorded event payloads.
- [ ] The experimental event is version-gated and documented as such.

**Dependencies:** T-61-007

**Reference:** Plan Phase 6 Sprint 3; OpenCode plugin documentation

**Estimated Complexity:** M

---

### T-63-004 — Deployment-guide sections and doctor checks for the new adapters

**Description:** Add a section per new agent to `docs/deployment-and-agent-integration-guide.md`
and `docs/agent-adapters-and-interception.md`, and teach `icx doctor` to detect each agent and
its hook registration.

**Research:** Verify every config path and key in the guide on the installed version.

**Acceptance Criteria:**

- [ ] One guide section per agent: install, config, verification, limitations.
- [ ] `icx doctor` reports presence, version and hook status for each new agent.
- [ ] Docs site builds with `zensical build --strict`.

**Dependencies:** T-63-001, T-63-002, T-63-003

**Reference:** `docs/deployment-and-agent-integration-guide.md`;
`docs/agent-adapters-and-interception.md` line 13 (contract coverage statement)

**Estimated Complexity:** M

## Sprint exit

- [ ] Gates green: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and
  coverage `>= 95%`.
- [ ] Docs and CHANGELOG updated for any user-visible change.
- [ ] Upstream re-scan done for Sprint 4 (Claude Code changelog and issues), and its tickets
  adjusted.
