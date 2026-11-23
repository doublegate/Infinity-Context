# ADR 0009: A read-only `icx` tool and a separate state-changing tool

Status: accepted, 2026-11-22 UTC. Chosen by the owner in v1.4.0 Phase 6 Sprint 4 (T-64-008).

## Context

The MCP server exposes one tool, `icx`, whose `op` selects an operation. Most operations only read
the workspace: `deref`, `slice`, `grep`, `expand`, `status`, and `xagent` without a message (a
poll). Three change state:

- `fold` opens or closes a branch and writes DAG nodes;
- `xagent` with a message broadcasts on the bus;
- `recall` advances the bus-delivery cursors, which is bookkeeping, not data the user sees.

T-64-007 measured Claude Code 2.1.330 in plan mode: a tool without `annotations.readOnlyHint` is
held, and an allow rule (`permissions.allow: ["mcp__icx"]`) does not lift the hold. With
`readOnlyHint: true` the tool runs. Marking today's single tool read-only would be false, because
`fold` and a broadcast change state, and MCP clients may use the hint to skip confirmation for
any call.

## Decision

Split the tool in two:

- **`icx`**, with `readOnlyHint: true`: `deref`, `slice`, `grep`, `recall`, `expand`, `status`,
  and `xagent` as a poll. `recall` stays here, since its only effect is internal cursor
  bookkeeping.
- **`icx_write`**, without the hint: `fold`, and `xagent` with a message.
- **No deprecation period:** each tool refuses the other's operations from v1.4.0. A write sent
  to `icx` is an error naming `icx_write`, so the hint is never false. (The first draft kept
  writes working on `icx` for one release; review found that this made `readOnlyHint` false
  during that release, and the owner chose to refuse them at once.)
- Both names are treated as icx's own tools by every hook adapter (never ingested), in every
  agent's naming form.

## Consequences

- In Claude Code plan mode, reading icx context (dereferencing stubs, grep, recall) works without
  a prompt, which is where the repeated prompts came from. `fold` is still held in plan mode,
  which suits plan mode, since nothing should be folded before the plan is approved.
- One allow rule, `mcp__icx`, still covers both tools in Claude Code.
- A second tool definition costs prompt tokens on every request: about the size of the current
  schema (under 180 tokens by `schema_token_estimate`), so roughly double.
- Every adapter's pager-name list and the docs gain the second name; Phase 7 Sprint 3 (typed
  eviction through `fold`, T-73-004) targets `icx_write`.
- Agents whose instructions fold with the `icx` tool get one error naming `icx_write`, then
  use it. Instruction files that say so should be updated.
- Rejected:
  - **The hint on the single tool:** false for `fold` and broadcasts, and it would let a client
    skip confirmation for state changes.
  - **No split and no hint:** plan mode keeps holding every icx call, the original complaint.
  - **Moving `recall` to `icx_write`:** its cursor bookkeeping is invisible to the user, and
    recall is the most frequent read.
