# Sprint 8 — Documentation Site

**Phase:** Phase 11 — Govern and Distribute  
**Release:** v1.9.0  
**Sprint Goal:** Turn the Zensical docs site into a user manual: a getting-started tutorial,
configuration and CLI references (the latter generated from clap), troubleshooting and FAQ,
rustdoc on Pages, a numbers table generated from benchmark JSON, and a recorded i18n decision.  
**Estimated Duration:** 1.5 weeks

## Research gate

Before any ticket starts:

- Re-read the current Zensical site configuration and the Pages workflow from v1.3.1 (PR #35).
- Check how to publish rustdoc beside the Zensical site on Pages without path collisions.
- Re-read claude/11 §G (generated numbers table) and the Phase 8 benchmark dataset schema.
- Collect the most frequent support questions from closed issues for the FAQ.

### Research notes

_None recorded yet._

## Tickets

### T-11-8-001 — Getting-started tutorial

**Description:** A tutorial from install to the first recalled stub, for one agent end to end,
with links for the others.

**Research:** Walk the tutorial on a clean machine with the Sprint 4 installers.

**Acceptance Criteria:**

- [ ] Every command in the tutorial runs as written on a clean runner (tested in CI where
  possible).

**Dependencies:** T-11-4-005  
**Reference:** Plan Phase 11 Sprint 8  
**Estimated Complexity:** S

---

### T-11-8-002 — Configuration reference

**Description:** A page listing every config key and env var, its default, precedence and
security notes, generated from the Sprint 6 schema.

**Research:** Confirm the schema exposes defaults and descriptions in a machine-readable form.

**Acceptance Criteria:**

- [ ] Generated from code; CI fails if it is stale.

**Dependencies:** T-11-6-002  
**Reference:** Plan Phase 11 Sprint 8  
**Estimated Complexity:** S

---

### T-11-8-003 — CLI reference generated from clap

**Description:** Generate the CLI reference pages from the clap definitions.

**Research:** Choose between `clap_mangen` output conversion and a custom generator.

**Acceptance Criteria:**

- [ ] Generated in CI; stale pages fail the docs build.

**Dependencies:** T-11-6-001  
**Reference:** Plan Phase 11 Sprint 8  
**Estimated Complexity:** S

---

### T-11-8-004 — Troubleshooting and FAQ

**Description:** Troubleshooting by symptom (repeated prompts, missing stubs, daemon down, hook
not firing per agent) and an FAQ, each pointing at the matching `icx doctor` check.

**Research:** Collect symptoms from closed issues and session logs.

**Acceptance Criteria:**

- [ ] Each entry names the `icx doctor` check that detects it.

**Dependencies:** T-11-6-003  
**Reference:** Plan Phase 11 Sprint 8  
**Estimated Complexity:** S

---

### T-11-8-005 — rustdoc published to Pages

**Description:** Build `cargo doc --no-deps` with warnings as errors and publish it under the
docs site.

**Research:** Confirm the Pages layout and that intra-doc links to feature-gated items do not
break the default build.

**Acceptance Criteria:**

- [ ] rustdoc builds with `-D warnings` and is linked from the site.

**Dependencies:** T-11-7-002  
**Reference:** Plan Phase 11 Sprint 8; module 40 (docs built with warnings as errors)  
**Estimated Complexity:** S

---

### T-11-8-006 — Generate the `v1.2.0 candidate` label in the asset script

**Description:** Replace the hard-coded `v1.2.0 candidate` label in
`scripts/generate_assets_and_graphs.py` (around L557, L602 and L606) with a value derived from
the workspace version.

**Research:** Confirm the current line numbers and every other hard-coded version in the script.

**Acceptance Criteria:**

- [ ] The label follows the workspace version; regenerated assets show the current release.

**Dependencies:** None  
**Reference:** Plan Phase 11 Sprint 8; `scripts/generate_assets_and_graphs.py`  
**Estimated Complexity:** S

---

### T-11-8-007 — Numbers table generated from benchmark JSON

**Description:** Generate the tables in `docs/numbers.md` from benchmark JSON output, so no
figure is typed by hand.

**Research:** Confirm the Phase 8 benchmark dataset schema and the Phase 10 paired-benchmark
output.

**Acceptance Criteria:**

- [ ] `docs/numbers.md` tables regenerate from JSON; CI fails if they are stale.
- [ ] Each figure carries methodology, hardware and date.

**Dependencies:** T-83-005  
**Reference:** Plan Phase 11 Sprint 8; claude/11 §G  
**Estimated Complexity:** M

---

### T-11-8-008 — Record the i18n decision

**Description:** Record a decision to defer internationalisation, with the conditions that would
reopen it.

**Research:** Check whether Zensical supports localisation and what it would cost.

**Acceptance Criteria:**

- [ ] Decision recorded in `docs/build-and-tooling.md` (or a short ADR).

**Dependencies:** None  
**Reference:** Plan Phase 11 Sprint 8 ("i18n deferred, with a recorded decision")  
**Estimated Complexity:** S

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Coverage >= 95% (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Docs and `CHANGELOG.md` (`[Unreleased]`) updated in the same change.
- [ ] Upstream re-scan for Sprint 9 done, its tickets adjusted, and findings recorded in its
  Research notes.
