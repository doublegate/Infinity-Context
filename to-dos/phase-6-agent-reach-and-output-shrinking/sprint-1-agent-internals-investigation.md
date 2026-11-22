# Sprint 1 — Agent Internals Investigation (REA/Ghidra)

**Phase:** Phase 6 — Agent Reach and Output Shrinking

**Release:** v1.4.0

**Sprint Goal:** Find integration points below the documented hooks of the installed agents,
record them in a dated dossier, and decide go or no-go for each one.

**Estimated Duration:** 2 weeks

## Research gate

Re-check before the sprint starts:

- **External docs and versions:** each vendor's licence and terms of use for the installed
  build; current hook documentation for Claude Code, Codex, agy, Muse and Hermes, so that
  anything the docs now cover is not treated as undocumented.
- **Installed agent behaviour:** the exact installed version and install path of each agent
  (`claude --version`, `codex --version`, `agy --version`, Muse, Hermes source commit). The
  investigation covers the user's own installs only.
- **Project evidence:** `ref-docs/agent-internals/rea-binary-and-sqlite-forensics.md` and
  `ref-docs/agent-internals/cross-agent-hook-and-wire-matrix.md` as the baseline; the
  field scan of agent sessions for the places users lose context.

### Research notes

**2026-11-22, research gate:**

- **Installed versions:**
  - Claude Code 2.1.330, native ELF at `~/.local/share/claude/versions/2.1.330`;
  - Codex CLI 0.171.0, static musl;
  - agy 1.5.14, ELF;
  - Muse Code 1.4.25 (`1.4.25-R9361.1`), static ELF;
  - Hermes v0.23.3, source checkout at `caafe5389ae` (2026-11-19). That commit differs from the
    `dc394c6` that the adapters doc records.
- **Terms review and scope (T-61-001):**
  - Codex is Apache-2.0 and Hermes is MIT.
  - The per-agent terms review and the owner's scope decisions are recorded in the private notes
    (ADR-0008), not in this file.
- **Public sources used to corroborate the findings:**
  - Codex (`openai/codex` at the installed tag);
  - Hermes (`NousResearch/hermes-agent` at `caafe5389ae`);
  - Claude Code (`anthropics/claude-code`: CHANGELOG, documentation and issues);
  - agy and Muse (public documentation and changelog).

**2026-11-22, investigation complete:**

- **Dossier:** `ref-docs/agent-internals/below-hook-findings-2026-11-22.md`, which holds the scope,
  the per-agent findings, the go/no-go table and the versions.
- **Private evidence:** the evidence is kept in the gitignored
  `ref-docs/agent-internals/private/`, per ADR-0008. That is where the "evidence bundle recorded"
  criteria below are met.
- **Follow-up planning:** Sprints 2, 4 and 5 and Phase 7 Sprint 3 were adjusted. Sprint 3 needed
  no change. New tickets: T-62-006, T-65-007, T-65-008, T-65-009.

## Tickets

### T-61-001 — Scope and terms review per vendor

**Description:** For each agent, record the licence and terms that apply to inspecting the
installed build, and the scope the investigation stays within: interoperability with the user's
own installs, no redistribution of vendor code, no circumvention of access controls, and no
network interaction with vendor services beyond normal use.

**Research:** Read the current terms for Claude Code, Codex CLI, agy, Muse and Hermes. Note
which are open source (Codex, Hermes) so that source reading replaces binary inspection there.

**Acceptance Criteria:**

- [x] A scope table per agent: licence, terms clause relied on, allowed methods (source read,
  REA, Ghidra), and excluded methods.
- [x] Any agent whose terms forbid inspection is marked out of scope, with the reason.
- [x] The scope table is the first section of the T-61-006 dossier. Amended by owner decision
  (2026-11-22): the table is kept in the private notes, and the public dossier's first section
  states the scope without it.

**Dependencies:** None

**Reference:** Plan Phase 6 Sprint 1 ("interoperability with the user's own installs only");
`ref-docs/agent-internals/rea-binary-and-sqlite-forensics.md`

**Estimated Complexity:** S

---

### T-61-002 — Inspect Claude Code below the hooks

**Description:** Use REA on the installed Claude Code to locate internal event emitters, the
shape of tool results before display, compaction internals, local IPC and session storage.

**Research:** Hypothesis: Claude Code exposes no supported surface beyond hooks, MCP and the
session JSONL, but its compaction path and tool-result rendering show which hook fields are
honoured per tool. Confirmed or rejected by tracing the `updatedToolOutput` handling per
built-in tool in the installed bundle.

**Acceptance Criteria:**

- [x] Installed version, bundle path and REA evidence bundle recorded.
- [x] Per built-in tool: where the result is shaped, and whether a hook replacement is applied.
- [x] Compaction trigger and summary path described, with the inputs it reads.
- [x] Findings feed T-64-001 and T-64-007.

**Dependencies:** T-61-001

**Reference:** Claude Code issues #86753 and #67442;
`ref-docs/agent-internals/cross-agent-hook-and-wire-matrix.md`

**Estimated Complexity:** L

---

### T-61-003 — Inspect Codex CLI below the hooks

**Description:** Read the Codex source at the installed release tag and confirm against the
installed binary: hook dispatch, whether any path can substitute an MCP or shell tool result,
the rollout writer (including zstd), and local IPC.

**Research:** Hypothesis: Codex applies `decision:"block"` as a whole-result substitution but
ignores `updatedMCPToolOutput`. Confirmed or rejected by locating the hook-response handling in
the source for the installed tag.

**Acceptance Criteria:**

- [x] Installed version and source tag recorded.
- [x] Hook-response handling documented per field, with file and line in the tagged source.
- [x] Rollout file naming and compression documented for T-62-003 and T-62-004.

**Dependencies:** T-61-001

**Reference:** ICX-AUD-005 (`docs/claude/06-agent-integration-verification.md`);
`docs/agent-adapters-and-interception.md` (Codex section, rollout tailer limitations)

**Estimated Complexity:** M

---

### T-61-004 — Inspect agy and Muse below the hooks

**Description:** Use REA on the installed agy and Muse to find internal events, pre-display
tool-result shapes, compaction internals, session databases, and the permission or policy plane
that controls MCP tool approval.

**Research:** Confirm which settings file and key each agent reads for MCP tool approval
(agy: `permissions.allow` in `~/.gemini/antigravity-cli/settings.json`; Muse: key still to be
determined on the installed version), and how agy handles `user_markdown`.

**Acceptance Criteria:**

- [x] Installed versions and REA evidence bundles recorded.
- [x] Muse's MCP tool-approval key identified on the installed version, or recorded as not
  found with the search method.
- [x] agy `user_markdown` handling described for T-65-003.

**Dependencies:** T-61-001

**Reference:** `ref-docs/agent-internals/rea-binary-and-sqlite-forensics.md`; plan "Immediate
fixes" (agy, Muse and Hermes approval settings)

**Estimated Complexity:** L

---

### T-61-005 — Read Hermes context-engine internals

**Description:** Read the Hermes source at the installed commit for the `ContextEngine`
interface, `select_context`, compaction, and how a plugin is loaded, so the Sprint 5 shim and the
Phase 7 resume packet target real interfaces.

**Research:** Confirm that the interface names and load path are unchanged since the commit
recorded in the adapters doc (`dc394c6`).

**Acceptance Criteria:**

- [x] Commit recorded; interface signatures quoted with file and line.
- [x] Plugin load path and failure behaviour described.
- [x] Findings feed T-65-002 and T-73-003.

**Dependencies:** T-61-001

**Reference:** ICX-AUD-006 (`docs/claude/findings.csv`); `docs/agent-adapters-and-interception.md`
(Hermes section)

**Estimated Complexity:** M

---

### T-61-006 — Findings dossier

**Description:** Write the findings as a new dated file under `ref-docs/agent-internals/`. The
corpus is immutable, so existing files are not edited; the new file cross-links them.

**Research:** None beyond T-61-002 to T-61-005.

**Acceptance Criteria:**

- [x] New file `ref-docs/agent-internals/below-hook-findings-<date>.md` with the scope table,
  per-agent findings, evidence references and agent versions.
- [x] No vendor source, binary excerpt beyond what interoperability needs, or secret in the
  file; gitleaks clean.
- [x] markdownlint clean and REUSE annotated.

**Dependencies:** T-61-002, T-61-003, T-61-004, T-61-005

**Reference:** Plan Phase 6 Sprint 1; `ref-docs/agent-internals/` (immutable corpus rule)

**Estimated Complexity:** M

---

### T-61-007 — Go/no-go decision per below-hook integration

**Description:** For each candidate integration found, decide go or no-go, weighing benefit,
stability across versions, terms, and maintenance cost. Every go item becomes a ticket in a
later sprint; every no-go item records its reason.

**Research:** Re-check each candidate against the latest agent release before deciding.

**Acceptance Criteria:**

- [x] A decision table in the dossier: candidate, agent, decision, reason, follow-up ticket.
- [x] Any go item is off by default and version-gated in its follow-up ticket.
- [x] Sprints 2 to 5 adjusted to the decisions.

**Dependencies:** T-61-006

**Reference:** Plan Phase 6 Sprint 1 ("a go/no-go decision per below-hook integration")

**Estimated Complexity:** S

## Sprint exit

- [x] Gates green: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and
  coverage `>= 95%`. On 2026-11-22: 358 tests pass, line coverage 95.43%. No Rust changed in
  this sprint.
- [x] Docs and CHANGELOG updated for any user-visible change.
- [x] Upstream re-scan done for Sprint 2 (Codex releases and hooks docs), and its tickets
  adjusted.
