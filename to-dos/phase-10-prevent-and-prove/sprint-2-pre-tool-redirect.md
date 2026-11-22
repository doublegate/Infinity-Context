# Sprint 2 — Pre-Tool Redirect

**Phase:** Phase 10 — Prevent and Prove  
**Release:** v1.8.0  
**Sprint Goal:** Stop oversized Read and WebFetch calls before they run and point the agent at
`slice` / `grep`, and stub results only when the saving is worth it.  
**Estimated Duration:** 1 week

## Research gate

Re-check before any ticket starts:

- **External docs and versions:** context-mode's redirect pattern and its current behaviour; each
  agent's PreToolUse deny/block semantics and how the denial reason reaches the model.
- **Installed agent behaviour:** on each installed version, whether blocking a Read or WebFetch
  with a reason leads the model to call `slice` / `grep` instead, or to retry the same call.
- **Project evidence:** the field finding that WebFetch results of 583 to 1,300 B were stubbed; the
  2 KiB floor and where it is measured (text or JSON envelope).

### Research notes

_None recorded yet._

## Tickets

### T-10-2-001 — Block oversized Read and WebFetch with a `slice` / `grep` suggestion

**Description:** In PreToolUse, block Read and WebFetch calls whose expected output exceeds a
configured size and return a reason suggesting `slice` or `grep` with the handle (the context-mode
pattern). The redirect is opt-in per agent.

**Research:** Hypothesis: the redirect lowers tokens per session without lowering task success.
Measurement: replayed sessions for tokens, and the Sprint 5 smoke subset for success. If models
retry the blocked call instead, close the ticket for that agent with the finding.

**Acceptance Criteria:**

- [ ] The redirect is off by default and enabled per agent.
- [ ] Reads under the threshold and reads with explicit ranges are never blocked.
- [ ] A contract test per supported agent shows the reason reaching the model.
- [ ] Token change is measured and published in `docs/numbers.md`.

**Dependencies:** None  
**Reference:** context-mode repository (pinned URL and commit recorded in the research notes);
plan Phase 10 Sprint 2  
**Estimated Complexity:** M

---

### T-10-2-002 — Minimum-saving ratio measured on the text

**Description:** Stub a result only when it clears both the 2 KiB floor and a minimum-saving ratio,
with sizes measured on the result text and not on the JSON envelope, so small results such as the
583 to 1,300 B WebFetch outputs are no longer stubbed.

**Research:** Reproduce the field finding and confirm the size was measured on the envelope.
Choose the ratio from the field corpus.

**Acceptance Criteria:**

- [ ] Sizes are measured on the extracted text in all adapters (test per adapter).
- [ ] The field cases of 583 to 1,300 B are no longer stubbed (regression test).
- [ ] The chosen ratio and its effect on the corpus are measured and published in
  `docs/numbers.md`.

**Dependencies:** None  
**Reference:** field finding (583 to 1,300 B WebFetch results stubbed); plan Phase 10 Sprint 2  
**Estimated Complexity:** S

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Line coverage is `>= 95%` (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Affected docs and `CHANGELOG.md` `[Unreleased]` are updated.
- [ ] Upstream re-scan for Sprint 3 is done and its tickets are adjusted.
