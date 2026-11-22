# Phase 11 (v1.9.0) Implementation Plan — Govern and Distribute

## Goal Description

Deliver v1.9.0 in nine sprints. The phase puts users in control of stored context (policy,
private spans, forget and purge), makes that context visible (timeline, repo map, viewer) and
portable (Markdown export and import), and moves `icx` onto standard distribution channels with a
signed, reproducible supply chain. It also closes the codebase gaps that block later work: the
hand-written CLI parser, env-only configuration, a shallow `icx doctor`, and the lack of a daemon
API that the proxy and pager can share.

## Ordered Approach

1. **Sprint 1 — Privacy and policy first.** `<private>` spans, `.icx/policy.toml`, `icx forget`
   and `icx purge` with `--dry-run`, and the config-file groundwork. Every later sprint that
   reads stored data (viewer, export, timeline) goes through the policy layer built here.
2. **Sprint 2 — Timeline and repo map.** `op:"timeline"` over the bi-temporal graph, and a
   tree-sitter repo map ranked by the existing PPR code, with `repo-mapper` evaluated first.
3. **Sprint 3 — Local viewer.** Read-only, loopback-only, token-gated; it replaces the Phase 8
   optional dashboard rather than running beside it.
4. **Sprint 4 — Packaging.** Reserve names on every channel before anything is announced, then
   the Claude Code plugin, the Codex plugin (only once its hook bundling is verified), the MCP
   Registry entry and `.mcpb` bundle, and `dist` releases.
5. **Sprint 5 — Standards and approvals.** OTel GenAI/MCP span names pinned to one semconv
   version, the managed-settings hooks policy, and install-time tool pre-approval.
6. **Sprint 6 — CLI, config and daemon API.** clap derive, `config.toml` with `icx config`,
   the deeper `icx doctor`, `icx gc`, and the versioned JSON-RPC API, which also carries the
   event-routing path the Phase 12 `icx-shm` comparison measures.
7. **Sprint 7 — Library and supply chain.** crates.io metadata and dry-run publish, API gates,
   the optional `pyo3` binding, SBOM, signatures and the reproducible-build check.
8. **Sprint 8 — Documentation site.** Tutorial, configuration and CLI references (the latter
   generated from clap), FAQ, rustdoc on Pages, the generated numbers table, and the i18n
   decision.
9. **Sprint 9 — Markdown export and import.** Folds, summaries and policy as git-friendly
   Markdown; the groundwork for the v2.0 sync decision.

Sprint 6 is sequenced after Sprint 4 in numbering only. The config file is needed by Sprints 1
and 4 in a minimal form; Sprint 1 lands the file schema and loader, and Sprint 6 adds the CLI
surface and full env-var coverage on top of it.

## Risks

- **Policy bypass.** A new read path (viewer, export, timeline) that forgets the policy layer
  would leak denied content. Mitigation: one policy-checked accessor in the store, and tests that
  each consumer refuses denied paths.
- **Viewer exposure.** A loopback web server is reachable by any local process and by DNS
  rebinding from a browser. Mitigation: bind to loopback only, require a per-launch token, check
  the `Host` header, serve read-only.
- **Name squatting.** Channel names (`icx`, `infinity-context`) may be taken before release.
  Mitigation: reserve them in the first sprint ticket of Sprint 4.
- **Unstable conventions.** OTel GenAI/MCP semantic conventions are in Development and may
  rename attributes. Mitigation: pin a semconv version and record it in the docs.
- **Plugin formats moving.** Claude Code and Codex plugin manifests change between releases.
  Mitigation: validate against the locally installed agent version in each sprint's research
  gate, and run `claude plugin validate --strict` in CI.
- **Supply-chain tooling drift.** Unpinned packaging and signing tools can redden CI without a
  code change. Mitigation: pin every tool version in the workflows.
- **CLI regression.** Moving to clap can change accepted flag spellings. Mitigation: a
  compatibility test over every flag form accepted today, including the v1.3.1 flag-value fixes.

## Verification

- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, and `cargo llvm-cov` at `--fail-under-lines 95` on every sprint PR.
- Policy tests: denied paths and tools never reach the CAS, the viewer or the export.
- `claude plugin validate --strict` and `cargo publish --dry-run` in CI.
- `cargo-semver-checks` against the last published API.
- The reproducible-build check in `release.yml` produces identical artifacts from two builds.
- The Phase 10 paired benchmark shows no significant regression, judged by its noise-aware rule.
- Every new performance figure is measured and published in `docs/numbers.md` with its
  methodology; no figure is claimed before it is measured.
