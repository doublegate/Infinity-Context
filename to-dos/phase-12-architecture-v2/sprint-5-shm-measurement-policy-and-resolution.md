# Sprint 5 — `icx-shm` Measurement, Documentation, Policy and Resolution

**Phase:** Phase 12 — Architecture Release  
**Release:** v2.0.0  
**Sprint Goal:** Publish the `icx-shm` measurements, make the unsafe-code policy wording precise
everywhere it appears, rewrite the README bus note beside the patent summary, write the release
and upgrade notes, mark ADR-0007 implemented, and close issue #32 with links to the PRs and the
release.  
**Estimated Duration:** 1.5 weeks

## Research gate

Before any ticket starts:

- Grep the repository for every statement of the unsafe-code policy ("no `unsafe`",
  `unsafe_code = "forbid"`, "zero `unsafe`") in README, `docs/`, `AGENTS.md`, `CLAUDE.md`, CI
  workflows and the docs site.
- Re-read the README patent summary and the bus note beside it.
- Re-run the Phase 8 per-hook traces to get the current share of hook time spent in the bus.
- Re-read issue #32's comments for any open question the closing comment must answer.

### Research notes

_None recorded yet._

## Tickets

### T-12-5-001 — Measurements in `docs/numbers.md`

**Description:** Measure cross-process publish-to-visible latency for the shm backend against the
file bus (the v1.3 baseline was a 38 µs worst case over 200 events, to be re-measured on the same
harness), and the share of hook time spent in the bus, on x86_64 and arm64. Record methodology,
hardware and run count. No figure is claimed before it is measured.

**Research:** Hypothesis: shm reduces publish-to-visible latency, and the bus share of hook time
falls. Confirming measurement: the distributions from the shared harness. A rejected hypothesis is
published as measured.

**Acceptance Criteria:**

- [ ] Latency distributions for both backends measured and published in `docs/numbers.md`, with
  methodology and hardware.
- [ ] Bus share of hook time measured and published in `docs/numbers.md`.
- [ ] No sub-microsecond or other figure appears in docs unless it comes from this measurement.

**Dependencies:** T-12-4-004  
**Reference:** Issue #32; ADR-0007 §Decision 6; `docs/numbers.md`  
**Estimated Complexity:** M

---

### T-12-5-002 — Policy wording: README Security section and the CI unsafe gate

**Description:** Change "no `unsafe` anywhere" to "`unsafe` confined to the opt-in `icx-shm`
crate" in the README Security section, `docs/security-and-threat-model.md`, the docs site, and the
CI unsafe gate's messages.

**Research:** Use the research-gate grep as the checklist of locations.

**Acceptance Criteria:**

- [ ] Every location in the checklist updated; a final grep finds no stale wording.
- [ ] The CI gate's failure message states the precise policy.

**Dependencies:** T-12-1-009  
**Reference:** ADR-0007 §Consequences (policy wording); issue #32  
**Estimated Complexity:** S

---

### T-12-5-003 — `AGENTS.md` change proposed as a diff only

**Description:** Prepare the `AGENTS.md` policy-wording change as a proposed diff for the owner
to apply, since that file holds the owner's uncommitted edits. Do not edit `AGENTS.md` directly.

**Research:** Read `AGENTS.md` (read-only) to locate the policy statements.

**Acceptance Criteria:**

- [ ] A unified diff is attached to the PR description for the owner to apply.
- [ ] `AGENTS.md` is not modified by the PR.

**Dependencies:** T-12-5-002  
**Reference:** Plan Phase 12 Sprint 5 ("proposed as a diff, since that file holds the user's
uncommitted edits")  
**Estimated Complexity:** S

---

### T-12-5-004 — `CLAUDE.md` project block wording

**Description:** Update the safety bullet in the `CLAUDE.md` managed project block from "zero
`unsafe` code anywhere" to the precise `icx-shm` policy, through the managed-block mechanism.

**Research:** Confirm how the managed block is regenerated so the change is not overwritten.

**Acceptance Criteria:**

- [ ] The project block states the precise policy and survives regeneration.

**Dependencies:** T-12-5-002  
**Reference:** Plan Phase 12 Sprint 5; `CLAUDE.md` §4  
**Estimated Complexity:** S

---

### T-12-5-005 — README: rewrite the bus note beside the patent summary

**Description:** Remove or rewrite the README note beside the patent summary so it states what
now ships: the safe file-backed bus by default, and the opt-in memory-mapped seqlock bus in
`icx-shm`.

**Research:** Re-read the patent summary to keep the note accurate without changing legal text.

**Acceptance Criteria:**

- [ ] The note matches the shipped implementation; legal text is unchanged.

**Dependencies:** T-12-5-001  
**Reference:** ADR-0007 §Context (README note); issue #32  
**Estimated Complexity:** S

---

### T-12-5-006 — Release notes and upgrade note for `icx-shm`

**Description:** Write the `icx-shm` section of the v2.0.0 release notes and an upgrade note:
how to enable `shm-bus`, set `bus.backend`, read `icx doctor`'s backend report, and what falls
back and why.

**Research:** Verify every command in the notes on a clean install.

**Acceptance Criteria:**

- [ ] `CHANGELOG.md` entry and upgrade note committed; commands tested.

**Dependencies:** T-12-4-003, T-12-5-001  
**Reference:** Module 70 (release notes); issue #32  
**Estimated Complexity:** S

---

### T-12-5-007 — ADR-0007 implemented and issue #32 closed with links

**Description:** Mark ADR-0007 `implemented` with links to the PRs, then close issue #32 with a
comment linking the PRs, the release and the measurements.

**Research:** Collect PR numbers and the release URL once published.

**Acceptance Criteria:**

- [ ] ADR-0007 status `implemented`, with PR links.
- [ ] Issue #32 closed with links to the PRs, the release and `docs/numbers.md`.

**Dependencies:** T-12-5-001, T-12-5-006, T-12-9-004  
**Reference:** Issue #32; `docs/adr/0007-opt-in-icx-shm-bus.md`  
**Estimated Complexity:** S

---

### T-12-5-008 — Decision on `auto` preferring shm in a later release

**Description:** Record that `bus.backend = auto` keeps selecting the file bus in v2.0, and the
criteria under which a later release may prefer shm, decided only after one release of field
use.

**Research:** Define the field evidence required (fallback rates, bug reports, measured gain).

**Acceptance Criteria:**

- [ ] Decision and criteria recorded in ADR-0007 (or its successor) and in `ROADMAP.md`.

**Dependencies:** T-12-5-001  
**Reference:** Plan Phase 12 Sprint 5; issue #32  
**Estimated Complexity:** S

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes, with and without
  `--features shm-bus`.
- [ ] `cargo test --workspace` passes, with and without `--features shm-bus`.
- [ ] Coverage >= 95% (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Docs and `CHANGELOG.md` (`[Unreleased]`) updated in the same change.
- [ ] Upstream re-scan for Sprint 6 done, its tickets adjusted, and findings recorded in its
  Research notes.
