# Agent Install and Session-Storage Survey: Public Summary

**Original survey:** 2026-11-16, of the owner's own local installs, for interoperability.

**This page:** a public summary written on 2026-11-22. The file name is kept so that existing
links still resolve.

## Why this page is a summary

On 2026-11-22 the owner moved the original survey's detail into the gitignored
`ref-docs/agent-internals/private/`. Only the interoperability conclusions remain public. This is
a deliberate exception to the corpus's rule that files are never edited. Earlier revisions stay
in git history.

Later investigations follow the same rule ([ADR-0008](../../docs/adr/0008-reverse-engineering-output-stays-private.md)):
public pages state interoperability conclusions only.

## Conclusions that informed icx

| Agent | What icx relies on |
| :--- | :--- |
| Claude Code | Documented surfaces only: hooks, MCP and the session transcript. |
| OpenAI Codex CLI | Hooks, MCP, and the local rollout files. Codex is open source under Apache-2.0. |
| Antigravity CLI (`agy`) | Hooks (`~/.gemini/config/hooks.json`) and MCP. icx does not read agy's internal session storage. |
| Hermes Agent | The `ContextEngine` plugin interface. Hermes is open source under MIT. |
| `ai-memory` | Coexists with icx; icx does not depend on its internals. |

## Where the detail lives now

- **Locally, never published:** the full 2026-11-16 survey, under
  `ref-docs/agent-internals/private/`.
- **The Phase 6 Sprint 1 investigation (v1.4.0):** its public findings and go/no-go decisions
  are in the dated `below-hook-findings-*.md` file in this directory.
- **Cross-agent hook contract:** [cross-agent-hook-and-wire-matrix.md](cross-agent-hook-and-wire-matrix.md).
