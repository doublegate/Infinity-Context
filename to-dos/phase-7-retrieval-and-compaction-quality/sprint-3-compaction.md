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

_None recorded yet._

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

**Description:** Emit the resume packet from `PreCompact` and/or `PostCompact` for Claude Code
and Codex, whichever the installed version applies to the post-compaction context.

**Research:** Hypothesis: a resume packet injected at compaction reduces the number of
re-reads of already-seen artifacts in the turns after compaction. Measured on recorded sessions
with and without the packet, counting repeated tool calls for the same file or command.

**Acceptance Criteria:**

- [ ] The event and field that work are recorded per agent version.
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

- [ ] Types and their semantics specified; the tool surface follows the T-64-008 decision.
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
