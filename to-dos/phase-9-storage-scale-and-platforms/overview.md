# Phase 9 — Storage, Scale and Platforms

**Release:** v1.7.0

## Goal

Make the workspace store scale with long sessions and parallel agents (incremental saves, CAS
compression, an index for cold grep, less lock contention), make upgrades and downgrades safe,
ship builds for aarch64 Linux, macOS and Windows, and close the open performance qualification
tickets with remeasured, published numbers.

## Exit Criterion

- [ ] Saving derived state is incremental, CAS compression is on with a measured ratio, and
  `grep_cold` uses an index; each change is measured and published in `docs/numbers.md`.
- [ ] `icx rescan --apply` re-redacts a stored workspace atomically, keeps old handles resolvable,
  and leaves no re-redacted secret in the database file (#102).
- [ ] `icx export --format <v>` writes a snapshot an older supported release can read, downgrade
  paths are documented, and an MCP server running against a newer snapshot format tells the user
  to reconnect instead of failing silently.
- [ ] Release artifacts build and pass tests on x86_64 and aarch64 Linux, macOS and Windows, and
  an `ubuntu-24.04-arm` CI runner is live for the v2.0 `icx-shm` stress runs (#32).
- [ ] T-120-024 and T-120-025 are closed, and the HDC kernels are remeasured and published.

## Sprints

- [Sprint 1 — Storage](sprint-1-storage.md)
- [Sprint 2 — Compatibility](sprint-2-compatibility.md)
- [Sprint 3 — Platforms](sprint-3-platforms.md)
- [Sprint 4 — Performance](sprint-4-performance.md)

## Dependencies on Earlier Phases

- Phase 1 (`CasStore`, `MerkleContextDag`) and Phase 3 (HDC index) for the storage and kernel
  work.
- Phase 7 Sprint 1 trigram index: Sprint 1 reuses it for `grep_cold` rather than building a second
  one.
- Phase 8 Sprint 3 traces and generated numbers table: every measurement in this phase is
  published through it.
- `to-dos/codex-audit-remediation.md` for T-120-024 and T-120-025.

## Standing Sources to Watch

- Rust release notes and `std::arch` / `is_x86_feature_detected!` changes; target tier changes for
  `aarch64-unknown-linux-gnu`, `aarch64-apple-darwin` and `x86_64-pc-windows-msvc`.
- `zstd`, `rustix`, `memmap2` and file-locking crate releases (Context7).
- GitHub Actions runner images, including `ubuntu-24.04-arm`, `macos-*` and `windows-*` changes.
- Claude Code, Codex CLI, Antigravity CLI, Hermes and Muse changelogs (MCP server lifecycle and
  reconnect behaviour, platform support).
- MCP specification changelog.
- Comparable local-context tools (context-mode, rtk) for storage and platform choices.

## Reference Docs

- [docs/virtual-memory-and-dag-engine.md](../../docs/virtual-memory-and-dag-engine.md)
- [docs/data-model.md](../../docs/data-model.md)
- [docs/build-and-tooling.md](../../docs/build-and-tooling.md)
- [docs/performance.md](../../docs/performance.md)
- [to-dos/codex-audit-remediation.md](../codex-audit-remediation.md)
