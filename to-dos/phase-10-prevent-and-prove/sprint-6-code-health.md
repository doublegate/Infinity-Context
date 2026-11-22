# Sprint 6 — Code Health

**Phase:** Phase 10 — Prevent and Prove  
**Release:** v1.8.0  
**Sprint Goal:** Pay down the debt found in the codebase-gap pass (2026-11-22) without changing
behaviour: split oversized modules, de-duplicate adapter code, type the CLI errors, add Cargo
features, and widen fuzzing, property tests and coverage. Each split lands in its own PR with tests
unchanged.  
**Estimated Duration:** 3 weeks

## Research gate

Re-check before any ticket starts:

- **External docs and versions:** current `cargo-fuzz`, `libfuzzer-sys`, `proptest` and
  `thiserror` versions and clippy lint names (`clippy::unwrap_used`) (Context7); GitHub Actions
  limits for a scheduled time-boxed fuzz job.
- **Installed agent behaviour:** the stdin payload shapes per agent that the shared parse block
  must accept (Phase 6 contract fixtures).
- **Project evidence:** fresh line counts and per-file coverage for `crates/icx-proxy/src/wire.rs`,
  `crates/icx-cli/src/main.rs`, `crates/icx-adapters/src/mmu.rs`, `crates/icx-codec/src/redact.rs`,
  `crates/icx-adapters/src/lib.rs` and `crates/icx-core/src/cas.rs`; the existing fuzz targets
  under `fuzz/fuzz_targets/` (`ahtl_stream`, `hdc_snapshot`, `mcp_jsonrpc`, `redact`); the state
  of the Phase 7 Sprint 4 structural redactor.

### Research notes

_None recorded yet._

## Tickets

### T-10-6-001 — Split `icx-proxy/src/wire.rs`

**Description:** Split `wire.rs` (3,998 lines) into `transform/{anthropic,openai,gemini}`, `relay`,
`http_server` and `upstream_curl` modules, and move its roughly 2,600 inline test lines to
`tests/`.

**Research:** Re-measure the file and its test lines; map which tests need crate-private access
and how they will reach it from `tests/` (a narrow test-support module).

**Acceptance Criteria:**

- [ ] Public API unchanged; the same tests pass with assertions unchanged.
- [ ] No module exceeds a size agreed in the PR description.
- [ ] Per-file coverage before and after is recorded in the PR.

**Dependencies:** None  
**Reference:** codebase-gap pass (2026-11-22); plan Phase 10 Sprint 6  
**Estimated Complexity:** L

---

### T-10-6-002 — Split `icx-cli/src/main.rs`

**Description:** Split `main.rs` (3,124 lines) into `src/cmd/<subcommand>.rs`, with `run_benchmark`
(455 lines) and `run_doctor` (217 lines) as their own modules.

**Research:** Re-measure; list shared helpers that need a common module.

**Acceptance Criteria:**

- [ ] CLI behaviour, flags and exit codes unchanged (existing CLI tests unchanged and passing).
- [ ] Each subcommand lives in its own module.

**Dependencies:** None  
**Reference:** codebase-gap pass (2026-11-22); plan Phase 10 Sprint 6  
**Estimated Complexity:** M

---

### T-10-6-003 — Split `mmu.rs`

**Description:** Split `icx-adapters/src/mmu.rs` into `ingest`, `persist` and `tail_slot` modules.

**Research:** Confirm module boundaries against the Phase 9 incremental-save changes so the split
does not fight them.

**Acceptance Criteria:**

- [ ] Behaviour unchanged; tests unchanged and passing.
- [ ] Persist code depends on ingest only through a narrow interface.

**Dependencies:** T-91-001  
**Reference:** codebase-gap pass (2026-11-22); plan Phase 10 Sprint 6  
**Estimated Complexity:** M

---

### T-10-6-004 — Split `redact.rs` behind a `Detector` trait

**Description:** Split `icx-codec/src/redact.rs` into `detectors/*` behind a `Detector` trait,
coordinated with the v1.5 structural redactor so the two changes do not conflict.

**Research:** Read the Phase 7 Sprint 4 ADR and implementation status; agree the trait shape with
it.

**Acceptance Criteria:**

- [ ] Redaction output is byte-identical on the redaction corpus and fuzz corpus.
- [ ] Each detector is a separate module with its own tests.
- [ ] Mutation testing on `redact.rs` shows no new survivors.

**Dependencies:** None (coordinates with T-74-003)  
**Reference:** codebase-gap pass (2026-11-22); plan Phase 7 Sprint 4; plan Phase 10 Sprint 6  
**Estimated Complexity:** L

---

### T-10-6-005 — Move conformance tests out of `icx-adapters/src/lib.rs`

**Description:** Move the 1,020-line and 756-line conformance test blocks in
`icx-adapters/src/lib.rs` into per-contract test files under `tests/`.

**Research:** Re-measure the blocks; confirm they use only public API or a test-support module.

**Acceptance Criteria:**

- [ ] Same tests, same assertions, now one file per contract under `tests/`.
- [ ] `lib.rs` contains no conformance tests.

**Dependencies:** None  
**Reference:** codebase-gap pass (2026-11-22); plan Phase 10 Sprint 6  
**Estimated Complexity:** M

---

### T-10-6-006 — Adapter de-duplication

**Description:** Add `adapters/common.rs` holding the identical `non_empty_body_value` and
`non_empty_json_value` helpers and one copy of the stdin-parse block that is duplicated five
times, and give `AgentHookAdapter` a default `parse_payload`.

**Research:** Diff the five stdin-parse copies to confirm they are identical or record the
differences that must be parameterised.

**Acceptance Criteria:**

- [ ] One implementation of each helper and of the stdin-parse block.
- [ ] All adapter contract tests pass unchanged.

**Dependencies:** T-10-6-005  
**Reference:** codebase-gap pass (2026-11-22); plan Phase 10 Sprint 6  
**Estimated Complexity:** M

---

### T-10-6-007 — Typed CLI errors and `clippy::unwrap_used`

**Description:** Introduce a typed CLI error enum with documented exit codes, and enable
`clippy::unwrap_used` on non-test code, fixing the three sites in `icx-hdc`.

**Research:** List current exit codes and their callers (agents treat some as hook results) so no
code changes meaning.

**Acceptance Criteria:**

- [ ] Each error variant maps to a documented exit code; hook-facing exit codes are unchanged.
- [ ] `clippy::unwrap_used` is denied outside tests and passes.

**Dependencies:** T-10-6-002  
**Reference:** codebase-gap pass (2026-11-22); plan Phase 10 Sprint 6  
**Estimated Complexity:** M

---

### T-10-6-008 — Cargo features `proxy`, `hermes` and `bus`

**Description:** Add `proxy`, `hermes` and `bus` features so library users can build a slimmer set;
none exist today. The default build stays as it is.

**Research:** Map which modules and dependencies each feature gates, and check intra-doc links to
feature-gated items.

**Acceptance Criteria:**

- [ ] The default build is unchanged.
- [ ] CI builds and tests each feature on its own and with all features.
- [ ] Docs build with warnings as errors under each feature set.

**Dependencies:** T-10-6-001  
**Reference:** codebase-gap pass (2026-11-22); plan Phase 10 Sprint 6  
**Estimated Complexity:** M

---

### T-10-6-009 — New fuzz targets and a nightly fuzz job

**Description:** Add fuzz targets for the proxy transforms, HTTP/SSE parsing, Codex rollout JSONL,
Hermes messages, the daemon request line, and graph/DAG/CAS loading (four targets exist today),
and add a nightly, time-boxed fuzz job in CI, which has none now.

**Research:** Confirm seed corpora per target from fixtures; pick a time box that fits the runner
limits.

**Acceptance Criteria:**

- [ ] Each new target builds and runs with a seed corpus.
- [ ] A scheduled CI job runs all targets within its time box and uploads crashes as artifacts.
- [ ] Any crash found is fixed with a regression test.

**Dependencies:** T-82-002  
**Reference:** codebase-gap pass (2026-11-22); `fuzz/fuzz_targets/`; plan Phase 10 Sprint 6  
**Estimated Complexity:** L

---

### T-10-6-010 — Property tests, `icx-store/tests/` and per-file coverage

**Description:** Add property tests for the chunker/CAS, bus ring, epoch controller and store
graph; add an `icx-store/tests/` suite; and raise per-file coverage where it is under 95%
(`wire.rs`, `cas.rs`, `main.rs`, `mmu.rs`, `redact.rs`), measured fresh first.

**Research:** Measure per-file coverage before writing tests and record the starting values.

**Acceptance Criteria:**

- [ ] Property tests exist for each listed component.
- [ ] `crates/icx-store/tests/` exists and runs in CI.
- [ ] Each listed file is at or above 95% line coverage, before and after values recorded in the
  PR.

**Dependencies:** T-10-6-001, T-10-6-002, T-10-6-003, T-10-6-004  
**Reference:** codebase-gap pass (2026-11-22); plan Phase 10 Sprint 6  
**Estimated Complexity:** L

---

### T-10-6-011 — AHTL parser tests generated from the EBNF grammar

**Description:** Generate parser test cases from the AHTL EBNF grammar (valid and near-miss
inputs), so the parser is checked against the specification rather than hand-picked cases.

**Research:** Confirm the grammar file in `docs/alien-holographic-codec.md` is complete enough to
generate from; record gaps as spec issues.

**Acceptance Criteria:**

- [ ] Generated valid inputs parse and round-trip; generated near-misses are rejected.
- [ ] Any grammar or parser disagreement is fixed in the prose or the code, per the docs-as-spec
  rule.

**Dependencies:** None  
**Reference:** `docs/claude/11-enhancements-and-optimizations.md` §D  
**Estimated Complexity:** M

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Line coverage is `>= 95%` (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Affected docs and `CHANGELOG.md` `[Unreleased]` are updated.
- [ ] Upstream re-scan for Sprint 7 is done and its tickets are adjusted.
