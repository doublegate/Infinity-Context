# Sprint 2 — Codex Output Shrinking

**Phase:** Phase 6 — Agent Reach and Output Shrinking

**Release:** v1.4.0

**Sprint Goal:** Make icx shrink tool output in Codex where the installed version allows it,
and ingest Codex rollouts from the CLI, including zstd-compressed ones.

**Estimated Duration:** 1.5 weeks

## Research gate

Re-check before the sprint starts:

- **External docs and versions:** the Codex hooks documentation for `decision:"block"` and
  `updatedMCPToolOutput`; Codex release notes since codex-cli 0.170.0; current `zstd` crate
  version via Context7.
- **Installed agent behaviour:** the installed Codex version, and the T-61-003 findings on hook
  response handling and rollout format.
- **Project evidence:** `CodexAdapter::SUPPORTS_TOOL_OUTPUT_REPLACEMENT` and its tests;
  `crates/icx-adapters/tests/protocol_validation.rs`; ICX-AUD-005 status in
  `docs/claude/findings.csv`.

### Research notes

_None recorded yet._

## Tickets

### T-62-001 — Verify `decision:"block"` substitution on the installed Codex

**Description:** Run a controlled Codex session with a hook that returns `decision:"block"` and
a replacement reason, and record exactly what the model sees for shell and MCP tool calls.

**Research:** Hypothesis: on the installed Codex, `decision:"block"` replaces the whole tool
result with the hook's text, and the model treats it as the result rather than as an error.
Confirmed if the rollout and the model's next turn show the stub in place of the raw output for
both tool kinds; rejected otherwise, and the ticket closes with the finding.

**Acceptance Criteria:**

- [ ] Codex version, hook config and raw rollout excerpts recorded in the research notes.
- [ ] Result per tool kind (shell, MCP): substituted, shown as error, or ignored.
- [ ] Decision recorded: proceed to T-62-002, or close with the finding.

**Dependencies:** T-61-003

**Reference:** `docs/agent-adapters-and-interception.md` Codex section (lines 110-117); ICX-AUD-005;
Codex hooks documentation

**Estimated Complexity:** S

---

### T-62-002 — Implement block-based substitution behind a flag

**Description:** If T-62-001 confirms substitution, add an opt-in mode in `CodexAdapter` that
returns `decision:"block"` with the icx stub for outputs over the stub threshold. Default stays
non-replacing.

**Research:** Re-run T-62-001 on the Codex version current at coding time.

**Acceptance Criteria:**

- [ ] Opt-in flag (env and config), off by default; default output byte-identical to v1.3.2.
- [ ] The stub carries the `#icx:<hash>` handle and dereferences losslessly.
- [ ] Contract test pins the reply shape; a live-run test detects a silent ignore.
- [ ] Saving on a recorded Codex session measured and published in `docs/numbers.md`.
- [ ] Adapters doc and deployment guide updated; CHANGELOG entry.

**Dependencies:** T-62-001

**Reference:** ICX-AUD-005; `docs/agent-adapters-and-interception.md` lines 110-117

**Estimated Complexity:** M

---

### T-62-003 — Wire `CodexRolloutTailer` into the CLI

**Description:** Expose the rollout tailer through the `icx` CLI so Codex sessions are ingested
from `~/.codex/sessions/**/rollout-*.jsonl` without library-only use.

**Research:** Confirm the current rollout path and naming on the installed Codex (T-61-003).

**Acceptance Criteria:**

- [ ] A CLI entry point (subcommand or daemon option) that tails rollouts with bounded reads.
- [ ] Ingested items are redacted and stored like hook output.
- [ ] Integration test over a recorded rollout fixture.
- [ ] The "not wired into the CLI" limitation removed from the adapters doc.

**Dependencies:** T-61-003

**Reference:** `docs/agent-adapters-and-interception.md` line 117 (rollout tailer known
limitations); ICX-AUD-005

**Estimated Complexity:** M

---

### T-62-004 — Read `.jsonl.zst` rollouts

**Description:** Add zstd decoding to the rollout tailer, with the same bounds on decompressed
size as on plain JSONL.

**Research:** Check the current `zstd` crate (or a pure-Rust decoder, given
`unsafe_code = "forbid"` applies only to this workspace) via Context7 and `cargo deny`.

**Acceptance Criteria:**

- [ ] `.jsonl.zst` rollouts are read; a decompression bomb is refused with a typed error.
- [ ] Fuzz or property test over truncated and corrupt frames.
- [ ] `deny.toml` and licence checks pass for the new dependency.

**Dependencies:** T-62-003

**Reference:** `docs/agent-adapters-and-interception.md` line 117; ICX-AUD-005 ("zstd rollouts
unsupported")

**Estimated Complexity:** S

---

### T-62-005 — Watch upstream `updatedMCPToolOutput` support

**Description:** Track Codex upstream for support of `updatedMCPToolOutput` (or an equivalent).
When it lands, add a version-gated replacement path and flip the capability for that version.

**Research:** At each sprint close, check Codex releases and issues for the field.

**Acceptance Criteria:**

- [ ] A dated watch entry in the research notes at each sprint close of this phase.
- [ ] If support lands: version-gated path, contract test, and capability matrix update.
- [ ] If not by phase close: the ticket carries over to the next phase with its status.

**Dependencies:** None

**Reference:** ICX-AUD-005; `docs/agent-adapters-and-interception.md` lines 110-114

**Estimated Complexity:** S

## Sprint exit

- [ ] Gates green: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and
  coverage `>= 95%`.
- [ ] Docs and CHANGELOG updated for any user-visible change.
- [ ] Upstream re-scan done for Sprint 3 (Gemini CLI, Cursor, OpenCode), and its tickets
  adjusted.
