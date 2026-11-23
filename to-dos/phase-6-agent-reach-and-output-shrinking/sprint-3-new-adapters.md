# Sprint 3 — New Adapters: Gemini CLI, Cursor and OpenCode

**Phase:** Phase 6 — Agent Reach and Output Shrinking

**Release:** v1.4.0

**Sprint Goal:** Add hook adapters for Gemini CLI, Cursor and OpenCode, each verified on an
installed version, with contract tests and a deployment-guide section.

**Estimated Duration:** 2 weeks

## Research gate

Re-check before the sprint starts:

- **External docs and versions:** Gemini CLI hooks reference (`AfterTool`, `BeforeModel`,
  `PreCompress`); Cursor hooks reference (`postToolUse`, `updated_mcp_tool_output`); OpenCode V2
  plugin API (`ctx.tool.hook("execute.after")`, `ctx.session.hook("context")`,
  `ctx.session.hook("compaction")`); the latest release of each.
- **Installed agent behaviour:** install each agent locally, record its version, and capture a
  raw hook payload for every event the adapter will handle.
- **Project evidence:** the `AgentHookAdapter` trait and existing adapters in
  `crates/icx-adapters`; ADR-0006 compatibility rules; the conformance tests in
  `crates/icx-adapters/src/lib.rs`.

### Research notes

**2026-11-22, research gate:**

- **Installed versions:**
  - **Gemini CLI:** not installed. The owner decided not to install it, so T-63-001 is built
    against the documented contract and marked unverified live. (`~/.gemini/settings.json` on
    this machine belongs to agy.)
  - **Cursor:** `cursor-agent` 2026.09.08-6caf4ff, with a user `~/.cursor/hooks.json`.
  - **OpenCode:** v2.2.7.
- **Gemini CLI** (docs, `google-gemini/gemini-cli` `docs/hooks/reference.md`):
  - **`AfterTool`** receives `tool_name`, `tool_input`,
    `tool_response{llmContent, returnDisplay, error?}`, `mcp_context` and
    `original_request_name`.
    - `decision: "deny"` with `reason` replaces the result the model sees, and the turn
      continues. This, not `tailToolCallRequest`, is the stub route.
    - `tailToolCallRequest` replaces the result with another tool's.
    - `hookSpecificOutput.additionalContext` is appended to the result.
    - Exit code 2 replaces the result with stderr.
  - **`BeforeModel`:** `hookSpecificOutput.llm_request` overrides parts of the request.
  - **`PreCompress`** is advisory only: it is asynchronous and cannot block or modify
    compression. `systemMessage` is shown to the user.
- **Cursor** (docs, cursor.com/docs/hooks):
  - **`postToolUse`** receives `tool_name`, `tool_input`, `tool_output` (a JSON-stringified
    result, not raw terminal text), `tool_use_id`, `cwd`, `duration` and model fields.
  - It returns `updated_mcp_tool_output` (an object, MCP tools only, which replaces the output the
    model sees) and `additional_context`.
  - **`afterShellExecution`** and **`afterMCPExecution`** are observation events.
- **OpenCode V2 plugin API.** v2.2.7 loads V2 plugins. Its binary contains the V2 hook name
  `"execute.after"` and neither V1 name (`tool.execute.after`,
  `experimental.session.compacting`). The working V2 plugin in `~/.config/opencode/plugins/`
  (written by ai-memory) confirms the shape:
  - **Plugin shape:** a default export `{ id, setup(ctx) }` typed `Plugin.Plugin` from
    `@opencode/plugin`; `setup` returns an async dispose function.
  - **Tool results:** `ctx.tool.hook("execute.after", event)`. The event carries `sessionID`,
    `tool`, `id` (the call id), `input`, `status` (`"completed"` or `"error"`),
    `result.{output | content, metadata}` and `error.message`. Per the docs, assigning
    `event.result` replaces the result.
  - **Context injection:** `ctx.session.hook("context", event)` adds system text with
    `event.system.push({type: "text", text})`, for the outgoing model call only.
  - **Compaction:** `ctx.session.hook("compaction", event)` receives `event.messages`, the
    transcript being summarized. Setting `event.result` supplies the summary and skips the model
    call. Live confirmation is part of T-63-003.
  - **Live results, 2026-11-22.** OpenCode v2.2.7, model `claude-agent/haiku`, isolated
    scratch project and icx workspace, plugin loaded from the project's `.opencode/plugins/`,
    `opencode run --standalone --archived`:
    - **Prompt append: verified.** The plugin loaded and `prompt` fired. The model quoted the
      `⟪ICX:TAIL_SLOT│agent=open-code…⟫` line back from its own message, so the append reaches
      it as user text.
    - **Tool-result stub: verified.** With `ICX_OPENCODE_STUB=1` and `seq 1 3000`, the model's
      tool result began with the icx stub, and it reported that 2999 was not in what it
      received, so assigning `event.result` replaces what the model sees. The handle
      dereferenced to OpenCode's own shortened output: the last 2,000 lines plus its notice
      that the full output was saved to a file. OpenCode shortens shell output before
      `execute.after`, so icx stores what the model would otherwise have seen.
    - **`compaction`: not exercised.** A one-shot run never compacts. The hook only seals the
      epoch.
    - **Model access:** the Antigravity (Google) models returned "Forbidden", both in
      `--standalone` mode and through the background service; GitHub Copilot's monthly quota was
      exhausted; and the OpenAI models tried returned errors. All of these are provider-side,
      since a control run with the plugin failing open failed the same way.
  - **Cursor live capture, 2026-11-22** (`cursor-agent` 2026.09.08-6caf4ff, after re-login):
    - Project-level `.cursor/hooks.json` loads, and `sessionStart` and `sessionEnd` fired. Their
      payloads match the documented common fields: `conversation_id`, `generation_id`, `model`,
      `hook_event_name`, `cursor_version`, `workspace_roots`, `user_email` and
      `transcript_path` (null), plus `session_id` and `is_background_agent` (and on
      `sessionEnd`, `reason`, `final_status` and `duration_ms`).
    - The tool-call capture could not run: "Weekly usage limit reached. It resets in 2 days."
      `postToolUse`, the MCP name form and `updated_mcp_tool_output` stay unverified.
  - **V1 to V2 mapping** (from the docs): `tool.execute.after` → `ctx.tool.hook("execute.after")`,
    `experimental.session.compacting` → `ctx.session.hook("compaction")`,
    `experimental.chat.system.transform` → `ctx.session.hook("context")`.

## Tickets

### T-63-001 — Gemini CLI adapter

**Description:** Implement a Gemini CLI adapter from the documented contract:

- `AfterTool` ingests the result, and in an opt-in mode replies `decision: "deny"` with the
  stub as `reason`, which the docs say replaces the result the model sees;
- `BeforeModel` (or `SessionStart`/`BeforeAgent` `additionalContext`, whichever the docs
  support) injects context;
- `PreCompress` seals the epoch, observation only, since it is advisory.

**Gemini CLI is not installed** (owner decision, 2026-11-22), so this adapter is not verified
live.

**Research:** Hypothesis: `AfterTool` `decision: "deny"` plus `reason` replaces a large tool
result with a stub that the model treats as the result, not as an error. The documented route
is `deny`, not `tailToolCallRequest`, which substitutes another tool's result. It cannot be
confirmed without an install. So the shrinking mode ships off by default, and its capability is
"documented, unverified" until someone verifies it on a live install. Watch for whether
Gemini CLI also runs tools in a script, which defeated the equivalent Codex route (T-62-001).

**Acceptance Criteria:**

- [x] Fixtures built from the documented payload examples, each labelled with the docs revision
  it came from (no live capture: not installed).
- [x] Adapter implements the three events; unknown fields are tolerated.
- [x] Contract tests pin input parsing and reply shapes.
- [x] Capability per event stated and tested. `AfterTool` shrinking is "documented, unverified"
  and off by default until live-verified.

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

- [x] Installed version and the live `sessionStart`/`sessionEnd` fields recorded. The tool-call
  payloads are deferred to v1.5.0 (#89), blocked by Cursor's usage limit.
- [x] Contract tests pin the reply shapes. The silent-ignore check for `updated_mcp_tool_output`
  is deferred to v1.5.0 (#89); the MCP replacement stays opt-in until then.
- [x] Non-MCP tool results are ingested without replacement unless the research shows support.

**Dependencies:** T-61-007

**Reference:** Plan Phase 6 Sprint 3; Cursor hooks documentation

**Estimated Complexity:** M

---

### T-63-003 — OpenCode plugin adapter

**Description:** Ship an OpenCode V2 plugin (`{ id, setup(ctx) }`, typed `Plugin.Plugin` from
`@opencode/plugin`) that calls `icx hook --agent opencode`. It registers:

- `ctx.tool.hook("execute.after")` to ingest each completed result and, over the stub threshold,
  replace `event.result` with the stub;
- `ctx.session.hook("prompt")` to append the legend and tail slot to the user's message. This
  replaced the planned `context` hook (system text) on 2026-11-22: icx context is untrusted data
  and is never written as system text, and per-request system text would defeat the provider's
  prompt cache;
- `ctx.session.hook("compaction")` to seal the epoch. It never sets `event.result`, so OpenCode
  keeps writing its own summary.

**Research:**

- **Confirm on the installed OpenCode v2.2.7:**
  - that assigning `event.result` in `execute.after` replaces what the model sees;
  - whether the result text is in `output` or `content`;
  - that `context` and `compaction` fire with the fields recorded above.
- **Record** the plugin package version and OpenCode version.
- **Watch for script execution:** whether OpenCode runs tools in a script, the Code Mode effect
  that defeated the Codex route (T-62-001).

**Acceptance Criteria:**

- [x] Plugin source in the repository, targeting the V2 API, with the OpenCode version it was
  verified on (v2.2.7). It needs no plugin package at run time (a type-only import).
- [x] The plugin fails open: if `icx` is missing, errors or times out, the original result and
  context pass through unchanged (bun tests).
- [x] `icx hook --agent opencode` parses the plugin's event payloads. Contract tests cover the
  payloads the plugin builds; the live v2.2.7 run exercised the same path end to end.
- [x] Live check recorded: the model quoted the stub, not the raw output, for a large result.
- [x] No V1 hook names in the plugin, its tests or the adapter. They appear only in notes that
  record their absence (this file, the phase overview and the adapters spec); the guide
  documents the V2 requirement.

**Dependencies:** T-61-007

**Reference:** Plan Phase 6 Sprint 3; OpenCode V2 plugin documentation
(`anomalyco/opencode` `services/site/src/docs/content/build/plugins/server.mdx`,
`migrate-v1.mdx`)

**Estimated Complexity:** M

---

### T-63-004 — Deployment-guide sections and doctor checks for the new adapters

**Description:** Add a section per new agent to `docs/deployment-and-agent-integration-guide.md`
and `docs/agent-adapters-and-interception.md`, and teach `icx doctor` to detect each agent and
its hook registration.

**Research:** Verify every config path and key in the guide on the installed version.

**Acceptance Criteria:**

- [x] One guide section per agent: install, config, verification, limitations (§2.7 to §2.9).
  Config keys checked on the installed versions where installed: Cursor `hooks.json` and
  `mcp.json`, OpenCode plugin path and `mcp.servers`. Gemini CLI's come from its docs.
- [x] `icx doctor` reports presence, version and hook status for each new agent.
- [x] Docs site builds with `zensical build --strict` (2026-11-22, no issues).

**Dependencies:** T-63-001, T-63-002, T-63-003

**Reference:** `docs/deployment-and-agent-integration-guide.md`;
`docs/agent-adapters-and-interception.md` line 13 (contract coverage statement)

**Estimated Complexity:** M

## Sprint exit

- [x] Gates green: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and
  coverage `>= 95%` (2026-11-22: 407 tests, 95.38% lines; the new adapters 96.8% to 99.1%).
- [x] Docs and CHANGELOG updated for any user-visible change.
- [ ] Upstream re-scan done for Sprint 4 (Claude Code changelog and issues), and its tickets
  adjusted.
