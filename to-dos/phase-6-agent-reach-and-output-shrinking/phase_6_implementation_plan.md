# Phase 6 (`v1.4.0`) Implementation Plan — Agent Reach and Output Shrinking

## Goal Description

Phase 6 turns the adapter layer from "documented contracts plus unit tests" into "contracts
verified on the installed agents". It also widens agent coverage and removes the last places
where icx ingests output but cannot shrink it. Each sprint lands as its own pull request against
`main`, and the release is cut at the phase close.

## Ordered Approach

1. **Sprint 1 — Agent internals investigation.** Run first, because its findings decide what
   the later sprints can rely on. Start with a scope and terms review per vendor. Then inspect
   the installed Claude Code, Codex, agy, Muse and Hermes with REA (Ghidra where needed) for
   internal event emitters, tool-result shapes before display, compaction internals, local IPC
   and session databases. Record the findings in a new dated file under
   `ref-docs/agent-internals/` and take a go/no-go decision per below-hook integration.
2. **Sprint 2 — Codex output shrinking.** Verify on the installed Codex whether the documented
   `decision:"block"` whole-result substitution works. Implement it behind a flag only if it
   does. Wire `CodexRolloutTailer` into the CLI and add `.jsonl.zst` support. Keep a watch ticket
   open for upstream `updatedMCPToolOutput` support.
3. **Sprint 3 — New adapters.** Gemini CLI, Cursor and OpenCode, each with contract tests and a
   deployment-guide section. Each adapter starts with a research ticket on the installed version.
4. **Sprint 4 — Claude Code depth.** Shape-exact `updatedToolOutput` for every built-in tool with
   a silent-ignore detector; the newer lifecycle events; the `additionalContext` budget; and the
   plan-mode permission-prompt test for the icx MCP tool.
5. **Sprint 5 — Install and verification tooling.** Live-run contract fixtures for Codex, agy,
   Hermes and Muse; a shipped Hermes `ContextEngine` shim; `icx install --agent X [--dry-run]`
   including per-agent MCP tool pre-approval; and a capability matrix generated from the
   contract tests.

Sprints 2, 3 and 4 are independent of each other once Sprint 1 has closed. Sprint 5 depends on
all of them, because its fixtures and matrix cover every adapter.

## Research Rule

Every sprint file opens with a research gate, and every ticket has a Research line. Work on a
ticket starts only after its research is recorded in the sprint's dated research notes:

- re-check the external facts it depends on (agent hook docs, MCP spec, crate versions via
  Context7, release notes and issues on GitHub);
- confirm agent-facing behaviour on the locally installed version, with REA where docs are
  silent;
- for a research-backed ticket, state the hypothesis and the measurement; a rejected hypothesis
  closes the ticket with the finding and no feature.

## Risks

- **Vendor terms.** Inspecting a binary may be restricted by its licence. Mitigation: T-61-001
  reviews terms first; the investigation is limited to interoperability with the user's own
  installs; no vendor code or extracted assets enter the repository.
- **Undocumented behaviour drifts.** A below-hook integration can break on any agent update.
  Mitigation: a go/no-go decision per integration, and anything shipped is off by default,
  version-gated and covered by a contract test that fails loudly.
- **Silent ignores.** An agent can accept a hook reply and discard the replacement (Claude Code
  #86753, #67442; Codex `updatedMCPToolOutput`). Mitigation: every replacement path has a test
  that detects whether the model-visible output actually changed.
- **Codex `decision:"block"` semantics.** Blocking may surface as an error to the model rather
  than as a substitute result. Mitigation: T-62-001 measures what the model sees before any code
  is written; if it is an error, the ticket closes with the finding.
- **Permission configs belong to other tools.** Writing another agent's permission file can
  widen what that agent may do. Mitigation: install prints the exact edit in `--dry-run`, never
  writes without explicit confirmation, and backs up the file it edits.
- **Plan-mode prompts.** `readOnlyHint` may not be honoured by the installed Claude Code.
  Mitigation: T-64-007 tests it first; annotations ship only if the test shows they work.

## Verification

- Contract fixtures for every adapter, captured from live runs with the agent version recorded
  in the fixture.
- The capability matrix is generated from those tests, and CI fails if the committed matrix
  differs from the generated one.
- Gates: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, and `cargo llvm-cov --workspace --all-targets --fail-under-lines 95`.
- `docs/agent-adapters-and-interception.md` and
  `docs/deployment-and-agent-integration-guide.md` updated in the same change as each adapter.
- Any performance or saving figure is measured and published in `docs/numbers.md` with its
  method; none is claimed before it is measured.
