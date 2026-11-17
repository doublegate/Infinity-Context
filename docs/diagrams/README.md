# Architecture & Relationship Diagrams (`docs/diagrams/`)

This directory contains normative Mermaid (`.mmd`) diagram sources for **Infinity-Context (`icx`)**:

- [`five-plane-architecture.mmd`](five-plane-architecture.mmd) — Complete 5-plane Virtual Context MMU dataflow across Claude Code, OpenAI Codex CLI, Google Antigravity CLI (`agy`), and Hermes Agent.
- [`crate-and-agent-relationships.mmd`](crate-and-agent-relationships.mmd) — Strict acyclic 8-crate Rust workspace dependency graph (`icx-core` through `icx-cli`).
- [`synchronous-hook-and-shm-flow.mmd`](synchronous-hook-and-shm-flow.mmd) — Sequence diagram of synchronous `PostToolUse` / `UserPromptSubmit` hook interception, BLAKE3 CAS storage, AHTL encoding, and `/dev/shm` Seqlock broadcast.
- [`cache-epoch-lifecycle.mmd`](cache-epoch-lifecycle.mmd) — State machine of the Prefix-Cache-Locked Epoch Controller (`>= 94%` KV-cache hit rate).

High-resolution Graphviz (`.dot`, `.png`, `.svg`) relationship diagrams live in [`../../images/`](../../images/), and vector theme-adaptive SVGs live in [`../images/`](../images/).
