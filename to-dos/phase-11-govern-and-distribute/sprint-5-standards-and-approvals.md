# Sprint 5 — Standards and Approvals

**Phase:** Phase 11 — Govern and Distribute  
**Release:** v1.9.0  
**Sprint Goal:** Name the Phase 8 traces with OpenTelemetry GenAI/MCP semantic conventions pinned
to one version, document the managed-settings hooks policy, and pre-approve the `icx` tool at
install time where each agent supports it.  
**Estimated Duration:** 1 week

## Research gate

Before any ticket starts:

- Re-read the OpenTelemetry GenAI and MCP semantic conventions; note their stability status and
  pick the version to pin.
- Re-read Claude Code managed settings (`allowManagedHooksOnly`) on the installed version.
- Re-check, per agent (Claude Code, Codex, `agy`, Hermes, Muse), how an MCP tool can be
  pre-approved and where the rule lives; confirm with REA where the docs are silent.
- Re-read the repeated-prompt issue noted in the plan's Immediate fixes, to confirm the root cause
  is a missing user-level allow rule.

### Research notes

_None recorded yet._

## Tickets

### T-11-5-001 — OTel GenAI/MCP span names on the Phase 8 traces

**Description:** Rename the Phase 8 per-hook spans and attributes to the OpenTelemetry GenAI and
MCP semantic conventions, pinned to one semconv version recorded in the docs, with an optional
OTLP exporter that is off by default.

**Research:** Confirm the chosen semconv version's span and attribute names, and how other tools
map MCP tool calls onto them.

**Acceptance Criteria:**

- [ ] Span and attribute names match the pinned version (tested against a fixture).
- [ ] Exporter is opt-in and listed in the network-call inventory (T-11-1-005).
- [ ] Pinned semconv version documented in `docs/interfaces.md`.

**Dependencies:** T-11-1-005, T-83-001  
**Reference:** Plan Phase 11 Sprint 5; OpenTelemetry GenAI and MCP semantic conventions  
**Estimated Complexity:** M

---

### T-11-5-002 — Document the managed-settings hooks policy

**Description:** Explain how `icx` behaves in organisations that set `allowManagedHooksOnly` and
how an administrator deploys the `icx` hooks through managed settings.

**Research:** Verify the setting's behaviour on the installed Claude Code version with a test
managed-settings file.

**Acceptance Criteria:**

- [ ] `docs/deployment-and-agent-integration-guide.md` has a managed-settings section with a
  tested example.
- [ ] `icx doctor` reports when hooks are blocked by managed policy.

**Dependencies:** None  
**Reference:** Plan Phase 11 Sprint 5; Claude Code managed settings documentation  
**Estimated Complexity:** S

---

### T-11-5-003 — Tool pre-approval for plugin-packaged installs

**Description:** Extend T-65-005 (`icx install` pre-approval) to the plugin and package-manager
installs of Sprint 4: when icx is installed as a Claude Code or Codex plugin, add the allow rule that pre-approves the
`icx` MCP tool wherever the agent supports it, so a missing user-level rule no longer causes
repeated permission prompts. The change is shown and confirmed before it is written.

**Research:** For each agent, confirm the allow-rule syntax and file on the installed version.

**Acceptance Criteria:**

- [ ] After `icx init`, the `icx` tool is approved without prompting on each supporting agent
  (tested per agent where automation allows).
- [ ] Agents without such support are listed in the guide.
- [ ] The edit is reversible by `icx init --uninstall` (tested).

**Dependencies:** T-65-005, T-11-4-001  
**Reference:** Plan Phase 11 Sprint 5; plan "Immediate fixes" (repeated prompts); review of PR #39  
**Estimated Complexity:** M

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Coverage >= 95% (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Docs and `CHANGELOG.md` (`[Unreleased]`) updated in the same change.
- [ ] Upstream re-scan for Sprint 6 done, its tickets adjusted, and findings recorded in its
  Research notes.
