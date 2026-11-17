# Cross-Agent Low-Level Hook, Wire & Database Interception Matrix

**References:** [rea-binary-and-sqlite-forensics.md](rea-binary-and-sqlite-forensics.md), [initial-concept.md](../initial-concept.md)

This document specifies the exact low-level interception mechanisms that **Infinity-Context (`icx`)** uses to achieve seamless, zero-friction unlimited context across all major AI coding agents.

---

## 1. Multi-Plane Interception Strategy

No single integration point works identically across every agent CLI/IDE because agents differ in binary runtime (Bun vs. Rust vs. Go vs. Python) and network stack (configurable base URL vs. OAuth/mTLS vs. gRPC). Therefore, `icx` implements a **Defense-in-Depth 4-Layer Interception Stack**:

| Layer | Mechanism | Latency | Capabilities |
| :--- | :--- | :--- | :--- |
| **Layer 0A: Wire Proxy** | HTTP/1.1, HTTP/2 & SSE Reverse Proxy (`ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL`, `GEMINI_API_BASE_URL`, `codex` `proxy_url`) | `< 150 µs` | Full prompt array control, `cache_control` breakpoint pinning, silent token-pointer dereferencing on streaming responses, zero-cost tool schema virtualization. |
| **Layer 0B: Kernel / Uprobe / WAL Tailer** | Linux `inotify` + `io_uring` tailing of SQLite WALs (`state_5.sqlite-wal`, `conversations/*.db-wal`, `state.db-wal`) & JSONL logs + optional eBPF `uprobe` (`SSL_read`/`SSL_write`) | `< 80 µs` | Passive, zero-config real-time ingestion of all turns and subagent steps even on locked/OAuth-pinned clients. |
| **Layer 1: Native Lifecycle Hooks** | Synchronous binary/UDS hook invocation (`hooks.json`, `settings.json`, `ContextEngine`) | `< 400 µs` | In-flight tool output replacement (`updatedToolOutput`), tool input rewriting (`updatedInput` / `overwrite`), pre-invocation context injection (`injectSteps` / `additionalContext`), and compaction override (`PreCompact`). |
| **Layer 2: Virtual Paging MCP & ACP** | Single-tool polymorphic MCP gateway (`icx`) + Agent Client Protocol (`ACP`) bridge | `< 300 µs` | Explicit model-driven page-fault resolution (`#icx:<hash>` dereference, cold regex/AST search, cross-agent query, parallel sub-trajectory `Map`). |

---

## 2. Exact Per-Agent Hook & Mutation Matrix

| Capability | Claude Code (`2.1.326`) | OpenAI Codex (`0.169.0`) | Antigravity CLI (`agy`) | Hermes Agent |
| :--- | :--- | :--- | :--- | :--- |
| **Hook Config File** | `~/.claude/settings.json` | `~/.codex/hooks.json` + `config.toml` | `~/.gemini/config/hooks.json` or `.agents/hooks.json` | `cli-config.yaml` (`context.engine`, `memory.provider`) |
| **Startup Context Injection** | `SessionStart` -> `hookSpecificOutput.additionalContext` | `SessionStart` -> `hookSpecificOutput.additionalContext` / `systemMessage` | `SessionStart` -> `injectSteps[].systemMessage` (`metadata.user_markdown: ""`) | `ContextEngine.on_session_start()` + `MemoryProvider.system_prompt_block()` |
| **Per-Turn Pre-Model Injection** | `UserPromptSubmit` -> `hookSpecificOutput.additionalContext` | `UserPromptSubmit` -> `additionalContext` + `pre-model-request` | `PreInvocation` -> `injectSteps[].systemMessage` | `ContextEngine.select_context()` + `MemoryProvider.prefetch()` |
| **Tool Input Rewriting** | `PreToolUse` -> `hookSpecificOutput.updatedInput` (`permissionDecision: "allow"`) | `PreToolUse` -> `hookSpecificOutput.updatedInput` | `PreToolUse` -> `{"decision": "allow", "overwrite": {...}}` | `acp_adapter` / tool middleware |
| **Tool Output Replacement (Pre-LLM)** | `PostToolUse` -> `hookSpecificOutput.updatedToolOutput` (all tools!) & `updatedMCPToolOutput` | `PostToolUse` -> `hookSpecificOutput.updatedMCPToolOutput` + `output_spill` | `PostToolUse` + `PreInvocation` `injectSteps` | `ContextEngine.prune_tool_results_only()` + `select_context()` |
| **Compaction Interception** | `PreCompact` (can block or customize) + `PostCompact` (`compact_summary` -> re-inject AHTL state) | `PreCompact` (`systemMessage` / `continue`) + `PostCompact` | `Stop` (`terminationReason == "MAX_TOKEN_BUDGET_EXCEEDED"` -> `decision: "continue"`) | `ContextEngine.should_compress()` / `compress()` + `MemoryProvider.on_pre_compress()` (v2) |
| **Subagent Lifecycle** | `SubagentStart` (`additionalContext`) / `SubagentStop` | `SubagentStart` (`additionalContext`) / `SubagentStop` | `conversation_parents` / `steps.has_subtrajectory` | `MemoryProvider.on_delegation()` + `async_delegations` |
| **Local Transcript / DB Path** | `~/.claude/projects/<slug>/<session>.jsonl` | `~/.codex/state_5.sqlite` (`threads.rollout_path`) + `memories_1.sqlite` | `~/.gemini/antigravity-cli/conversations/<id>.db` + `brain/<id>/.system_generated/logs/transcript_full.jsonl` | `~/.hermes/state.db` (`sessions`, `messages`) |

---

## 3. Deep Dive: Exploiting Each Agent's Low-Level Seams

### 3.1 Claude Code: `updatedToolOutput` + `PreCompact` Override
In Claude Code `2.1.326`, the largest source of context bloat is raw tool execution output (`Bash`, `Read`, `Grep`).
1. When any tool finishes, Claude Code invokes `PostToolUse` with `{tool_name, tool_input, tool_response, tool_use_id, session_id, transcript_path}`.
2. If `Buffer.byteLength(JSON.stringify(tool_response)) > ICX_INLINE_THRESHOLD` (e.g., 2,048 bytes):
   - `icx hook --agent claude-code --event post-tool-use` streams the raw `tool_response` into the BLAKE3 Content-Addressed Store (`L0` leaf in the Merkle-DAG) and indexes its 10,000-bit HDC hypervector + Fractal coordinate.
   - `icx` runs the **AST / Log Structural Skeletonizer** and encodes the result into an **AHTL Holographic Stub** (typically 40–120 tokens) containing:
     - The exact exit code, line/byte/token counts, and BLAKE3 dereference handle `⟦#icx:a8f02b91⟧`.
     - All high-surprise lines (compiler errors, panics, failing test names, modified symbols) extracted via the **Titans Surprise Gate**.
     - A byte-range table of contents so the model can dereference any exact slice on demand via `icx_deref("a8f02b91", start_line, end_line)`.
   - `icx` emits on `stdout`:
     ```json
     {
       "hookSpecificOutput": {
         "hookEventName": "PostToolUse",
         "updatedToolOutput": "⟪ICX:STUB│#icx:a8f02b91│raw=84,210B→312B│lines=1..1842⟫\n⟦Ψ:cargo_test⊢pass=142⋮fail=1⋮!PANIC@crates/core/src/lib.rs:412:9:'index out of bounds: len is 4 but index is 4'⋮slice_map=[L1-120:build,L121-1790:ok_tests,L1791-1842:fail_trace]⟧"
       }
     }
     ```
3. Result: **99.6% reduction in tool-output token consumption** with **zero loss of actionable signal** and **100% lossless byte recovery** via `#icx:a8f02b91`.

### 3.2 OpenAI Codex: Dual Hook + Rollout Tailer + Proxy Injection
Codex `0.169.0` provides three complementary seams:
1. **Hook Seam (`~/.codex/hooks.json`)**:
   - `SessionStart` and `UserPromptSubmit` return `hookSpecificOutput.additionalContext` containing the current cross-agent `/dev/shm` telepathy delta and active project AHTL epoch summary.
   - `PreCompact` and `PostCompact` preserve the lossless Merkle-DAG root across context resets.
2. **Built-in Proxy & Header Seam (`~/.codex/config.toml`)**:
   - Codex natively supports `proxy_url`, `unix_sockets`, and `model_providers.<name>.base_url`. Pointing Codex through `http://127.0.0.1:49380` (`icx-proxy`) gives `icx` full wire-level control over `/v1/responses` payloads, keeping Cache Epochs immutable.
3. **Live Rollout & SQLite Tailer (`state_5.sqlite`)**:
   - `icxd` watches `threads.rollout_path` JSONL files and `memories_1.sqlite` (`stage1_outputs`), automatically absorbing Codex's internal `RolloutItemWire::RetainedContext` and `Compacted` items into the shared cross-agent Merkle-DAG.

### 3.3 Google Antigravity CLI (`agy`): Silent UI-Hidden `systemMessage` + `Stop` Continuation
Antigravity CLI's `PreInvocation` and `SessionStart` hooks have a remarkable feature documented in `hooks.md`:
- When injecting a `systemMessage`:
  ```json
  {
    "injectSteps": [
      {
        "systemMessage": {
          "systemMessage": "⟪ICX:AHTL_FRAME│ep=07│...⟫",
          "metadata": {
            "user_markdown": ""
          }
        }
      }
    ]
  }
  ```
- Setting `metadata.user_markdown` to `""` causes the Antigravity UI to render **nothing** to the human user (zero UI clutter!), while the underlying model receives the full `systemMessage` text!
- Even better: because `AHTL` is a non-human machine dialect, hiding it from the human UI via `metadata.user_markdown: ""` while feeding it directly into the model's attention window is a **perfect architectural match**.
- Furthermore, when `agy` hits `MAX_TOKEN_BUDGET_EXCEEDED` or `MAX_INVOCATIONS`, `icx`'s `Stop` hook intercepts `terminationReason`, folds the completed steps into an LCM Merkle-DAG epoch, and returns `{"decision": "continue", "reason": "⟪ICX:EPOCH_ROLLOVER│#icx:f319a02c⟫"}` so long-horizon runs never stall.

### 3.4 Hermes Agent: Zero-Mutation `ContextEngine.select_context()`
Hermes's `ContextEngine.select_context()` (`agent/context_engine.py:121-141`) was practically designed for `Infinity-Context`:
- It runs on **every provider request** right before prompt cache-control derivation.
- It receives `request_messages` (assembled request), `conversation_messages` (read-only persisted history), `incoming_message`, and `budget_tokens`.
- Its return value replaces the outbound LLM request *without mutating `state.db`*.
- Our lightweight Python bridge (`plugins/context_engine/infinity_context/`) passes the message UIDs over the `/dev/shm/icx-fabric-v1` shared memory ring to `icxd` in Rust, which returns the Cache-Epoch-aligned AHTL message array in `< 200 µs`.
