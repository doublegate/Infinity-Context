# REA Binary and Session-Storage Survey: Public Summary

**Original survey:** 2026-11-16, Reverse-Engineer-Anything (REA) static inspection of the owner's
own local installs, for interoperability.

**This page:** a public summary written on 2026-11-22.

## Why this page is a summary

The original ledger recorded binary-level detail about the installed builds: ELF note offsets,
REA evidence IDs, recovered build and module metadata, and the schemas of the agents' local session
databases. Claude Code, the Antigravity CLI (`agy`) and Muse Code are proprietary, so on
2026-11-22 the owner moved that detail into the gitignored `ref-docs/agent-internals/private/`.
Only the interoperability conclusions remain public. This is a deliberate exception to the
corpus's rule that files are never edited. Earlier revisions stay in git history.

Findings from later investigations follow the same rule. Raw REA and Ghidra output never enters
the repository, and public pages state interoperability conclusions only (see `.gitignore`).

## Conclusions that informed icx

| Agent | Build form (as surveyed) | What icx relies on |
| :--- | :--- | :--- |
| Claude Code | A single-file executable bundling a JavaScript runtime and the application. | Only documented surfaces: hooks, MCP and the session transcript. |
| OpenAI Codex CLI | A static Rust binary. Open source under Apache-2.0, so source reading is the primary method. | Hooks, MCP, and the local rollout files. |
| Antigravity CLI (`agy`) | A Go binary. | Hooks (`~/.gemini/config/hooks.json`) and MCP. Session data is kept locally, but icx does not read its internal format. |
| Hermes Agent | Python, run from an MIT-licensed source checkout. | The `ContextEngine` plugin interface. |
| `ai-memory` | A Rust binary. | Coexists with icx; icx does not depend on its internals. |

## Where the detail lives now

- **Locally, never published:** the full 2026-11-16 ledger at
  `ref-docs/agent-internals/private/rea-binary-and-sqlite-forensics.full-2026-11-16.md`.
- **The Phase 6 Sprint 1 investigation (v1.4.0):**
  - its public findings, scope table and go/no-go decisions are in the dated
    `below-hook-findings-*.md` file in this directory;
  - its raw evidence is under `private/`.
- **Cross-agent hook contract:** [cross-agent-hook-and-wire-matrix.md](cross-agent-hook-and-wire-matrix.md).
