# ADR 0001: Five-Plane Hybrid Interception Over Single-Surface Integration

## Status

Historical decision (2026-11-16); capability and measurement claims superseded by
[ADR 0006](0006-v1-2-0-context-and-compatibility.md). Preserve the original rationale below.

## Context

AI coding and research agents on a developer workstation (Claude Code `2.1.326`, OpenAI Codex CLI `0.169.0`, Google Antigravity CLI `agy`, Hermes Agent) have heterogeneous binary runtimes (Bun ELF, static-pie musl Rust, Go/C++ ELF, Python) and transport layers (configurable HTTP base URLs vs. OAuth-pinned gRPC/mTLS). Relying solely on an MCP server fails to prevent bulky `Bash`/`Read` outputs from bloating the context window, while relying solely on an HTTP reverse proxy fails on OAuth-locked or non-HTTP transports.

## Decision

We adopt a **Five-Plane Hybrid Interception Architecture**:

1. **Plane 0 (Wire Proxy & SQLite/WAL Tailer):** Intercepts HTTP/2 & SSE traffic where base URLs are configurable and tails local SQLite WALs (`state_5.sqlite`, `conversations/*.db`, `state.db`) and JSONL transcripts for locked transports.
2. **Plane 1 (Native Lifecycle Hooks):** Uses each agent's synchronous mutation hooks (`updatedToolOutput`, `injectSteps`, `pre-model-request`, `ContextEngine.select_context`) to virtualize bulky tool outputs before LLM ingestion.
3. **Plane 2 (AHTL Codec & Titans Surprise Gate):** Compresses context into non-human BPE-aligned symbolic frames.
4. **Plane 3 (LCM BLAKE3 Merkle-DAG):** Stores 100% lossless byte-exact chunks with `#icx:<hash>` pointers.
5. **Plane 4 & 5 (HDC/Fractal/PPR Index + `/dev/shm` Seqlock Bus + Single-Tool MCP Pager):** Provides `< 50 µs` associative retrieval and `< 100 µs` cross-agent telepathy.

## Consequences

- **Positive:** 100% coverage across every agent CLI/IDE regardless of network transport; zero-loss tool-output virtualization before tokens are billed; sub-millisecond hook execution.
- **Negative:** Requires maintaining per-agent wire schema translators in `crates/icx-adapters`, mitigated by comprehensive schema conformance tests against the REA-extracted schemas.
