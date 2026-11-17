# Phase 0 — Pre-Cursory Setup, Deep Research, REA Forensics & Workspace Bootstrap

## Goal

Complete all pre-cursory research, academic literature acquisition, Reverse-Engineer-Anything (`rea`) binary/SQLite forensics across the local agent ecosystem, implementation specification derivation (`docs/`), hierarchical phase/sprint planning (`to-dos/`), `ai-memory` project onboarding, and the initial compiling/tested 8-crate Rust workspace bootstrap so the project is 100% ready for Phase 1 production expansion.

## Exit Criteria

- [x] Initial concept documented in `ref-docs/initial-concept.md`.
- [x] 22 primary arXiv research PDFs downloaded and verified in `ref-docs/papers/` with mathematical index (`ref-docs/papers/INDEX.md`).
- [x] Shipped agent binaries (`claude` `2.1.326`, `codex` `0.169.0`, `agy`, `ai-memory`) and SQLite databases (`state_5.sqlite`, `memories_1.sqlite`, `conversation_summaries.db`, `conversations/*.db`, `state.db`) reverse-engineered via REA (`ev_*` Evidence IDs recorded in `ref-docs/agent-internals/`).
- [x] Complete implementation specification suite (13 docs + 4 ADRs) written in `docs/`.
- [x] Hierarchical roadmap and sprint tickets minted in `to-dos/`.
- [x] `.ai-memory.toml` and durable pinned wiki pages (`concepts/`, `decisions/`, `gotchas/`) saved in `ai-memory` under `default/doublegate-infinity-context`.
- [x] 8-crate Rust Cargo workspace (`icx-core`, `icx-hdc`, `icx-codec`, `icx-store`, `icx-bus`, `icx-adapters`, `icx-proxy`, `icx-cli`) implemented, formatted, lint-clean, and passing all unit/integration/benchmark gates.
