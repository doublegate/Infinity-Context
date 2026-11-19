# 06 — Agent Integration vs Current Upstream APIs

Workstream W6. Verified 2026-11-18 against upstream documentation and the installed binaries:
Claude Code 2.1.328, codex-cli 0.170.0, agy 1.5.11, and the Hermes source at `dc394c6`. No agent
sessions were started and no live API calls were made. Items marked *(inferred)* follow from the
documentation but were not exercised live.

Verdicts: **CONFIRMED**, **CONTRADICTED**, **PARTIAL**, **UNVERIFIABLE**.

## ICX-AUD-004 — Claude Code: several hook outputs never reach the model (High)

Source: <https://code.claude.com/docs/en/hooks>.

| Project behaviour (location) | Upstream | Verdict |
| --- | --- | --- |
| Event names; `additionalContext` on SessionStart, UserPromptSubmit and SubagentStart | as documented | CONFIRMED |
| PostToolUse `updatedToolOutput` for built-in tools (`claude.rs:117-128`) | Supported for built-in tools; "the value must match the tool's output shape" | PARTIAL: icx always emits a **string**, while Bash `tool_response` is an object. Shape compatibility needs a live test; the binary contains the error text "updatedToolOutput is not the REPL tool's Output shape" |
| PreCompact / PostCompact `systemMessage` (`claude.rs:168-180`) | `systemMessage` is a "Warning message shown to the user". PreCompact *stdout* becomes compaction instructions | CONTRADICTED: the virtualization context icx sends here is shown to the user or discarded, never given to the model |
| SubagentStop reads `summary` / `outcome` (`claude.rs:155-166`) | The input field is `last_assistant_message`; `systemMessage` goes to the user | CONTRADICTED: subagent folding receives no outcome text |
| PreCompact "exit 2 blocks compaction" (deployment guide :857-860) | The mechanism exists | PARTIAL: `icx hook` always exits 0 (`main.rs:34-45`) and never emits `decision: "block"` |
| `Stop` hook configured in the guide | The event exists | icx has no handler and returns `{}` |
| PreToolUse echo (`claude.rs:71-80`) | `permissionDecision` and `updatedInput` are output-only | CONTRADICTED: dead code (ICX-AUD-009) |
| "Claude Code 2.1.326+" | — | UNVERIFIABLE (installed version is 2.1.328) |

**Remediation.** Route model-facing content through the fields the model actually reads:
`additionalContext` where it is supported, and PreCompact stdout for compaction instructions.
Use `last_assistant_message`. Add contract tests that pin each event's input and output schema to
fixtures captured from a real Claude Code run.

## ICX-AUD-005 — Codex: output replacement is unsupported; the documented config is wrong (High)

Source: Codex hooks documentation (developers.openai.com/codex/hooks), plus the local `~/.codex`.

| Project behaviour | Upstream | Verdict |
| --- | --- | --- |
| `~/.codex/hooks.json` exists | yes | CONFIRMED |
| Guide config (`docs/deployment-and-agent-integration-guide.md:286-345`): kebab-case events, array `command`, `timeout_ms`, `[features] codex_hooks = true` | PascalCase events, string `command`, `timeout` in seconds; `codex_hooks` is a deprecated alias and hooks are on by default | CONTRADICTED |
| `updatedMCPToolOutput` replaces MCP output | "parsed but not supported yet" | CONTRADICTED: no replacement happens |
| `additionalContext` for native tools | Appended alongside the full output | Net effect: **tokens increase**, the opposite of virtualization |
| `PreModelRequest` for context control | Diagnostic only | CONTRADICTED |
| Rollout tailer for `~/.codex/sessions/**/rollout-*.jsonl` | 482 of 520 local rollout files are `.jsonl.zst`; Codex is moving to `thread_history_1.sqlite` | PARTIAL: no zstd support. The tailer is also not wired into the CLI at all (only tests use it) |
| `state_5.sqlite` `threads.rollout_path`; `[mcp_servers.icx]` | as documented | CONFIRMED |

## ICX-AUD-006 — Hermes: the documented integration cannot load (High)

Source: the local Hermes repository.

- **ContextEngine method names:** CONFIRMED (`agent/context_engine.py:85-181`). The real
  signatures differ from those the adapter assumes; for example there are no `protect_*`
  parameters.
- **Python shim in the guide (:465-598):** cannot be instantiated. It omits the abstract `name`,
  `update_from_response` and `should_compress`, and the `plugins/` directory it references does
  not exist in this repository.
- **`~/.hermes/hooks.yaml` with SessionStart, PostToolUse and PreCompact:** CONTRADICTED.
  - Shell hooks live under `hooks:` in `~/.hermes/config.yaml` and use snake_case events
    (`pre_tool_call`, `on_session_start`, …; `hermes_cli/plugins.py:109`).
  - Unknown event names are refused (`agent/shell_hooks.py:224`).
  - Gateway hooks use `~/.hermes/hooks/<name>/HOOK.yaml`.
- **`on_pre_compress`:** CONFIRMED.

## ICX-AUD-007 — Antigravity (`agy`): three contradictory config locations; wrong schema (Medium)

Source: <https://antigravity.google/docs/hooks.md>.

| Project claim | Verdict |
| --- | --- |
| `~/.gemini/hooks.json` with `BeforeModel` / `AfterTool` / `PreCompress` (README :136, :271) | CONTRADICTED: these are Gemini CLI event names |
| `~/.gemini/antigravity-cli/hooks.json` (guide :382-384) | CONTRADICTED: not an official location, and absent locally |
| `~/.gemini/config/hooks.json` and `.agents/hooks.json` (`icx doctor`, hook matrix) | CONFIRMED |
| Config shape: top-level `"hooks"` plus `command` / `args` | CONTRADICTED: upstream uses named groups, a single `command` string, and `timeout` in seconds |
| Events SessionStart, PreToolUse, PostToolUse, PreInvocation, PostInvocation, Stop | CONFIRMED; `SubagentStart`, `SubagentStop` and `SessionEnd` are UNVERIFIABLE |
| Stop reasons `MAX_TOKEN_BUDGET_EXCEEDED` / `MAX_INVOCATIONS` (`antigravity.rs:459`) | PARTIAL: the documented reasons are `model_stop`, `max_steps_exceeded` and `error` |
| PreToolUse response | `decision` is required; icx defaults to `allow` for every tool, which may override the user's normal approval flow *(inferred)* |
| `transcriptPath` in the payload | CONFIRMED |

## ICX-AUD-008 — MCP protocol version is hard-coded and outdated (Medium)

- `icx mcp-serve` always answers `initialize` with `"protocolVersion": "2024-11-05"`
  (`mcp.rs:154`), ignoring the version the client requests.
- `server/discover` returns -32601 (method not found).
- The current MCP specification (2026-07-28) removes the `initialize` handshake, carries the
  version on every request, and makes `server/discover` mandatory; 2025-11-25 is the last
  revision with `initialize`.

So icx works with clients that perform the legacy handshake, and fails with clients that speak
only the current revision. Newline-delimited JSON-RPC framing, `notifications/initialized`, and
`tools/call` results with `content[]` plus `isError` are correct.

- **Remediation:** negotiate the version (echo the highest mutually supported one), implement
  `server/discover`, and add a conformance test against the official MCP schema.

## ICX-AUD-003 — The Anthropic proxy's cache strategy likely *reduces* prompt-cache hits (High)

Verified facts (<https://platform.claude.com/docs/en/build-with-claude/prompt-caching>):

- Up to 4 breakpoints are allowed.
- `cache_control` is placed "directly on individual content blocks" or "at the top level"; message
  objects are not a documented location.
- The lookback is 20 positions per breakpoint.
- The TTL is 5 minutes by default or 1 hour, and "a 1-hour cache entry must appear before any
  5-minute cache entries".
- The minimum cacheable length is 512 tokens on current models (Fable 5.x, Opus 5.x, Sonnet 5.5,
  Haiku 5.5) and up to 4,096 on older ones. The guide's "1,024–4,096" (:922) is outdated.

What `transform_anthropic_messages` does (`wire.rs:93-131`), and the consequence:

1. **Undocumented placement.** It writes `cache_control` onto the *message object*
   (`msg["cache_control"]`, `wire.rs:127`), not onto a content block. The breakpoint is therefore
   ignored or rejected *(inferred; not exercised live)*.
2. **Client breakpoints removed.** It strips every client-supplied `cache_control` in the older
   history (`wire.rs:113`), discarding Claude Code's own breakpoints.
3. **Rewritten prefix.** Messages leaving the protected tail are rewritten into AHTL stubs on the
   next turn. The byte prefix the previous turn cached no longer exists, so that write is never
   read and each turn pays a new cache write on the rewritten span (1.25× input price).
4. **Uncached tail.** The per-turn tail slot is appended inside the final message, after the last
   breakpoint, so it is never reusable.
5. **TTL ordering.** A 5-minute breakpoint inserted before a client's 1-hour breakpoint violates
   the ordering rule and can cause a 400 *(inferred)*.
6. **Fragile hits.** Any hits depend on byte-identical recompression across turns, which no test
   covers.

There is also no attempt to stay within the client's existing layout. README :258 ("system
prompt plus up to 3 epoch boundaries") is contradicted by the code, which places exactly one
breakpoint.

- **Remediation:**
  - Virtualize tool output *before* it enters the transcript, via the hook path, so history
    never changes after the fact.
  - Preserve client breakpoints, and put dynamic content only after the last breakpoint.
  - Measure `cache_read_input_tokens` / `cache_creation_input_tokens` from real responses and
    publish those numbers instead of the simulated ratio (ICX-AUD-002).

## OpenAI caching and model references (Low)

- **Caching:** caching is automatic at ≥ 1,024 tokens for current models, but the minimum varies
  by model. "128-token increments" and "1024-token blocks" are wrong: 128 is the reporting
  granularity, and some models cache at 2,048-token intervals. `prompt_cache_key` affects routing
  only, and icx does not set it. Newer models add explicit breakpoints and retention controls
  that the docs do not mention.
- **Model references:** the guide cites "Claude Opus 4.6 / Sonnet 4.6, 200K–1M" and "GPT-5 /
  Codex / o3 / o4-mini" (:922-923). The current Claude lineup is Fable 5.2, Opus 5.5, Sonnet 5.5
  and Haiku 5.5, all with 1M context. Avoid hard-coding model tables in docs.

## Integration test recommendation

Each adapter needs a **contract fixture suite**: real hook payloads captured from each agent
version, with the response validated against the agent's published schema. That suite should run
in CI on every bump of a pinned agent version. Without it, the High-severity mismatches above
reached a v1.0.0 release with 100% line coverage on the adapter files.
