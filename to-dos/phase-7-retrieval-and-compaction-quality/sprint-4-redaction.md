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

2026-11-25:

- **Scoping in other scanners:** gitleaks reports the capture group that holds the secret value
  (`SecretGroup`), with keyword identifiers as a prefilter (Context7, `/gitleaks/gitleaks`
  v8.30). That is the value-only scope ADR-0011 adopts; icx still hides more than gitleaks
  reports where the value is code (#29).
- **Field evidence:** probed on `main` (80f4b94) with byte-span output, because icx's own hook
  redacts tool output with the same redactor and so hides what is being measured. 10 of 17
  representative lines lost bytes they should have kept (`docs/adr/0011-structural-redactor.md`).
- **Tools:** `proptest` 1.x as already pinned in the workspace; `cargo mutants -f <file>`
  writes `mutants.out/` with `caught.txt`/`missed.txt` (Context7, `/sourcefrog/cargo-mutants`).
- **Pinned tests found during T-74-003:** a 15-digit `ACCESS_TOKENS` value is pinned as a secret,
  so the numeric bound is "more than 10 digits" (as for the counter rule), not the 16 first
  drafted; a harmless value followed by a quoted comment (`get_pw(); // "..."`) must still hide
  the quote.
- **Security review of the first implementation** (automated, on c93a5e3) found three bypasses,
  closed with tests in `redact_structural_v150.rs` (`review_bypasses_stay_closed`): a literal
  glued to more text (`PASSWORD="ab"cd` is one shell word), `$`-prefixed passwords taken for
  variables, and INI values (`password = my.dog.rex`) taken for code references.
- **T-74-001 reference map** (2026-11-25): artifact ids are content hashes, and a changed artifact
  cascades through the Merkle-DAG `Turn`/`ToolArtifact` digests, branches, L3/L4 epochs and the bus
  root. Derived copies of the text sit in packets, frames, graph entities, HDC postings, FTS5 bodies
  and bus slots. Old handles in transcripts stop resolving without an alias table, and deleted rows
  stay in SQLite free pages until `VACUUM`. Full map in #102; owner decision: report only in v1.5.0.
- **First field run of `icx rescan`:** 169 of 4,694 artifacts on this project's workspace, all
  stored by versions before v1.3.1. The current redactor is idempotent on all of them (a second
  pass changes nothing), and its rules are unchanged from v1.4.1, which confirms that the
  findings are older content and not a regression. A second review
  (on 5a5f1c9) found three more, also closed with tests: `#`, `/` or `,` glued to a literal in
  shell text (`PASSWORD="a"#b`), a bare call such as `abc(def)` taken for code, and upper-case
  `$NAME` values without an `_` taken for references.

## Tickets

### T-74-001 — `icx rescan` with a dry-run default

**Description:** Add `icx rescan` that runs the current redactor over stored artifacts and
reports what it would redact. Owner decision (2026-11-25): v1.5.0 ships the report only; the
re-redaction step is T-91-005 in v1.7.0 (#102), because the store is content-addressed throughout
(see the research notes).

**Research:** Confirm how stored objects are referenced (DAG, graph, snapshots), so a
re-redaction leaves no dangling or unredacted reference.

**Acceptance Criteria:**

- [x] Dry-run is the default and writes nothing (tested): v1.5.0 reports only, and `--apply` is
  refused with a pointer to #102.
- [x] Report lists counts per detector and artifact handles, never the secret values (tested).
- [ ] Re-redaction (atomic, with a snapshot, Merkle integrity verified) moved to T-91-005 (#102).

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

- [x] ADR in Nygard format under `docs/adr/`, cross-linking the v1.3.0 split decision (#29).
- [x] The ADR lists the formats handled and the fallback for unknown text.
- [x] Owner review recorded before T-74-003 starts.

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

- [x] Every existing redaction test passes; every assertion that a secret is absent is unchanged,
  and the six that pinned the old output shape (a literal hidden with its quotes and the rest of
  its line, a short number under `auth_tokens`) pin the new one.
- [x] The false positives in #90 are fixed and in the corpus: key names containing `token`,
  `key` or `session` no longer blank `None`, `null`, `""`, numbers, booleans or call expressions,
  and a redaction covers the value only, not the rest of the line.
- [x] Property tests: no secret from a generated set survives in any supported format (27 shapes,
  7 keys, 2,000 cases); output never panics on arbitrary input.
- [x] Missed secrets (zero required) and over-redacted bytes measured and published in
  `docs/numbers.md`: 0 missed before and after; 183 bytes to 24.
- [x] `docs/alien-holographic-codec.md` and `docs/security-and-threat-model.md` updated.

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
