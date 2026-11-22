# Sprint 2 — Compatibility

**Phase:** Phase 9 — Storage, Scale and Platforms  
**Release:** v1.7.0  
**Sprint Goal:** Make downgrades possible and documented, and make a running MCP server notice a
newer snapshot format after an upgrade and tell the user to reconnect.  
**Estimated Duration:** 1 week

## Research gate

Re-check before any ticket starts:

- **External docs and versions:** the MCP specification's guidance on server errors and
  notifications that clients surface to the user.
- **Installed agent behaviour:** how each installed client (Claude Code, Codex, agy, Hermes, Muse)
  shows an MCP error or notification, and how the user reconnects or restarts an MCP server in
  each.
- **Project evidence:** every snapshot magic and format version in use (ICXHDC04, ICXHDC05 and
  later); the known downgrade limits in `PROJECT-STATUS.md:76-78`; the field finding of a snapshot
  magic mismatch after an upgrade while an MCP server from the older binary was still running.

### Research notes

_None recorded yet._

## Tickets

### T-92-001 — `icx export --format <v>`

**Description:** Add `icx export --format <v>` to write a workspace in an older supported snapshot
format, dropping or folding data that format cannot hold and reporting what was dropped.

**Research:** List which formats are still supported for export and what each cannot represent
(for example Muse events before their format version).

**Acceptance Criteria:**

- [ ] Export to each supported format is read correctly by that release's reader (round-trip test
  with recorded reader fixtures).
- [ ] Dropped data is listed in the command output and in a JSON report.
- [ ] Export never modifies the source workspace.

**Dependencies:** None  
**Reference:** `PROJECT-STATUS.md:76-78`; plan Phase 9 Sprint 2  
**Estimated Complexity:** M

---

### T-92-002 — Documented downgrade paths

**Description:** Document, per release, which older releases can read its workspace, and the
downgrade procedure (backup, export, verify) in the deployment guide.

**Research:** Verify each documented path by running it against the release binaries.

**Acceptance Criteria:**

- [ ] A compatibility table lists reader and writer versions per format.
- [ ] Each documented procedure has been run end to end and the result recorded.
- [ ] `icx doctor` reports the workspace format and the oldest release that can read it.

**Dependencies:** T-92-001  
**Reference:** `PROJECT-STATUS.md:76-78`; `docs/deployment-and-agent-integration-guide.md`  
**Estimated Complexity:** S

---

### T-92-003 — Detect a newer snapshot format and tell the user to reconnect

**Description:** When an MCP server started from an older binary finds that the workspace snapshot
has been upgraded to a newer format (snapshot magic mismatch), it must fail closed with a clear
message telling the user to reconnect or restart the MCP server, instead of returning opaque
errors on every call.

**Research:** Reproduce the field finding: upgrade `icx` while a client holds a running server,
then call the tool. Confirm on each installed client how the message is shown and the exact
reconnect action to name.

**Acceptance Criteria:**

- [ ] A test with a newer snapshot magic produces one actionable error naming the reconnect step.
- [ ] The server does not write to or modify the newer snapshot.
- [ ] The message is verified on each installed client and the reconnect step per client is
  documented.

**Dependencies:** None  
**Reference:** field finding (snapshot magic mismatch after upgrade); plan Phase 9 Sprint 2  
**Estimated Complexity:** S

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Line coverage is `>= 95%` (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Affected docs and `CHANGELOG.md` `[Unreleased]` are updated.
- [ ] Upstream re-scan for Sprint 3 is done and its tickets are adjusted.
