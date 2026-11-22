# Sprint 4 — Packaging and Distribution

**Phase:** Phase 11 — Govern and Distribute  
**Release:** v1.9.0  
**Sprint Goal:** Ship `icx` as a Claude Code plugin, a Codex plugin (once verified), an MCP
Registry entry with a `.mcpb` bundle, and `dist` releases across Homebrew, cargo-binstall, an
installer script, AUR and npm, with the names reserved first.  
**Estimated Duration:** 2 weeks

## Research gate

Before any ticket starts:

- Re-read the Claude Code plugin documentation on the installed version: manifest fields, hooks,
  MCP servers, skills, `bin/`, `userConfig`, and `claude plugin validate --strict`.
- Re-check whether Codex plugins can bundle hooks on the installed version; confirm with REA
  where the docs are silent.
- Re-read the MCP Registry `server.json` schema, the `.mcpb` bundle format, and the registry's
  GitHub OIDC publishing flow.
- Check the current `dist` (cargo-dist) release, its installer and package targets, and pin it.
- Re-read rtk's notes on package-name squatting and check availability of `icx` and
  `infinity-context` on every channel.
- Confirm the Phase 10 Sprint 6 codebase-gap findings that gate crates.io publishing.

### Research notes

_None recorded yet._

## Tickets

### T-11-4-001 — Reserve the `icx` and `infinity-context` names on every channel

**Description:** Claim the names on crates.io, npm, Homebrew (tap), AUR, the MCP Registry
namespace and the plugin marketplaces before anything is announced, recording which are taken and
the fallback name used.

**Research:** Check each channel's current owner of both names and its reservation or
placeholder policy.

**Acceptance Criteria:**

- [ ] A table of channel, name, status and owner is committed to `docs/build-and-tooling.md`.
- [ ] Placeholders published where the channel allows, with a pointer to the repository.

**Dependencies:** None  
**Reference:** Plan Phase 11 Sprint 4; rtk name-squatting notes  
**Estimated Complexity:** S

---

### T-11-4-002 — Claude Code plugin

**Description:** Package hooks, the MCP server, a skill, `bin/` and `userConfig` as a Claude Code
plugin, without a `claude-` name prefix, validated with `claude plugin validate --strict` in CI.

**Research:** Confirm every manifest field against the installed Claude Code version and that a
plugin-installed hook resolves the plugin's own `icx` binary.

**Acceptance Criteria:**

- [ ] `claude plugin validate --strict` passes in CI (pinned Claude Code version).
- [ ] A clean install from the plugin registers hooks and the MCP server, and `icx doctor` reports
  them as pointing at the plugin's binary.
- [ ] Plugin name has no `claude-` prefix.

**Dependencies:** T-11-4-001  
**Reference:** Plan Phase 11 Sprint 4; Claude Code plugin documentation  
**Estimated Complexity:** M

---

### T-11-4-003 — Codex plugin (conditional on hook bundling)

**Description:** Package the Codex integration as a Codex plugin, only once its hook bundling is
verified on the installed version.

**Research:** Hypothesis: Codex plugins can bundle the hooks `icx` needs. Confirming measurement:
a test plugin whose hook fires under both interactive and `codex exec` turns. Rejection closes
the ticket with the finding and keeps the current `icx init` setup for Codex.

**Acceptance Criteria:**

- [ ] Verification result recorded in the sprint Research notes.
- [ ] If confirmed: plugin installs and hooks fire in both modes (tested); if rejected: decision
  documented in `docs/deployment-and-agent-integration-guide.md`.

**Dependencies:** T-11-4-001  
**Reference:** Plan Phase 11 Sprint 4; Codex CLI plugin documentation  
**Estimated Complexity:** M

---

### T-11-4-004 — MCP Registry `server.json` and `.mcpb` bundle via GitHub OIDC

**Description:** Publish `icx` to the MCP Registry with a `server.json`, build a `.mcpb` bundle,
and publish both from `release.yml` using GitHub OIDC rather than a stored token.

**Research:** Confirm the current `server.json` schema version and OIDC flow; validate a draft
locally with the registry's tooling.

**Acceptance Criteria:**

- [ ] `server.json` validates against the pinned schema in CI.
- [ ] `.mcpb` bundle installs in a client that supports it (tested on one client).
- [ ] Publishing uses OIDC; no long-lived registry secret is stored.

**Dependencies:** T-11-4-001, T-84-002  
**Reference:** Plan Phase 11 Sprint 4; MCP Registry documentation; `.mcpb` format  
**Estimated Complexity:** M

---

### T-11-4-005 — `dist` releases: Homebrew, cargo-binstall, installer, AUR, npm

**Description:** Adopt `dist` for release artifacts and generate the Homebrew formula,
cargo-binstall metadata, an installer script, an AUR PKGBUILD and an npm wrapper package, for
every platform the Phase 9 builds produce.

**Research:** Confirm `dist` support for each target and channel at the pinned version, and
whether it composes with the existing `release.yml` steps.

**Acceptance Criteria:**

- [ ] A tag build produces every listed artifact; each installs `icx` on a clean runner.
- [ ] `dist` version pinned in the workflow.
- [ ] Installer script verifies checksums before installing.

**Dependencies:** T-11-4-001, T-93-004  
**Reference:** Plan Phase 11 Sprint 4; `dist` documentation  
**Estimated Complexity:** L

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Coverage >= 95% (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Docs and `CHANGELOG.md` (`[Unreleased]`) updated in the same change.
- [ ] Upstream re-scan for Sprint 5 done, its tickets adjusted, and findings recorded in its
  Research notes.
