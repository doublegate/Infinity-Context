# Agent Adapters, Wire Proxy & Interception Specification

**References:** [ref-docs/agent-internals/rea-binary-and-sqlite-forensics.md](../ref-docs/agent-internals/rea-binary-and-sqlite-forensics.md), [ref-docs/agent-internals/cross-agent-hook-and-wire-matrix.md](../ref-docs/agent-internals/cross-agent-hook-and-wire-matrix.md), [docs/deployment-and-agent-integration-guide.md](deployment-and-agent-integration-guide.md)  
**Target Crates:** `crates/icx-adapters`, `crates/icx-proxy`  
**Release:** `v1.2.0` candidate

---

## 1. Purpose

`crates/icx-adapters` and `crates/icx-proxy` connect the **Infinity-Context (`icx`)** Virtual Context MMU to agent CLIs. They implement five native hook adapters (**Claude Code**, **OpenAI Codex CLI**, **Google Antigravity CLI (`agy`)**, **Hermes Agent**, **Meta Muse Code**), the single-tool MCP pager (`McpPagerServer`), and an **HTTP/1.1 & SSE Cache-Epoch reverse proxy** (`WireProxyTransformer`).

Hook contracts below were checked against upstream documentation on 2026-11-18 (Claude Code 2.1.328, codex-cli 0.170.0, agy 1.5.11, Hermes source `dc394c6`). `crates/icx-adapters/tests/claude_contract.rs` pins the Claude Code output shapes; the other adapters are covered by unit tests, not by fixtures captured from live agent runs.

The v1.2.0 Codex correction is also pinned by `protocol_validation.rs` and the local
Codex 0.171 diagnostic: tool-output replacement is unsupported. The tests exercise adapter
output and storage; they are not live paid-provider or complete host-integration acceptance.
Current compatibility and evidence limits supersede older design promises in
[ADR 0006](adr/0006-v1-2-0-context-and-compatibility.md).

Common behaviour of `icx hook --agent <agent> --event <EVENT>`:

- The workspace is resolved as `--dir` > `ICX_DIR` > the per-project default under `$XDG_STATE_HOME/icx/workspaces/`. A repository-local `.icx/` is never loaded implicitly.
- If an `icx daemon` serves the workspace, the hook is forwarded over its private Unix socket; otherwise it runs in-process under the workspace lock. `ICX_NO_DAEMON=1` disables the daemon path.
- stdin is bounded. The process always exits 0. On malformed input or internal errors it prints `{}` (or a non-replacing response), so the agent keeps its original behaviour.
- Outputs of the `icx` pager tool itself (`InfinityContextMmu::is_icx_pager_tool`) are never re-stubbed.
- Content is redacted (`SecretRedactor::redact`) before it reaches the store, index, or bus. Model-facing text is additionally Tier-0 delimiter-escaped.
- The daemon commits with SQLite WAL `synchronous=FULL` under the workspace lock before
  sending success. Save failure yields a failed response and the daemon reloads from disk
  before the next request (exiting after 5 consecutive failures); the CLI may then fall back to
  in-process handling, which is the only ingestion because a failed request is never saved.
  One request per connection: bytes after the request line are rejected as a protocol error. There is no acknowledged write-behind window.
- Tail producers select fitting whole frames, prioritizing the header and best recall, then
  sibling and remaining recall/PPR frames. Oversized recoverable frames can use compact CAS
  handles. Unsent sibling frames remain queued; their cursor is not advanced past them.
  The default tail cap is 1,024 conservative accounting units, also limited by remaining managed
  window space. `icx context-budget` controls the persisted model policy. It does not cap all
  external native-agent messages or an entire provider request; callers reserve that content.

---

## 2. Claude Code Adapter (`ClaudeCodeAdapter`)

Upstream reference: <https://code.claude.com/docs/en/hooks>.

### 2.1 Hook Event Handling (`icx hook --agent claude-code --event <event>`)

1. **`SessionStart`:**
   - Returns the AHTL legend and the initial tail slot in `hookSpecificOutput.additionalContext`:

     ```json
     {
       "hookSpecificOutput": {
         "hookEventName": "SessionStart",
         "additionalContext": "⟪ICX:LEGEND│Δ=code_mut│Ψ=tool_stub│Ω=invariant│Θ=xagent│Φ=folded_branch│↦=deref_via_icx_tool⟫\n⟪ICX:TAIL_SLOT│agent=claude-code│...⟫"
       }
     }
     ```

2. **`UserPromptSubmit`:**
   - Reads `prompt`, builds the tail slot (HDC query + fractal suffix + PPR walk + unseen sibling bus frames), and returns it in `hookSpecificOutput.additionalContext`.
3. **`PostToolUse` (in-flight tool output virtualization):**
   - Reads `tool_name`, `tool_input`, and `tool_response`. If the output is at or above `tool_virtualize_threshold` (default 512 bytes):
     1. Redacts secrets via `SecretRedactor`.
     2. Chunks and commits the bytes into `CasStore`, obtaining `#icx:<16-hex>`.
     3. Runs `StructuralSkeletonizer`, the Titans-inspired `TitansSurpriseGate` heuristic, and `AhtlEncoder` to produce a compact stub.
     4. Publishes a summary frame on the bus.
     5. Returns `updatedToolOutput` **in the tool's own output shape**. Claude Code ignores a replacement whose shape does not match the tool's output. For Bash the object `{stdout, stderr, interrupted, isImage}` is returned with the stub in `stdout`; for other object outputs all fields are kept and the largest string leaf is replaced. MCP tools (`mcp__*`) additionally get `updatedMCPToolOutput` (string):

        ```json
        {
          "hookSpecificOutput": {
            "hookEventName": "PostToolUse",
            "updatedToolOutput": {
              "stdout": "⟪ICX:STUB│#icx:7a2e91f0ab12cd34│raw=64120B→298B│lines=1..1420⟫\n⟦Ψ:Bash:cargo_test#L1-1420⊢✓pass:84⋮bytes:64120↦#icx:7a2e91f0ab12cd34⋮Ψ:⠞⠧⡁⢂⟧",
              "stderr": "",
              "interrupted": false,
              "isImage": false
            }
          }
        }
        ```

   - Below the threshold the response is `{}`.
4. **`PreToolUse`:** returns only `{"hookSpecificOutput":{"hookEventName":"PreToolUse"}}`. icx makes no permission decision and does not modify the tool input; registering this hook is unnecessary.
5. **`SubagentStart` / `SubagentStop`:**
   - `SubagentStart` opens an isolated subagent branch and returns a tail slot in `additionalContext`.
   - `SubagentStop` reads `last_assistant_message`, folds the branch into an `L2` node, and returns `{}`. Any decision or `additionalContext` here would keep the subagent running.
6. **`PreCompact` & `PostCompact`:**
   - `PreCompact` seals the active window into an `L3` `CacheEpochNode` and prints **plain stdout** (not JSON), which Claude Code uses as compaction instructions: keep every `#icx:` handle and icx packet verbatim, followed by the current context map. Output stays under Claude Code's 10,000-character limit. (`systemMessage` is discarded on this event.)
   - `PostCompact` stores `compact_summary` in the workspace, seals the epoch, and returns `{}`.
   - icx never blocks compaction.
7. **`SessionEnd`:** folds open subagent branches, seals the epoch, returns `{}`.
8. **`Stop`:** not handled; returns `{}`.

---

## 3. OpenAI Codex CLI Adapter (`CodexAdapter`)

Upstream reference: the Codex hooks documentation (developers.openai.com/codex/hooks). Hooks are configured in `~/.codex/hooks.json` with PascalCase event names, a string `command`, and `timeout` in seconds; hooks are enabled by default (`codex_hooks` is a deprecated feature alias).

### 3.1 Hook & Wire Integration

1. **`SessionStart`, `UserPromptSubmit` & `SubagentStart`:**
   - Return `{"continue": true, "hookSpecificOutput": {"hookEventName": ..., "additionalContext": ...}, ...}` with the legend and/or tail slot.
   - Each `additionalContext` is kept within 10,000 UTF-8 bytes (`CODEX_INLINE_CONTEXT_MAX_BYTES`, v1.4.0). That is Codex's default per-handler `additionalContextLimit` of 2,500 tokens, which Codex estimates as bytes / 4. Above the limit, Codex writes the context to a file and gives the model only a preview. The tail slot is already far smaller; a larger value would be replaced whole by a diagnostic, so no `#icx:` handle is cut.
2. **`PostToolUse`:**
   - Ingests the tool output (store, index, bus) in every case.
   - Returns a non-replacing response (`continue:true` and `hookSpecificOutput.hookEventName`
     for ingested outputs), with neither `updatedMCPToolOutput` nor `additionalContext`.
     `CodexAdapter::SUPPORTS_TOOL_OUTPUT_REPLACEMENT` is `false`. v1.1.0 emitted the unsupported
     MCP replacement field; v1.2.0 removes it rather than claiming replacement succeeded.
   - **Net effect:** through hooks alone, icx does not shrink tool output in Codex; the model still sees the full native output. Use the wire proxy (§6) to reduce what Codex resends.
   - **Live-verified on Codex 0.171.0 (2026-11-22, T-62-001):** neither documented reply substitutes a result. Every model available to this install (`tool_mode: code_mode_only`) runs tools inside a Code Mode script, and Code Mode hands the script the original result.
     - `continue:false` plus a reason changes nothing the model sees, for shell and MCP tools alike.
     - `decision:"block"` reaches the model as a failed tool call carrying the reason.
     - icx therefore keeps the non-replacing reply.
3. **`PreToolUse`:** returns `continue: true` with no decision and no input change.
4. **`SubagentStop`:** reads `last_assistant_message` and folds the subagent branch; the fold frame is returned in `systemMessage`.
5. **`PreCompact`, `PostCompact` & `SessionEnd`:** seal the epoch and return `{"continue": true, "systemMessage": "<deterministic prefix>"}`.
6. **`PreModelRequest` / `PostModelRequest`:** not used. Upstream defines them as diagnostic only; they cannot change the request.
7. **Rollout ingestion (`icx codex-ingest`, v1.4.0; `codex_ingest` module):**
   - **What it reads:** the rollouts under `$CODEX_HOME/sessions` (default `~/.codex/sessions`) whose `session_meta.cwd` is the project or lies beneath it, as recorded or once symlinks are resolved, including a linked git worktree of the project. `--all-projects` drops the filter. Only a bounded prefix (4 MiB) of each file is decoded to find the `cwd`.
   - **Both forms:** plain `rollout-*.jsonl` and the zstd-compressed `rollout-*.jsonl.zst` files that Codex writes for rollouts older than 7 days. When both forms of one rollout exist, only the plain file, which Codex appends to, is read.
   - **Compressed files** are decoded by a pure-Rust decoder that verifies frame checksums. They are measured before anything is ingested and refused above 1 GiB decompressed, so a corrupt frame or a decompression bomb ingests nothing.
   - **Bookmarks:** each rollout's read position is saved in `codex-rollouts.json` in the workspace, keyed by its path without `.zst` and checked against a hash of its first 64 KiB. Ingestion resumes where it stopped when Codex recompresses a file or decompresses it again on append, and starts from the beginning for a replaced file. An incomplete trailing line is left for the next run.
   - **No double indexing:** a rollout record whose redacted output the store already holds is skipped and counted as "already stored". Usually that is the copy icx's Codex hook ingested when the tool ran. Measured on this project's live workspace: 1,403 of 1,583 rollout tool outputs were already stored. Messages and turn context, which hooks never see, are ingested as new. Copies that differ in formatting are not recognised.
   - **Safety:** symlinks and special files are refused.
   - **Locking:** rollouts are selected before the workspace lock is taken. Ingestion then runs in batches of about 1.5 s: each batch takes the lock, reloads the store, ingests, saves records and bookmarks, and releases it. Live hooks and the daemon are never locked out for the whole scan.
   - **Not automatic:** run `icx codex-ingest` on demand or on a schedule; hooks do not call it.
   - `CodexRolloutTailer` remains the in-process tailer for plain files.

---

## 4. Google Antigravity CLI (`agy`) Adapter (`AntigravityAdapter`)

Upstream reference: <https://antigravity.google/docs/hooks>. Hooks are configured in a workspace `.agents/hooks.json` or the global `~/.gemini/config/hooks.json` as named groups of events; each handler has a single `command` string, optional `type` (default `"command"`), `timeout` in seconds (default 30), and `enabled`. The documented events are `SessionStart`, `PreToolUse`, `PostToolUse`, `PreInvocation`, `PostInvocation`, and `Stop`. (Gemini CLI event names such as `BeforeModel`, `AfterTool`, or `PreCompress` do not apply to agy.)

### 4.1 camelCase Protojson Hook Contract

1. **`SessionStart`, `PreInvocation` & `PostInvocation`:**
   - Read `conversationId`, `transcriptPath`, `lastUserInput` (or `prompt` / `query`), and related fields.
   - Build the tail slot (HDC/PPR/fractal recall + unseen bus frames) and return it as an injected system message with `"terminationBehavior": ""`:

     ```json
     {
       "injectSteps": [
         {
           "systemMessage": {
             "systemMessage": "⟪ICX:TAIL_SLOT│conv=983dcb21⟫\n⟦Θ:codex-desktop@crates/icx-core/src/dag.rs⊢⊕fn:seal_epoch↦#icx:19fa0042b7c3d5e6⟧",
             "metadata": {
               "user_markdown": ""
             }
           }
         }
       ],
       "terminationBehavior": ""
     }
     ```

   - The `metadata.user_markdown` key is intended to keep the frame out of the chat UI; it is not documented upstream, so that effect is unverified.
   - `PostInvocation` never sets `force_continue` (v1.0.x did when the input carried `forceContinue`).
2. **`PreToolUse`:**
   - agy requires a `decision`. icx always returns `{"decision": "allow"}`, expressing no opinion, and never modifies the tool call. v1.0.x echoed `decision` / `overwrite` from its input; that was removed. Registering this hook is unnecessary.
3. **`PostToolUse`:**
   - Ingests `toolCall.args` and the tool response. When the payload omits the response, reads the matching step from the last `512 KiB` of `transcriptPath`. The transcript is opened non-blocking without following symlinks, and only regular files are read. Returns `{}`.
4. **`Stop`:**
   - Always returns `{"decision": "stop"}`; icx never keeps the agent running. It folds open subagents and seals the active window into an epoch (forced on `max_steps_exceeded` or `error`), so the next session resumes from that state:

     ```json
     {
       "decision": "stop"
     }
     ```

   - v1.0.x returned `"continue"` on `MAX_TOKEN_BUDGET_EXCEEDED` / `MAX_INVOCATIONS`, overriding the user's budget; that was removed.
5. **`SubagentStart`, `SubagentStop` & `SessionEnd`:** handled by the adapter (branch isolation and folding), but not part of agy's published event list.

---

## 5. Hermes Agent Adapter (`HermesAdapter`)

Target: the Hermes `ContextEngine` ABC (`agent/context_engine.py`) and `MemoryProvider.on_pre_compress` (`agent/memory_provider.py`).

- `HermesAdapter` implements the context-engine logic in Rust: `select_context`, `prune_tool_results_only`, `compress`, `on_turn_complete`, `on_pre_compress` (checkpoint API version 2), and `SessionStart`, all reachable through `icx hook --agent hermes --event <name>` with a JSON payload on stdin. The JSON contract is listed in the [deployment guide §2.5](deployment-and-agent-integration-guide.md#25-hermes-agent-configuration-contextengine-plugin).
- **No Python plugin or shim ships in this repository.** To use icx with Hermes, a user must write a `ContextEngine` plugin that calls `icx hook --agent hermes ...`. The plugin must implement the abstract `name` property, `update_from_response`, `should_compress`, and `compress`; `select_context`, `on_turn_complete`, and `prune_tool_results_only` are optional. Hermes loads context-engine plugins from `plugins/context_engine/<name>/` or `$HERMES_HOME/plugins/<name>/`, selected by `context.engine` in `~/.hermes/config.yaml`.
- Hermes has no `hooks.yaml`. Its shell hooks (under `hooks:` in `config.yaml`, snake_case events such as `pre_tool_call` and `on_session_start`) and gateway hooks (`~/.hermes/hooks/<name>/HOOK.yaml`) are not handled by icx.
- `select_context` replaces middle-history messages with one `icx_cache_epoch` system message plus an `icx_tail_slot` message, without touching Hermes's `state.db` `messages` table. Preserved verbatim: the protected head and tail, `icx` pager results, messages with structured non-text content, and complete `tool_calls` groups.

---

## 6. Meta Muse Code Adapter (`MuseAdapter`, v1.3.0)

`icx hook --agent muse --event <event>`. Contract observed on Muse Code 1.4.25 with probe hooks
(a real headless session; every event below fired and its payload was recorded):

- **Configuration.** Command hooks live in `~/.config/muse/settings.json` (`${XDG_CONFIG_HOME}`
  honoured) in Claude Code's shape: `"hooks": {"<Event>": [{"matcher": "", "hooks": [{"type":
  "command", "command": "...", "timeout": <seconds>}]}]}`. The file must keep
  `"schema_version": 1`. Muse rejects an unknown event name (`unsupported hook event`), and
  `Interrupt` and `ToolUseStart` admit only `async: true` hooks. MCP servers are added with
  `muse mcp add <name> --user -- <command> [args]`.
- **Invocation.** Hooks run in the project directory with the caller's environment (plus
  `MUSE_SESSION_ID` and `MUSE_THREAD_ID`), so icx resolves the project workspace from the working
  directory, as for Claude Code.
- **Payload.** Claude-Code-shaped JSON on stdin: `hook_event_name`, `session_id`, `cwd`,
  `tool_name`, `tool_input`, `tool_response`, `tool_use_id`, plus `turn_id`, `model`,
  `model_provider` and `permission_mode`; `transcript_path` is null. The Bash tool's
  `tool_response` is a JSON-encoded string (`output`, `exit_code`, `terminal_status`,
  `truncated`); icx stores its `output` text and uses its `exit_code`. Subagent events carry
  `subagent_id` and `child_session_id`; `Stop`/`SubagentStop` carry `last_assistant_message`.
- **Replies.** `hookSpecificOutput.additionalContext` from SessionStart, UserPromptSubmit and
  PostToolUse reaches the model; `hookSpecificOutput` must name its `hookEventName`. A reply's
  `systemMessage` is shown to the user, not the model.

| Event | icx action | Reply |
| --- | --- | --- |
| `SessionStart` | Tail slot with the AHTL legend | `additionalContext` |
| `UserPromptSubmit` | Prompt-scoped tail slot (recall) | `additionalContext` |
| `PostToolUse` | Ingest the tool output (Bash JSON unwrapped) | `{"hookSpecificOutput":{"hookEventName":"PostToolUse"}}` |
| `PostToolUseFailure` | Ingest the error text as an anomaly | `{}` |
| `SubagentStart` / `SubagentStop` | Open / fold the subagent branch | `{}` |
| `PreCompact` / `PostCompact` | Seal the epoch; PostCompact also stores a `compact_summary` when present | `{}` |
| `SessionEnd` | Fold open subagents, seal the epoch | `{}` |
| any other event | none | `{}` |

**No output replacement.** Muse refuses it ("PostToolUse output cannot replace tool output"),
so `MuseAdapter::SUPPORTS_TOOL_OUTPUT_REPLACEMENT` is `false`. Under Muse icx stores every tool
output and feeds recent activity and recall back into the conversation, like Codex; it does not
shrink tool output in Muse's own context. No reply carries a JSON `null`, and malformed stdin
fails open with `{}`.

**Workspace trust.** Muse loads MCP servers, the `icx` pager included, only in a trusted
workspace; hooks run either way. See the deployment guide §2.6 (#30).

**Compatibility.** `AgentKind::Muse` (value 8) is new in v1.3.0. A v1.2.x binary cannot read a
workspace in which a Muse event was recorded, so upgrade every agent's `icx` before wiring Muse.

## 7. Wire-Level Reverse Proxy (`crates/icx-proxy`)

`icx proxy` is an HTTP/1.1 listener (upstream requests are sent over HTTPS through `curl`).

- **Bind:** `127.0.0.1:49380` by default (or `ICX_PROXY_BIND`). Non-loopback binds are refused unless `--allow-remote` is passed.
- **Authentication:** every request must carry the per-install token, as the path prefix `/t/<token>` or the header `x-icx-token: <token>`; otherwise the proxy answers `401`. The header is removed before forwarding. The token is derived from `install.key` and printed at startup (`set ANTHROPIC_BASE_URL / OPENAI_BASE_URL to http://127.0.0.1:49380/t/<token>`). `--insecure-no-auth` disables the check and is not recommended.
- **Routing:** after the token prefix is removed, the provider is selected by an optional route prefix (`/anthropic`, `/openai`, `/gemini`, stripped before forwarding), by the API path (`/v1/messages`, `/v1/responses`, `/v1/chat/completions`, `/v1beta/models/*`), or by `--provider`. Typical base URLs:
  - `ANTHROPIC_BASE_URL=http://127.0.0.1:49380/t/<token>`
  - `OPENAI_BASE_URL=http://127.0.0.1:49380/t/<token>/v1`
- **Upstream:** `--upstream` / `ICX_UPSTREAM_URL`, else the provider default (`https://api.anthropic.com`, `https://api.openai.com`, `https://generativelanguage.googleapis.com`).

### Proxy Transformations

1. **Outbound Request Pass:**
   - Tier-0-escapes forged icx markers (`⟦ ⟧ ⟪ ⟫`) anywhere in any non-system message, including the latest one, unless the block is byte-identical to one icx emitted. System/developer messages and `icx` pager results are left as-is.
   - Replaces oversized middle-history content outside the last 2 messages with deterministic AHTL packets, exempting `icx` pager outputs. Folded messages are normalized to content-block form, so folded history stays byte-stable across turns.
   - **Anthropic:** keeps every client `cache_control` breakpoint; adds at most one icx breakpoint, on the last content block of the folded region, only while the request has fewer than 4 breakpoints; uses a 1-hour TTL when a later client breakpoint uses one.
   - Appends the Ephemeral Tail Slot (HDC/PPR/fractal recall and unseen bus frames) to the final message, replacing any earlier tail slot.
2. **Inbound Response Pass:**
   - Expands inline dereference directives (`⟪!icx:deref(#icx:<16-hex>)⟫` or `⟪!icx:deref(#icx:<16-hex>,start:end)⟫`) in SSE and JSON responses. Expanded content is escaped before it is returned.
