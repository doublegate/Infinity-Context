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

**2026-11-22, from Sprint 1** (`ref-docs/agent-internals/below-hook-findings-2026-11-22.md`, Codex
section and decision table). Source at `rust-v0.171.0`; the installed binary matches the tag.

- **`decision:"block"`:** the model receives the reason as a *failed* tool call
  (`registry.rs:1080-1088`). It is a no-go as a substitution.
- **`continue:false` with `reason`/`stopReason` on PostToolUse:** the model sees only that text in
  place of the result. The call is not marked failed, and the turn continues
  (`registry.rs:371-393`, `1089-1096`; `post_tool_use.rs:44-49`). T-62-001 and T-62-002 now target
  this route.
- **Field rejections:**
  - `updatedToolOutput` makes the whole reply fail to parse;
  - `updatedMCPToolOutput` marks the hook Failed and drops its `additionalContext`;
  - `suppressOutput:true` is rejected.
- **Which calls are covered:** shell commands arrive as tool `Bash` and always count as success.
  PostToolUse runs only for successful calls, so a failed MCP call is never seen.
- **Rollouts:**
  - path `~/.codex/sessions/YYYY/MM/DD/rollout-<ts>-<thread>[_<id>].jsonl`;
  - zstd level 3 for files older than 7 days, behind `local_thread_store_compression`, and
    `.zst` files already exist on this machine;
  - each stored output is capped at 64 KiB.
- **`additionalContextLimit`:** defaults to about 2,500 tokens. Text over the limit is spilled to
  a file and only a preview reaches the model (new ticket T-62-006).

**2026-11-22, upstream re-scan at Sprint 1 close:**

- **Releases:** 0.171.0 is still the latest stable release; 0.173.0 is in alpha (alpha.4,
  2026-11-21).
- **Hook change in the alpha:** a per-handler `onFailure` setting (openai/codex#62831). Its
  default, `continue`, keeps today's behaviour; `block` withholds a tool's result when its
  PostToolUse hook fails.
- **What icx must do:** keep `onFailure` unset (fail-open) in the hook config it writes. Check this
  in T-62-001 and in `icx install` (T-65-004) once the setting reaches a stable release.
- **No other changes:** nothing else in the hooks or rollout code since the tag changes the
  Sprint 1 findings.

**2026-11-22, research gate for T-62-003, T-62-004 and T-62-006:**

- **zstd decoder:** `ruzstd` 0.9.1 (MIT, pure Rust, one dependency `twox-hash`, MIT). Its default
  `hash` feature verifies frame checksums. It needs no C toolchain, so static musl release builds
  are unchanged. `cargo deny check`: advisories, bans, licences and sources all pass.
  - The C-binding `zstd` crate was rejected: it needs a C build.
  - `structured-zstd` was rejected: it adds a compressor icx does not need.
- **Codex rollout form on this machine:**
  - 56 plain and 518 compressed rollouts, 216 MB;
  - each compressed file is a single frame;
  - the first record is `session_meta`, whose `payload.cwd` identifies the project.
- **Codex token estimate:** `approx_token_count` is UTF-8 bytes / 4, rounded up
  (`codex-rs/utils/string/src/truncate.rs:94-97`). The default limit is
  `DEFAULT_HOOK_OUTPUT_TOKEN_LIMIT = 2_500` (`hooks/src/output_spill.rs`), so the inline bound is
  10,000 bytes.
- **icx's own tail slot:** capped at 2,048 bytes at most by the epoch presets, plus the legend, so
  every Codex reply was already within the bound. T-62-006 therefore pins the bound at the reply,
  rather than adding a truncation algorithm.

**2026-11-22, results on the installed data** (release build, throwaway workspace, read-only on
`~/.codex/sessions`):

- **This project:** 33 rollouts, 2,906 records in 24.7 s. The second run took 1.5 s and ingested
  nothing.
- **`--all-projects`:** 574 rollouts (518 compressed) and 99,283 records in 426 s, with a peak of
  about 2 GB of memory. Nothing was skipped, so every real compressed rollout decoded.

## Tickets

### T-62-001 — Verify `continue:false` substitution on the installed Codex

**Description:** Run a controlled Codex session with a PostToolUse hook that returns
`continue:false` and the icx stub as `reason`, and record exactly what the model sees for shell
and MCP tool calls. Run `decision:"block"` alongside it as the control.

**Research:** Hypothesis, from the tagged source (Sprint 1): on the installed Codex,
`continue:false` plus `reason` replaces the whole tool result with the hook's text, the call is
not shown as failed, and the turn continues, while `decision:"block"` shows as a failed call.
Confirmed if the rollout and the model's next turn show the stub in place of the raw output for
both tool kinds; rejected otherwise, and the ticket closes with the finding.

**Acceptance Criteria:**

- [ ] Codex version, hook config and raw rollout excerpts recorded in the research notes.
- [ ] Result per tool kind (shell, MCP) and per reply (`continue:false`, `decision:"block"`):
  substituted, shown as error, or ignored.
- [ ] Whether the rollout stores the substitute or the original recorded.
- [ ] Decision recorded: proceed to T-62-002, or close with the finding.

**Dependencies:** T-61-003

**Reference:** `docs/agent-adapters-and-interception.md` Codex section (lines 110-117); ICX-AUD-005;
Codex hooks documentation

**Estimated Complexity:** S

---

### T-62-002 — Implement `continue:false` substitution behind a flag

**Description:** If T-62-001 confirms substitution, add an opt-in mode in `CodexAdapter` that
returns `continue:false` with the icx stub as `reason`, for outputs over the stub threshold. The
default stays non-replacing.

**Research:** Re-run T-62-001 on the Codex version current at coding time. The behaviour relies
on a side effect of `continue:false`, so the mode is version-gated to versions where T-62-001
passed.

**Acceptance Criteria:**

- [ ] Opt-in flag (env and config), off by default; default output byte-identical to v1.3.2.
- [ ] Version-gated: on an unverified Codex version the mode stays off and logs why.
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

- [x] A CLI entry point (subcommand or daemon option) that tails rollouts with bounded reads.
  `icx codex-ingest`, built on a new `codex_ingest` module that shares the tailer's bounded
  record reader. It filters by session `cwd` and saves bookmarks in the workspace.
- [x] Ingested items are redacted and stored like hook output (`ingest_payload_with_exit_code`
  runs `SecretRedactor` before storage).
- [x] Integration test over a recorded rollout fixture (`crates/icx-cli/tests/codex_ingest_cli.rs`,
  with a synthetic `zstd -3` rollout).
- [x] The "not wired into the CLI" limitation removed from the adapters doc.

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

- [x] `.jsonl.zst` rollouts are read; a decompression bomb is refused with a typed error
  (`RolloutIngestError::DecompressedTooLarge`, 1 GiB, checked before anything is ingested).
- [x] Fuzz or property test over truncated and corrupt frames
  (`crates/icx-adapters/tests/codex_rollout_zstd_props.rs`, plus deterministic truncations in the
  module tests).
- [x] `deny.toml` and licence checks pass for the new dependency.

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

- [ ] A dated watch entry in the research notes at each sprint close of this phase. The first is
  recorded (2026-11-22, Sprint 1 close: not supported); more follow at each sprint close.
- [x] Until support lands, the existing tests that keep `updatedToolOutput` and
  `updatedMCPToolOutput` out of every Codex reply stay in place. On 0.171.0 either field breaks
  the reply (Sprint 1).
- [ ] If support lands: version-gated path, contract test, and capability matrix update.
- [ ] If not by phase close: the ticket carries over to the next phase with its status.

**Dependencies:** None

**Reference:** ICX-AUD-005; `docs/agent-adapters-and-interception.md` lines 110-114;
`ref-docs/agent-internals/below-hook-findings-2026-11-22.md`

**Estimated Complexity:** S

---

### T-62-006 — Keep Codex `additionalContext` within `additionalContextLimit`

**Description:** Codex spills `additionalContext` over the per-handler `additionalContextLimit`
(default about 2,500 tokens) to a file, and the model sees only a preview and the path. Budget
every Codex `additionalContext` reply so the recall and tail-slot content stays under the limit,
and document the handler-side setting for users who raise it.

**Research:** Confirm the default and the token estimate Codex uses on the installed version
(`output_spill.rs:13`, `discovery.rs:695-712` at `rust-v0.171.0`). Measure the current reply
sizes on recorded sessions.

**Acceptance Criteria:**

- [x] Codex replies are budgeted below the default limit, using Codex's own token estimate or a
  documented conservative bound (`CODEX_INLINE_CONTEXT_MAX_BYTES` = 10,000 bytes = 2,500 × 4).
- [x] Truncation keeps every `#icx:` handle intact and is deterministic: an oversized value is
  replaced whole by a diagnostic, never cut.
- [x] Property test: no reply exceeds the budget for any input (proptest on the guard; an
  integration test over every preset and the three context-carrying events).
- [x] Deployment guide documents `additionalContextLimit` (0 disables the limit).

**Dependencies:** None

**Reference:** `ref-docs/agent-internals/below-hook-findings-2026-11-22.md` (Codex
`additionalContext`); Codex `codex-rs/hooks/src/output_spill.rs`

**Estimated Complexity:** S

## Sprint exit

- [ ] Gates green: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and
  coverage `>= 95%`.
- [ ] Docs and CHANGELOG updated for any user-visible change.
- [ ] Upstream re-scan done for Sprint 3 (Gemini CLI, Cursor, OpenCode), and its tickets
  adjusted.
