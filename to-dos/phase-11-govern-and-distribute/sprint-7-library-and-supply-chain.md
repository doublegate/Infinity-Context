# Sprint 7 — Library and Supply Chain

**Phase:** Phase 11 — Govern and Distribute  
**Release:** v1.9.0  
**Sprint Goal:** Make the crates publishable and their APIs gated (metadata, `missing_docs`,
`cargo-semver-checks`, an MSRV decision), add an optional `pyo3` binding for the codec and
redactor, and sign every release artifact with an SBOM and a reproducible-build check.  
**Estimated Duration:** 1.5 weeks

## Research gate

Before any ticket starts:

- Check current `cargo-semver-checks`, `pyo3`/`maturin`, CycloneDX (`cargo-cyclonedx`) and
  cosign/minisign versions via Context7 or release notes, and pin them.
- Re-read crates.io metadata requirements and categories list.
- Re-read the Hermes shim to confirm which codec and redactor functions it would call.
- Review the reproducible-build approach (`SOURCE_DATE_EPOCH`, `--remap-path-prefix`, locked
  dependencies) for the release targets.

### Research notes

_None recorded yet._

## Tickets

### T-11-7-001 — crates.io metadata, READMEs, examples and dry-run publish

**Description:** Add `repository`, `readme`, `keywords`, `categories` and `documentation` to every
crate, per-crate READMEs and `examples/`, and run `cargo publish --dry-run` for each crate in CI.

**Research:** Confirm category slugs and that the license expression is accepted by crates.io.

**Acceptance Criteria:**

- [ ] `cargo publish --dry-run` passes for every crate in CI.
- [ ] Each crate has a README and at least one compiling example.

**Dependencies:** None  
**Reference:** Plan Phase 11 Sprint 7  
**Estimated Complexity:** M

---

### T-11-7-002 — API gates: `missing_docs`, `cargo-semver-checks`, MSRV

**Description:** Enable `missing_docs` on the public traits, add a `cargo-semver-checks` gate
against the last release, and record a decision on the library MSRV with a pinned-toolchain job.

**Research:** Measure the current undocumented public surface and choose the MSRV from the
features the code uses.

**Acceptance Criteria:**

- [ ] `missing_docs` passes under `-D warnings`.
- [ ] `cargo-semver-checks` runs on every PR (pinned version).
- [ ] MSRV decision recorded in `docs/build-and-tooling.md` and enforced by a CI job.

**Dependencies:** T-11-7-001  
**Reference:** Plan Phase 11 Sprint 7  
**Estimated Complexity:** M

---

### T-11-7-003 — Optional `pyo3` binding for the codec and redactor

**Description:** An optional Python package exposing the AHTL codec and the secret redactor, used
by the Hermes shim, built with `maturin`, off by default in the Rust workspace.

**Research:** Hypothesis: the binding removes the Hermes shim's subprocess round trip at an
acceptable packaging cost. Confirming measurement: shim latency with and without the binding.
Rejection keeps the subprocess path and records the finding.

**Acceptance Criteria:**

- [ ] Redactor output from Python matches the Rust golden vectors byte for byte (parity test).
- [ ] Latency comparison measured and published in `docs/numbers.md`.
- [ ] Default builds unchanged with the binding off.

**Dependencies:** None  
**Reference:** Plan Phase 11 Sprint 7; `crates/icx-codec`; Hermes adapter  
**Estimated Complexity:** L

---

### T-11-7-004 — CycloneDX SBOM for every release

**Description:** Generate a CycloneDX SBOM in `release.yml` and attach it to each release.

**Research:** Confirm the SBOM tool covers the whole workspace dependency graph.

**Acceptance Criteria:**

- [ ] Each release has an SBOM asset that validates against the CycloneDX schema.

**Dependencies:** None  
**Reference:** Plan Phase 11 Sprint 7; CycloneDX  
**Estimated Complexity:** S

---

### T-11-7-005 — Signed release artifacts

**Description:** Sign every artifact and the checksum file with cosign (keyless, OIDC) or
minisign, and document verification.

**Research:** Choose cosign or minisign after checking how each downstream channel verifies.

**Acceptance Criteria:**

- [ ] Every asset has a signature that verifies with the documented command (tested in CI).

**Dependencies:** T-11-7-004  
**Reference:** Plan Phase 11 Sprint 7; Sigstore cosign; minisign  
**Estimated Complexity:** S

---

### T-11-7-006 — Reproducible-build check in `release.yml`

**Description:** Build the release binaries twice in independent jobs and compare digests.

**Research:** Identify sources of nondeterminism (paths, timestamps, build IDs) on each target.

**Acceptance Criteria:**

- [ ] Two independent builds produce identical digests for each supported target, or the
  remaining differences are documented with a tracking issue.

**Dependencies:** None  
**Reference:** Plan Phase 11 Sprint 7  
**Estimated Complexity:** M

---

### T-11-7-007 — crates.io publishing decision

**Description:** Decide whether to publish the crates to crates.io in v1.9.0, based on the
Phase 10 Sprint 6 codebase-gap findings and Sprint 7's API gates.

**Research:** Review the codebase-gap findings and the `cargo publish --dry-run` result from
T-11-7-001.

**Acceptance Criteria:**

- [ ] Decision and rationale recorded in `docs/build-and-tooling.md`.
- [ ] If publishing: crates published in dependency order after T-11-7-001 and T-11-7-002 pass.

**Dependencies:** T-11-7-001, T-11-7-002, T-10-6-001 to T-10-6-008  
**Reference:** Plan Phase 11 Sprint 4 ("depends on the codebase-gap findings")  
**Estimated Complexity:** S

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Coverage >= 95% (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Docs and `CHANGELOG.md` (`[Unreleased]`) updated in the same change.
- [ ] Upstream re-scan for Sprint 8 done, its tickets adjusted, and findings recorded in its
  Research notes.
