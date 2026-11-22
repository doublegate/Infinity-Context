# Sprint 3 — Log and Test-Output Templating

**Phase:** Phase 10 — Prevent and Prove  
**Release:** v1.8.0  
**Sprint Goal:** Collapse repetitive logs into templates and summarise test failures, enabled by
default only if a measured reduction over the field corpus justifies it.  
**Estimated Duration:** 1.5 weeks

## Research gate

Re-check before any ticket starts:

- **External docs and versions:** the `logdrain` crate (version, license, maintenance, API) and the
  Drain algorithm description; Drain3 for reference behaviour; current cargo, pytest and jest
  output formats including JSON reporters.
- **Installed agent behaviour:** how each agent displays long tool output, so summaries are placed
  where the model reads them.
- **Project evidence:** the `StructuralSkeletonizer` in `icx-codec`; the field corpus of logs and
  test runs; the redactor's interaction with masked fields.

### Research notes

_None recorded yet._

## Tickets

### T-10-3-001 — Evaluate `logdrain` against an in-house Drain

**Description:** Compare the `logdrain` crate with a small in-house Drain implementation on the
field corpus: template quality, speed, dependency cost and license compatibility.

**Research:** Hypothesis: one of the two produces fewer templates without merging distinct errors.
Measurement: template count, error-line preservation and time on the field corpus. The decision
and its data are recorded in an ADR.

**Acceptance Criteria:**

- [ ] Both options are measured on the same corpus.
- [ ] License and `cargo deny` checks pass for the chosen option.
- [ ] An ADR records the decision and the measurements.

**Dependencies:** None  
**Reference:** `logdrain` (<https://crates.io/crates/logdrain>); He et al., "Drain: An Online Log
Parsing Approach with Fixed Depth Tree" (ICWS 2017); plan Phase 10 Sprint 3  
**Estimated Complexity:** M

---

### T-10-3-002 — Drain-style template mining in the skeletonizer

**Description:** Add template mining to the skeletonizer with masks for UUIDs, IP addresses and
hex values, emitting each template once with counts and keeping every distinct error line verbatim.

**Research:** Confirm masks do not hide values the redactor needs to see first (redaction runs
before templating).

**Acceptance Criteria:**

- [ ] Distinct error lines are never merged into a template (golden tests).
- [ ] Masks apply only after redaction.
- [ ] The original text stays retrievable through the stub handle.

**Dependencies:** T-10-3-001  
**Reference:** plan Phase 10 Sprint 3; `docs/alien-holographic-codec.md`  
**Estimated Complexity:** L

---

### T-10-3-003 — Test-failure summarisation for cargo, pytest and jest

**Description:** Summarise test runs to the failing tests, their assertion messages and locations,
plus pass/fail counts, for cargo, pytest and jest output.

**Research:** Confirm output formats on current versions, including parallel and JSON reporters.

**Acceptance Criteria:**

- [ ] Every failing test name, message and location from the input appears in the summary (golden
  tests per runner).
- [ ] Counts match the runner's own totals.
- [ ] Unknown formats pass through unchanged.

**Dependencies:** None  
**Reference:** plan Phase 10 Sprint 3  
**Estimated Complexity:** M

---

### T-10-3-004 — Gate templating on a measured reduction

**Description:** Enable templating and summaries by default only if they reduce output over the
field corpus without losing error information; otherwise ship them opt-in and record why.

**Research:** Hypothesis: templating reduces tokens on the field corpus with no lost error lines.
Measurement: token reduction and error-line recall on the corpus. A rejected hypothesis leaves the
feature opt-in with the finding.

**Acceptance Criteria:**

- [ ] Reduction and error-line recall are measured and published in `docs/numbers.md`.
- [ ] The default (on or opt-in) follows the measurement and is documented.

**Dependencies:** T-10-3-002, T-10-3-003  
**Reference:** plan Phase 10 Sprint 3  
**Estimated Complexity:** S

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Line coverage is `>= 95%` (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Affected docs and `CHANGELOG.md` `[Unreleased]` are updated.
- [ ] Upstream re-scan for Sprint 4 is done and its tickets are adjusted.
