# Phase 5 — Long-Horizon Verification, Benchmarks & Production Packaging

## Goal

Validate the complete 8-crate **Infinity-Context (`icx`)** workspace against multi-million-token Needle-in-a-Haystack / OOLONG simulations, verify cross-agent `/dev/shm` telepathy across Claude Code, Codex, Antigravity CLI, and Hermes, and ship the `icx` CLI (`init`, `hook`, `mcp-serve`, `proxy`, `encode-ahtl`, `decode-ahtl`, `deref`, `status`, `doctor`, `benchmark`).

## Exit Criteria

- [x] Multi-agent integration test proves that a bug/panic ingested by Claude Code (`PostToolUse`) is immediately broadcast over `/dev/shm`, received by Antigravity CLI (`PreInvocation`) and Codex (`UserPromptSubmit`), and losslessly dereferenced via the `icx` MCP tool.
- [x] Long-horizon simulation (200+ turns) achieves `>= 15x` token compression, `>= 94%` prefix-cache hit rate, and `100%` passkey retrieval accuracy.
- [x] `icx benchmark` and `icx doctor` execute cleanly and pass all SLO checks.

## Sprints

- [Sprint 1 — Multi-Million-Token Needle-in-a-Haystack & Prefix-Cache Simulator](sprint-1-nih-and-prefix-cache-sim.md)
- [Sprint 2 — CLI Tooling, `icx doctor`, `icx benchmark` & Release Packaging](sprint-2-cli-doctor-and-packaging.md)

## Reference Docs

- [docs/testing-strategy.md](../../docs/testing-strategy.md)
- [docs/performance.md](../../docs/performance.md)
