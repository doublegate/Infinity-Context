# Sprint 3 — Compaction

**Phase:** Phase 7 — Retrieval and Compaction Quality

**Release:** v1.5.0

**Sprint Goal:** Give the agent a resume packet at compaction, and let the agent evict context
it no longer needs through typed `fold` operations.

**Estimated Duration:** 2 weeks

## Research gate

Re-check before the sprint starts:

- **External docs and versions:** Claude Code and Codex `PreCompact`/`PostCompact` payloads and
  what a reply may inject; the Hermes `select_context` interface; the papers CWL
  (arXiv 2606.11213), CAT (arXiv 2512.22087) and SWE-MeM (arXiv 2606.28434), and any code they
  publish.
- **Installed agent behaviour:** on each installed agent, what the model sees after compaction
  and whether injected text survives into the next turn; the T-61-002 and T-61-005 findings.
- **Project evidence:** the T-64-008 decision on the `fold` tool; the current fold and expand
  implementation; Phase 6 contract fixtures for the compaction events.

### Research notes

**2026-11-22, from Phase 6 Sprint 1**
(`ref-docs/agent-internals/below-hook-findings-2026-11-22.md`). Re-check each item at sprint
start.

- **Claude Code 2.1.330:**
  - PreCompact stdout is appended to the summarizer's instructions, up to 10,000 characters, so
    an instruction to keep `#icx:` handles reaches the summary.
  - PostCompact output reaches the user only, so the resume packet must be re-injected from the
    SessionStart that follows compaction (T-73-002).
- **Codex 0.171.0:**
  - No hook can change the compaction summary; only `compact_prompt` and
    `experimental_compact_prompt_file` can.
  - PreCompact is useful only to seal an epoch and take a snapshot (T-73-002).
- **Hermes `caafe5389ae`:**
  - `on_session_start` is called after every committed compaction, with
    `boundary_reason="compression"`.
  - A request-only `select_context` then carries the packet without persisting it, and fails
    open (T-73-003).
- **agy 1.5.14 and Muse 1.4.25:**
  - agy has no compaction hook;
  - Muse has PreCompact and PostCompact, but their payloads are undocumented.
  - Neither is in scope for this sprint.

## Tickets

### T-73-001 — Resume packet format and byte budget

**Description:** Define a resume packet: the most relevant `#icx:` handles with one-line
summaries, open tasks, and the current fold tree, rendered deterministically within a byte
budget.

**Research:** Decide the budget per agent from its documented context-injection limit (for
Claude Code, the `additionalContext` limit handled in T-64-006).

**Acceptance Criteria:**

- [ ] Format specified in `docs/virtual-memory-and-dag-engine.md`.
- [ ] Property test: the packet never exceeds the budget and every handle dereferences.
- [ ] Rendering is deterministic for the same store state.

**Dependencies:** T-64-006

**Reference:** Plan Phase 7 Sprint 3; `docs/virtual-memory-and-dag-engine.md` (Context-Folding)

**Estimated Complexity:** M

---

### T-73-002 — Inject the resume packet in Claude Code and Codex

**Description:** Deliver the resume packet after compaction through the event each agent applies
to the post-compaction context. Sprint 1 found that neither compaction hook can carry it to the
model:

- **Claude Code:** `PreCompact` stdout tells the summarizer to keep every `#icx:` handle, and the
  packet itself goes in the `additionalContext` of the `SessionStart` that follows compaction.
  `PostCompact` output reaches the user only.
- **Codex:** no hook can change the summary, so `PreCompact` only seals the epoch and takes a
  snapshot. The packet goes in the `additionalContext` of the first hook after compaction that
  carries it (`SessionStart` if Codex fires one after compaction, otherwise the next
  `UserPromptSubmit`), within `CODEX_INLINE_CONTEXT_MAX_BYTES` (T-62-006).

**Research:**

- **Re-confirm the routes on the installed versions:** Claude Code's post-compaction
  `SessionStart`, and which Codex event first follows compaction.
- **Hypothesis:** a resume packet delivered after compaction reduces re-reads of
  already-seen artifacts in the turns that follow. Measured on recorded sessions with and
  without the packet, counting repeated tool calls for the same file or command.

**Acceptance Criteria:**

- [ ] The event and field that carry the packet are recorded per agent version.
- [ ] Contract tests over live-run fixtures.
- [ ] Re-read counts with and without the packet measured and published in `docs/numbers.md`.
- [ ] Opt-out switch documented.

**Dependencies:** T-73-001, T-65-001

**Reference:** Plan Phase 7 Sprint 3; `docs/agent-adapters-and-interception.md` (PreCompact and
PostCompact sections)

**Estimated Complexity:** M

---

### T-73-003 — Resume packet through Hermes `select_context`

**Description:** Extend the Hermes shim to supply the resume packet through `select_context`.

**Research:** Confirm the `select_context` signature on the installed Hermes commit.

**Acceptance Criteria:**

- [ ] Shim returns the packet within the budget; fails open.
- [ ] Live verification recorded; fixture added.

**Dependencies:** T-73-001, T-65-002

**Reference:** Plan Phase 7 Sprint 3; ICX-AUD-006

**Estimated Complexity:** S

---

### T-73-004 — Agent-directed typed eviction through `fold`

**Description:** Let the agent fold context by type (for example finished subtask, stale build
log, superseded file read), keeping a handle and a summary, following the CWL, CAT and SWE-MeM
eviction patterns.

**Research:** Hypothesis: typed eviction lowers context size per turn without lowering task
success. Measured on recorded sessions as active-context bytes and on a small task set as
success rate, with and without typed eviction. If success drops, the ticket closes with the
finding.

**Acceptance Criteria:**

- [ ] Types and their semantics specified. Per the T-64-008 decision (ADR-0009), typed folds are
  an operation of the state-changing `icx_write` tool, never of the read-only `icx` tool.
- [ ] Every fold is reversible with `expand`; no content is lost (Merkle-verified test).
- [ ] Context size and success rate measured and published in `docs/numbers.md`.

**Dependencies:** T-64-008, T-73-001

**Reference:** CWL arXiv 2606.11213; CAT arXiv 2512.22087; SWE-MeM arXiv 2606.28434

**Estimated Complexity:** L

## Sprint exit

- [ ] Gates green: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and
  coverage `>= 95%`.
- [ ] Docs and CHANGELOG updated for any user-visible change.
- [ ] Upstream re-scan done for Sprint 4 (secret-scanner rule sets, redaction literature), and
  its tickets adjusted.
