# Sprint 4 — Redaction

**Phase:** Phase 7 — Retrieval and Compaction Quality

**Release:** v1.5.0

**Sprint Goal:** Let users re-check stores written under older rules, replace the
"first quote to end of line" rule with a structural redactor, and close the mutation gaps in
`redact.rs`.

**Estimated Duration:** 2 weeks

## Research gate

Re-check before the sprint starts:

- **External docs and versions:** current gitleaks rule set and comparable secret scanners for
  detector coverage; `proptest` and `cargo-mutants` versions via Context7.
- **Installed agent behaviour:** not agent-facing, except that redaction output must keep the
  shapes pinned by the Phase 6 contract fixtures.
- **Project evidence:** `docs/releases/v1.3.1/README.md` line 68 (stored content is not
  re-checked); the v1.3.0 redactor split decision (#29); the field over-redaction log;
  `docs/claude/12-v1.1.0-remediation-status.md` line 47 (ICX-AUD-039 mutation results).

### Research notes

_None recorded yet._

## Tickets

### T-74-001 — `icx rescan` with a dry-run default

**Description:** Add `icx rescan` that runs the current redactor over stored artifacts and
reports what it would redact. With explicit confirmation it re-redacts by writing new CAS
objects and updating references, never by editing objects in place.

**Research:** Confirm how stored objects are referenced (DAG, graph, snapshots), so a
re-redaction leaves no dangling or unredacted reference.

**Acceptance Criteria:**

- [ ] Dry-run is the default and writes nothing (tested).
- [ ] Report lists counts per detector and artifact handles, never the secret values.
- [ ] Re-redaction is atomic per workspace and leaves a backup or the prior snapshot.
- [ ] Merkle integrity verified after re-redaction.

**Dependencies:** None

**Reference:** `docs/releases/v1.3.1/README.md` line 68

**Estimated Complexity:** M

---

### T-74-002 — Structural redactor ADR

**Description:** Write an ADR for a structural redactor that understands quoting, JSON, YAML,
TOML, env files and shell assignments, replacing the "first quote to end of line" rule.

**Research:** Collect the over-redaction cases from the field log and the under-redaction cases
from existing tests; compare with how gitleaks scopes matches.

**Acceptance Criteria:**

- [ ] ADR in Nygard format under `docs/adr/`, cross-linking the v1.3.0 split decision (#29).
- [ ] The ADR lists the formats handled and the fallback for unknown text.
- [ ] Owner review recorded before T-74-003 starts.

**Dependencies:** None

**Reference:** Issue #29; v1.3.1 release notes; field over-redaction log (session scan
2026-11-22)

**Estimated Complexity:** M

---

### T-74-003 — Implement the structural redactor

**Description:** Implement the redactor from T-74-002, with property tests and a corpus built
from the field over-redaction log.

**Research:** Hypothesis: the structural redactor catches every secret the current redactor
catches and redacts fewer non-secret bytes. Measured on the corpus as missed secrets (must be
zero) and over-redacted bytes, before and after.

**Acceptance Criteria:**

- [ ] Every existing redaction test passes unchanged.
- [ ] The false positives in #90 are fixed and in the corpus: key names containing `token`,
  `key` or `session` no longer blank `None`, `null`, `""`, numbers, booleans or call expressions,
  and a redaction covers the value only, not the rest of the line.
- [ ] Property tests: no secret from a generated set survives in any supported format; output
  never panics on arbitrary input.
- [ ] Missed secrets (zero required) and over-redacted bytes measured and published in
  `docs/numbers.md`.
- [ ] `docs/alien-holographic-codec.md` and `docs/security-and-threat-model.md` updated.

**Dependencies:** T-74-002

**Reference:** Plan Phase 7 Sprint 4; issues #29 and #90

**Estimated Complexity:** XL

---

### T-74-004 — Close the mutation gaps in `redact.rs`

**Description:** Re-run mutation testing on `redact.rs` after T-74-003 and add tests that kill
the surviving mutants that matter.

**Research:** Re-run `cargo-mutants` at the current commit; triage survivors into real gaps and
equivalent mutants.

**Acceptance Criteria:**

- [ ] Mutation results recorded with commit and counts.
- [ ] Every non-equivalent surviving mutant is killed or documented with a reason.
- [ ] ICX-AUD-039 status updated in `docs/claude/findings.csv`.

**Dependencies:** T-74-003

**Reference:** ICX-AUD-039 (`docs/claude/12-v1.1.0-remediation-status.md` line 47)

**Estimated Complexity:** M

## Sprint exit

- [ ] Gates green: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and
  coverage `>= 95%`.
- [ ] Docs and CHANGELOG updated for any user-visible change; v1.5.0 release notes drafted.
- [ ] Upstream re-scan done for Phase 8 Sprint 1 (provider caching docs), and its tickets
  adjusted.
