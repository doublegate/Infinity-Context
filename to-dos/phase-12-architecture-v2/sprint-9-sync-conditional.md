# Sprint 9 — Sync (Conditional) and v2.0.0 Release

**Phase:** Phase 12 — Architecture Release  
**Release:** v2.0.0  
**Sprint Goal:** Decide team and cross-machine sync through a privacy ADR, build it only if the
ADR approves a model (redacted content only, end-to-end encryption) on top of the v1.9 Markdown
export and policy file, otherwise record the decision not to build it; then release v2.0.0.  
**Estimated Duration:** 2 weeks

## Research gate

Before any ticket starts:

- Re-read the Phase 11 sync groundwork note (T-11-9-003), the Markdown export format and the
  policy file.
- Survey comparable tools' sync models (git-backed Markdown in basic-memory and the Cline memory
  bank) and end-to-end encryption options.
- Re-run the redactor golden vectors; only redacted content may ever leave the machine.

### Research notes

_None recorded yet._

## Tickets

### T-12-9-001 — Privacy ADR for sync

**Description:** An ADR that approves or rejects a sync model. An approvable model syncs redacted
content only, uses end-to-end encryption, builds on the v1.9 Markdown export and policy file
rather than a new protocol, and honours policy denials on every machine.

**Research:** Threat-model the candidate model (compromised remote, compromised peer, policy
disagreement between machines).

**Acceptance Criteria:**

- [ ] ADR committed with an explicit approve or reject decision.

**Dependencies:** T-11-9-003  
**Reference:** Plan Phase 12 Sprint 9; Phase 12 overview ("Team sync builds on the v1.9 Markdown
export")  
**Estimated Complexity:** M

---

### T-12-9-002 — Build sync (only if the ADR approves)

**Description:** Implement the approved model: export, encrypt, transfer through a user-chosen
remote (for example a git repository), decrypt and import, with policy enforced on both ends.

**Research:** Confirm the encryption crate and key exchange chosen in the ADR via Context7.

**Acceptance Criteria:**

- [ ] Unredacted or denied content never leaves the machine (tested).
- [ ] Two machines converge on the same folds and summaries (integration test).
- [ ] Off by default; listed in the network-call inventory.

**Dependencies:** T-12-9-001, T-11-9-002  
**Reference:** Plan Phase 12 Sprint 9  
**Estimated Complexity:** XL

---

### T-12-9-003 — Record the decision not to build (if the ADR rejects)

**Description:** If the ADR rejects sync, record the decision and the conditions that would reopen
it in `ROADMAP.md`, and close T-12-9-002 unbuilt.

**Research:** None beyond the ADR.

**Acceptance Criteria:**

- [ ] Decision recorded in `ROADMAP.md` and `CHANGELOG.md`.

**Dependencies:** T-12-9-001  
**Reference:** Plan Phase 12 Sprint 9 ("Otherwise this sprint records the decision not to build
it")  
**Estimated Complexity:** S

---

### T-12-9-004 — v2.0.0 release closeout

**Description:** Run the release ceremony: version bump to 2.0.0, `CHANGELOG.md` `[Unreleased]`
renamed, upgrade notes (format break and `icx-shm`), docs sweep, paired benchmark under the
noise-aware rule, tag and release with SBOM and signatures through every Phase 11 channel.

**Research:** Run the Phase 10 paired benchmark and the Sprint 3 qualification gate on the release
commit.

**Acceptance Criteria:**

- [ ] No significant regression on the Phase 10 paired benchmark.
- [ ] Sprint 3 qualification gate green on the release commit.
- [ ] Every Phase 12 exit criterion in `overview.md` checked; release published with all assets.

**Dependencies:** All other Phase 12 tickets except T-12-5-007, which follows the release  
**Reference:** Module 70 (release ceremony); Phase 10 Sprint 5 paired benchmark  
**Estimated Complexity:** M

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes, with and without
  `--features shm-bus`.
- [ ] `cargo test --workspace` passes, with and without `--features shm-bus`.
- [ ] Coverage >= 95% (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Docs and `CHANGELOG.md` updated in the same change.
- [ ] Upstream re-scan for the next release done and recorded in `ROADMAP.md`.
