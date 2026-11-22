# Sprint 9 — Markdown Export and Import

**Phase:** Phase 11 — Govern and Distribute  
**Release:** v1.9.0  
**Sprint Goal:** Export folds, summaries and the policy file to git-friendly Markdown and import
them back, following the basic-memory and Cline memory-bank pattern, as the groundwork for the
v2.0 sync decision; then close out v1.9.0.  
**Estimated Duration:** 1.5 weeks

## Research gate

Before any ticket starts:

- Re-read basic-memory's and the Cline memory bank's file layouts and front-matter conventions.
- Confirm the fold and summary structures in `crates/icx-core` and what an import must rebuild
  (DAG links, index postings).
- Re-run the redactor golden vectors; export goes through the redactor and the policy layer.
- Re-read the Phase 12 Sprint 9 sync requirements so the export format does not block them.

### Research notes

_None recorded yet._

## Tickets

### T-11-9-001 — Markdown export of folds, summaries and policy

**Description:** `icx export --markdown <dir>` writes one file per fold or summary with front
matter (handle, time range, agent, source hashes) and a copy of the policy file, after policy
and redaction.

**Research:** Choose the file naming and front-matter fields after comparing basic-memory and the
Cline memory bank.

**Acceptance Criteria:**

- [ ] Denied and `<private>` content is never exported (tested).
- [ ] Output is deterministic: two exports of the same state are byte-identical.
- [ ] Format documented and versioned in `docs/data-model.md`.

**Dependencies:** T-11-1-001, T-11-1-002, T-92-001 (`--markdown` is a second output format of
the same `icx export` subcommand; `--format <v>` and `--markdown` are mutually exclusive)  
**Reference:** Plan Phase 11 Sprint 9; basic-memory; Cline memory bank  
**Estimated Complexity:** M

---

### T-11-9-002 — Markdown import with validation

**Description:** `icx import --markdown <dir>` validates front matter and rebuilds folds,
summaries and index entries; it refuses files that fail validation and reports them. Dry-run by
default.

**Research:** Decide how imported content is marked (provenance) and that it is treated as
untrusted data on recall.

**Acceptance Criteria:**

- [ ] Export then import round-trips to an equivalent store (property test).
- [ ] Malformed or oversized files are refused with a clear error (fuzz target on the parser).
- [ ] Imported content carries provenance and the untrusted-data marker.

**Dependencies:** T-11-9-001  
**Reference:** Plan Phase 11 Sprint 9; module 60 (fuzz untrusted-input parsers)  
**Estimated Complexity:** M

---

### T-11-9-003 — Sync groundwork note for v2.0

**Description:** Record how the export format and policy file would carry team or cross-machine
sync, as input to the Phase 12 Sprint 9 privacy ADR.

**Research:** Review the conflict behaviour of exports under git merge.

**Acceptance Criteria:**

- [ ] Note committed under `docs/` and linked from the Phase 12 Sprint 9 ticket.

**Dependencies:** T-11-9-001  
**Reference:** Plan Phase 11 Sprint 9 ("groundwork the v2.0 sync decision builds on")  
**Estimated Complexity:** S

---

### T-11-9-004 — v1.9.0 release closeout

**Description:** Run the release ceremony: version bump, `CHANGELOG.md` `[Unreleased]` renamed to
`1.9.0`, docs sweep, paired benchmark check, tag and release with SBOM, signatures and every
channel from Sprint 4.

**Research:** Run the Phase 10 paired benchmark under its noise-aware rule before tagging.

**Acceptance Criteria:**

- [ ] No significant regression on the Phase 10 paired benchmark.
- [ ] Every Phase 11 exit criterion in `overview.md` checked.
- [ ] Release published with all assets and channel artifacts.

**Dependencies:** All Phase 11 tickets  
**Reference:** Module 70 (release ceremony); Phase 10 Sprint 5 paired benchmark  
**Estimated Complexity:** M

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Coverage >= 95% (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Docs and `CHANGELOG.md` updated in the same change.
- [ ] Upstream re-scan for Phase 12 Sprint 1 done, its tickets adjusted, and findings recorded in
  its Research notes.
