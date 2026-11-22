# Phase 11 — Govern and Distribute (v1.9.0)

## Goal

Give users control over what `icx` stores and for how long, make the stored context inspectable
and portable, and ship `icx` through the channels users already install from. Phase 11 adds the
per-project policy file, `<private>` spans, `icx forget`/`icx purge`, a timeline and repo map, a
read-only local viewer, plugin and registry packaging, OpenTelemetry GenAI/MCP span names, a clap
CLI with a config file, a versioned daemon JSON-RPC API, crates.io readiness with a signed supply
chain, a documentation site, and Markdown export and import.

Privacy and policy (Sprint 1) land before the viewer (Sprint 3) and the export (Sprint 9), so that
nothing denied by policy is ever displayed or exported.

## Exit Criterion

- [ ] `.icx/policy.toml` is enforced on every store, viewer and export path, and `<private>` spans
  are never written to the CAS (tested).
- [ ] `icx forget` and `icx purge` run with `--dry-run` and report exactly what would be removed.
- [ ] The viewer serves read-only on loopback behind a token, and absorbs the Phase 8 dashboard.
- [ ] The Claude Code plugin passes `claude plugin validate --strict` in CI; the MCP Registry
  entry and `.mcpb` bundle are published with GitHub OIDC; `dist` produces every listed channel.
- [ ] The CLI runs on clap derive with `--version`, completions and a man page, and every env var
  has a config-file equivalent with documented precedence (env > file > default).
- [ ] The daemon exposes a versioned JSON-RPC API that the proxy and pager share.
- [ ] Release artifacts carry a CycloneDX SBOM and signatures, and the reproducible-build check
  passes in `release.yml`.
- [ ] The documentation site publishes the tutorial, references, FAQ and rustdoc.
- [ ] Folds, summaries and policy round-trip through Markdown export and import.
- [ ] No default telemetry: every network call `icx` can make is listed in the docs.
- [ ] No significant regression on the Phase 10 paired benchmark, judged by its noise-aware rule.

## Sprints

- [Sprint 1 — Privacy and Policy](sprint-1-privacy-and-policy.md)
- [Sprint 2 — Timeline and Repo Map](sprint-2-timeline-and-repo-map.md)
- [Sprint 3 — Local Viewer](sprint-3-local-viewer.md)
- [Sprint 4 — Packaging and Distribution](sprint-4-packaging-and-distribution.md)
- [Sprint 5 — Standards and Approvals](sprint-5-standards-and-approvals.md)
- [Sprint 6 — CLI, Config and Daemon API](sprint-6-cli-config-and-daemon-api.md)
- [Sprint 7 — Library and Supply Chain](sprint-7-library-and-supply-chain.md)
- [Sprint 8 — Documentation Site](sprint-8-documentation-site.md)
- [Sprint 9 — Markdown Export and Import](sprint-9-markdown-export-and-import.md)

## Dependencies on Earlier Phases

- **Phase 7 (v1.5.0), Sprint 4 — Redaction:** export and the viewer reuse the redactor; nothing
  they show or write bypasses it.
- **Phase 8 (v1.6.0), Sprint 3 — Observability:** the per-hook traces that Sprint 5 renames to
  OTel GenAI/MCP spans, the `icx stats` data, and the optional dashboard the viewer absorbs.
- **Phase 8 (v1.6.0), Sprint 4 — MCP 2026-07-28 alignment:** the MCP surface that the registry
  entry and `.mcpb` bundle describe.
- **Phase 9 (v1.7.0), Sprint 1 — Storage:** the store the retention, TTL and `icx gc` policies act
  on.
- **Phase 9 (v1.7.0), Sprint 3 — Platforms:** the aarch64, macOS and Windows builds that `dist`
  packages.
- **Phase 10 (v1.8.0), Sprint 4 — Savings ledger:** the savings view in the viewer.
- **Phase 10 (v1.8.0), Sprint 5 — Paired benchmark harness:** the no-regression rule.
- **Phase 10 (v1.8.0), Sprint 6 — Code health:** the codebase-gap findings that decide whether the
  crates are published to crates.io.

Phase 12 depends on this phase for the config file (`bus.backend`), the daemon JSON-RPC API (the
`icx-shm` socket-routing comparison), and the Markdown export (the sync decision).

## Standing Sources to Watch

- Claude Code changelog and plugin documentation (`claude plugin validate`, `userConfig`,
  managed settings, `allowManagedHooksOnly`).
- OpenAI Codex CLI changelog, plugin and hook-bundling documentation.
- Google Antigravity CLI (`agy`), Hermes Agent and Meta Muse Code release notes.
- MCP specification changelog, the MCP Registry `server.json` schema, and the `.mcpb` bundle
  format.
- OpenTelemetry GenAI and MCP semantic conventions (status is Development; pin a version).
- `dist` (cargo-dist), cargo-binstall, Homebrew, AUR and npm packaging guidance.
- `clap`, `tree-sitter`, `pyo3`, `cargo-semver-checks`, CycloneDX and Sigstore/cosign releases
  (Context7 for library APIs).
- Comparable tools: Aider (repo map), basic-memory and the Cline memory bank (Markdown export),
  context-mode (telemetry practice), rtk (name-squatting notes).

## Reference Docs

- [docs/security-and-threat-model.md](../../docs/security-and-threat-model.md)
- [docs/interfaces.md](../../docs/interfaces.md)
- [docs/build-and-tooling.md](../../docs/build-and-tooling.md)
- [docs/deployment-and-agent-integration-guide.md](../../docs/deployment-and-agent-integration-guide.md)
- [Phase 11 implementation plan](phase_11_implementation_plan.md)
