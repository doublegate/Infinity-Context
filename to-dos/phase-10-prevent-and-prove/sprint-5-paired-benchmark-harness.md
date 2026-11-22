# Sprint 5 — Paired Benchmark Harness

**Phase:** Phase 10 — Prevent and Prove  
**Release:** v1.8.0  
**Sprint Goal:** Prove the savings with a paired benchmark (same agent and model, with and without
`icx`) whose release gate responds to statistically significant regressions and not to run-to-run
noise, within a recorded cost budget.  
**Estimated Duration:** 2 weeks

## Research gate

Re-check before any ticket starts:

- **External docs and versions:** the current Terminal-Bench 2.0 harness (version, task set, how
  agents are plugged in), its guidance on repeated runs, and the leaderboard token and cost
  fields. Do not quote competitors' self-reported figures as comparable to ours.
- **Installed agent behaviour:** which installed agents the harness can drive, and whether each
  supports a fixed temperature and seed.
- **Project evidence:** the Sprint 1 to 4 ledger, so per-task token savings can be cross-checked;
  `docs/numbers.md` and the Phase 8 generated-table schema; the side-agent review of 2026-11-22:
  pass rate on 89 tasks moves several points between identical runs, and each paired full run is
  about 180 paid agent sessions.

### Research notes

_None recorded yet._

## Tickets

### T-10-5-001 — `icx bench --harness terminal-bench`

**Description:** Add `icx bench --harness terminal-bench` to run the same agent and model on the
same tasks with and without `icx`, and report per task and overall: pass rate, input tokens and
cost per task.

**Research:** Confirm how the harness isolates runs and whether `icx` state can be reset per task
so runs do not leak context into each other.

**Acceptance Criteria:**

- [ ] Each task runs in both arms with `icx` state reset between tasks.
- [ ] Per-task pass/fail, input tokens and cost are recorded for both arms.
- [ ] Output is in the Phase 8 benchmark dataset schema.

**Dependencies:** T-83-005  
**Reference:** Terminal-Bench (<https://github.com/laude-institute/terminal-bench>); plan Phase 10
Sprint 5  
**Estimated Complexity:** L

---

### T-10-5-002 — Run manifest and hold-equal rule

**Description:** Write a run manifest for every run that records model, agent version, harness
version, temperature and seed where supported, plus the agent, model and maximum spend. Runs are
comparable only when all of these are equal; a run with any of them changed is not comparable and
is rerun.

**Research:** Confirm which of these fields each agent and the harness expose, and record the ones
that cannot be fixed.

**Acceptance Criteria:**

- [ ] Every run writes a manifest with all hold-equal fields.
- [ ] The comparison step refuses to compare runs whose hold-equal fields differ, and says which
  field differs.
- [ ] Fields that cannot be fixed for an agent are listed in the published results.

**Dependencies:** T-10-5-001  
**Reference:** plan Phase 10 Sprint 5 (side-agent review, 2026-11-22)  
**Estimated Complexity:** S

---

### T-10-5-003 — Baseline variance from repeated paired runs

**Description:** Before any gate is used, run at least three repeated paired runs (with and
without `icx`) under one manifest and record the run-to-run spread of pass rate per task and
overall, and of tokens and cost.

**Research:** Hypothesis: run-to-run spread on the full task set is several points of pass rate,
as the review reported. Measurement: the spread across the repeated runs. The measured spread,
whatever it is, becomes the noise baseline.

**Acceptance Criteria:**

- [ ] At least three repeated paired runs complete under identical manifests.
- [ ] Per-task and overall spread for pass rate, tokens and cost are measured and published in
  `docs/numbers.md`.
- [ ] Tasks that flip between runs in the same arm are listed as unstable.

**Dependencies:** T-10-5-001, T-10-5-002  
**Reference:** plan Phase 10 Sprint 5 (side-agent review, 2026-11-22)  
**Estimated Complexity:** M

---

### T-10-5-004 — Statistical release gate

**Description:** Block a release only on a pass-rate drop that is statistically significant across
repeated paired runs. The test is paired per task on the same tasks: McNemar's test on per-task
outcomes, or a paired bootstrap of the per-task pass-rate difference, two-sided at the 95%
confidence level (the owner may change the level; the gate reads it from configuration and records
it in the manifest). A drop larger than the measured spread (T-10-5-003) also blocks. Single-run
drops within the spread are reported and do not block.

**Research:** Confirm with a simulation on the baseline data that the chosen test at the stated
level does not flag identical-configuration runs more often than its nominal rate.

**Acceptance Criteria:**

- [ ] The gate implements McNemar or a paired bootstrap on per-task pairs, at a stated confidence
  level recorded in the manifest.
- [ ] A drop larger than the measured spread blocks.
- [ ] A single-run drop within the spread is reported and does not block (test).
- [ ] The false-positive rate on identical-configuration runs is measured and published in
  `docs/numbers.md`.

**Dependencies:** T-10-5-003  
**Reference:** plan Phase 10 Sprint 5; McNemar, "Note on the sampling error of the difference
between correlated proportions or percentages", Psychometrika 12 (1947)  
**Estimated Complexity:** M

---

### T-10-5-005 — Cost budget, smoke subset and when the full suite runs

**Description:** Enforce a budget per run (agent, model and maximum spend in the manifest; the run
stops at the cap). Define a cheaper smoke subset of 15 to 20 tasks for per-PR checks. The full
paired suite runs only at release candidates, or when a change touches stubbing, rewriting or
recall.

**Research:** Pick the smoke tasks from the baseline runs: stable tasks that exercise stubbing,
rewriting and recall. Confirm the path filters that decide when a change touches those areas.

**Acceptance Criteria:**

- [ ] A run stops at its spend cap and records that it stopped.
- [ ] The smoke subset has 15 to 20 tasks, chosen and justified from the baseline data.
- [ ] CI runs the smoke subset on PRs and the full suite only on release candidates or on changes
  to stubbing, rewriting or recall.
- [ ] Cost per smoke and full run is measured and published in `docs/numbers.md`.

**Dependencies:** T-10-5-002, T-10-5-003  
**Reference:** plan Phase 10 Sprint 5 (side-agent review, 2026-11-22)  
**Estimated Complexity:** M

---

### T-10-5-006 — Publish results and manifests

**Description:** Publish paired results, the variance baseline, the test and confidence level, and
every run manifest in `docs/numbers.md` through the generated table, without comparing against
competitors' self-reported figures.

**Research:** Confirm the leaderboard fields so published token figures use the same definitions.

**Acceptance Criteria:**

- [ ] Results, manifests, spread and the gate's outcome are published in `docs/numbers.md`.
- [ ] No competitor self-reported figure is presented as comparable.
- [ ] README and release notes link to the published rows instead of repeating numbers.

**Dependencies:** T-10-5-004, T-10-5-005  
**Reference:** plan Phase 10 Sprint 5; `docs/numbers.md`  
**Estimated Complexity:** S

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Line coverage is `>= 95%` (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Affected docs and `CHANGELOG.md` `[Unreleased]` are updated.
- [ ] Upstream re-scan for Sprint 6 is done and its tickets are adjusted.
