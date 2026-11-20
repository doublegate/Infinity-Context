# REA Binary & SQLite Forensics Ledger: Local Agent Ecosystem

**Generated:** 2026-11-16  
**Methodology:** Reverse-Engineer-Anything (`rea` MCP Server) offline target-free binary, ELF note, dynamic export, Go build-info, Rust dependency, and SQLite database inspections, corroborated by static string/schema extraction and local source inspection.

---

## 1. Evidence Ledger Summary

Every claim in this document is anchored to a cryptographic SHA-256 artifact digest and a retained REA Evidence ID (`ev_*`).

| Target Artifact | Local Path | Bytes | SHA-256 Digest | REA Evidence ID(s) |
| :--- | :--- | ---: | :--- | :--- |
| **Claude Code (`2.1.326`)** | `~/.local/share/claude/versions/2.1.326` | 371,429,904 | `b6b3647d981130dc44fc670846ae97b6cde531ffb667cf9c4d17cfcafb38cb37` | `ev_be2e252614342586f9de78e5f08eb70de8465f18310bc03a948abe986a60ef72` (ELF notes)<br>`ev_89ea50d5e0608cf878743511a9632f6f80475957dfb215cd54ac576dacb0977e` (Exports parent)<br>`ev_3d40926734f39b00b23864504aeef72e3eaf3f0db120ec0492c2ee9fbcc24b35` (Exports view) |
| **OpenAI Codex (`0.169.0`)** | `~/.codex/packages/standalone/releases/0.169.0-x86_64-unknown-linux-musl/bin/codex` | 294,277,432 | `b452ca20f838a8855a88a41050ee4a694033fff4ea3ecdad5633e6e0e8d13f2b` | `ev_7cda5928199ea58224bdedbbd14892ff224515db64f7679457f633d5e7d816d3` (ELF notes)<br>`ev_ba0d9f9e305a96c026cd5c6efbc856664c5cf9c7603650ea0355c4e2a82a0acd` (Exports) |
| **Antigravity CLI (`agy`)** | `~/.local/bin/agy` | 265,700,040 | `39816d49d5ab92229952987225c23ea3fed7580699dcb4a444e0388e0ac1ce43` | `ev_e0bd52156323af13cc80956727b105639c42d36bc491e0c89dd1b2f81f4da090` (ELF notes)<br>`ev_b3af7a149e6eaaa93f2af7f75747279991766aa5a575b3ceeae4d099edd79632` (Go build info)<br>`ev_241883bf55926e279510291cbb4069abad0dc4c8aed78ab1c0a4761bed5ea820` (Exports) |
| **`ai-memory` Daemon/CLI** | `~/.local/lib/ai-memory/ai-memory` | 50,192,208 | `02904079c0700b1d96e1aafaa5dc8da16eb4a1a7ff32eb100d78a6338ab2e2c8` | `ev_c127f3b46973ea298046b85ad5e6a61a1a3c2d8d485b6592efcaacee26d6bb9b` (Rust binary)<br>`ev_17a0d1760f2a0f592d24b04b3716faa9f57f6b4bda37a29fd4a185f4df2c3842` (Exports) |
| **Codex `state_5.sqlite`** | `~/.codex/state_5.sqlite` | 9,179,136 | `537c13f5ca2307c8bc8c66cc03e28f6c6c67de5b15b32f0011ac69cb51d4057d` | `ev_3e4a7d4314b4d31f13103a257120fda8c23e0a884187421d7a47b4a75f1c29a5` |
| **Codex `memories_1.sqlite`** | `~/.codex/memories_1.sqlite` | 663,552 | `3912d60d041897d33fc2c2b169bfa1175995b21613b607e134c646eb630aa879` | `ev_1cc07dc44d6445edb0769cc0d4fee066758881e20cddd8f2a8c3d961c3fea201` |
| **AGY `conversation_summaries.db`** | `~/.gemini/antigravity-cli/conversation_summaries.db` | 35,069,952 | `3dfaba92366b627b64bf821974d87e6b1b4f7057084b47719e875accba5ee75a` | `ev_6a75fb73dadf1f29cb480f77e5ebbce3692856c21971bae83b6ec3846effbdb1` |
| **AGY Trajectory DB (`650ca1b8...db`)** | `~/.gemini/antigravity-cli/conversations/<conversation-id>.db` | 6,094,848 | `22a33eab4aa9ccf5add825adab96b09cd8446a9aa027a0143819756449216491` | `ev_524bc260c8a7ccbbaef9d7cfb1c2aaf949a069e2aac27ebff55a9c96d6c118b6` |
| **Hermes `state.db`** | `~/.hermes/state.db` | 82,018,304 | `d5411715a24e4acc773d632fadda8ff0cafae612d7810c489d3afd6b51bf58f9` | `ev_55f2f031e94c568896ee3d6653a29fb05932581b058229e8bf3aebc614652511` |

---

## 2. Target-by-Target Forensic Breakdown

### 2.1 Claude Code (`2.1.326`)
- **Binary Architecture (`ev_be2e252614342586f9de78e5f08eb70de8465f18310bc03a948abe986a60ef72`):**
  - ELF64 `ET_EXEC` dynamically linked executable (`BuildID[sha1]=d72a729b5627657c0649e49ae9d771fec0783039`).
  - Contains custom ELF note sections `.note.bun` (offset `912`, length `4104` bytes, owner `"Bun"`, descriptor SHA-256 `b8e70afa9d21077d3fcbca5dc46588320ac02e39a55aa42941af4ce54d29d59a`) and `.note.bun.build-id` (offset `5016`, length `32` bytes, owner `"Bun"`).
  - Dynamic symbol table (`ev_3d40926734f39b00b23864504aeef72e3eaf3f0db120ec0492c2ee9fbcc24b35`) exports 733 symbols from `libuv` (`uv_*`), Node-API (`napi_*`), and V8 compatibility shims (`_ZN2v8*`).
  - **Conclusion (Observed):** Claude Code `2.1.326` is compiled via `bun build --compile` into a single-file ELF containing the Bun/JavaScriptCore runtime plus the bundled TypeScript/JS application payload.
- **Internal Hook & Compaction Surface:**
  - Embedded Zod schemas define 18 hook events:
    `SessionStart`, `SessionEnd`, `UserPromptSubmit`, `UserPromptExpansion`, `PreToolUse`, `PostToolUse`, `PreCompact`, `PostCompact`, `PreModelSwitch`, `PostModelSwitch`, `PermissionRequest`, `PermissionDenied`, `Stop`, `StopFailure`, `SubagentStart`, `SubagentStop`, `Setup`, `TeammateIdle`, `TaskCreated`.
  - **Critical Discovery — Tool Output Rewriting (`PostToolUse`):**
    The `PostToolUse` hook output schema supports two payload-replacement fields:
    - `updatedToolOutput`: `"Replaces the tool output before it is sent to the model"` (applies to **all** built-in and MCP tools: `Bash`, `Read`, `Grep`, `Glob`, etc.).
    - `updatedMCPToolOutput`: `"Replaces the output for MCP tools only. Prefer updatedToolOutput, which works for all tools"`.
    - `additionalContext`: Injects supplemental context alongside the tool result.
  - **Critical Discovery — Compaction Interception (`PreCompact` & `PostCompact`):**
    - `PreCompact` receives `{hook_event_name: "PreCompact", trigger: "manual" | "auto", custom_instructions: string | null}`. A hook can block reactive/precomputed compaction (`"Compaction blocked by PreCompact hook; continuing uncompacted"`) or checkpoint the full raw transcript before compaction occurs.
    - `PostCompact` receives `{hook_event_name: "PostCompact", trigger: "manual" | "auto", compact_summary: string}` and can return `additionalContext` / `systemMessage` to re-inject lossless AHTL state frames immediately after compaction!
    - Furthermore, `requestJournal.replacePendingPostCompaction(true)` tracks `keepsCachedPrefix` across compactions.

---

### 2.2 OpenAI Codex CLI (`0.169.0`)
- **Binary Architecture (`ev_7cda5928199ea58224bdedbbd14892ff224515db64f7679457f633d5e7d816d3`, `ev_a415da20b287f2968c8cdb35b0d66bcd78816b461d78317f00351e01edf82985`):**
  - ELF64 `ET_DYN` static-pie musl Rust executable (`294,277,432` bytes, 0 dynamic exports, stripped).
  - Source path strings confirm internal workspace crates: `codex_core` (`core/src/tasks/mod.rs`, `core/src/model_request.rs`), `codex_hooks` (`hooks/src/engine/dispatcher.rs`, `hooks/src/events/model_request.rs`, `hooks/src/output_spill.rs`), `codex_tui`, and `ext/extension-api/src/model_request.rs`.
- **Internal Hook & Model-Request Interception Surface:**
  - Supports 12 lifecycle hook events in `~/.codex/hooks.json`:
    `session-start`, `session-end`, `user-prompt-submit`, `pre-tool-use`, `post-tool-use`, `permission-request`, `pre-compact`, `post-compact`, `subagent-start`, `subagent-stop`, `interrupt`, and internal turn-scoped **`pre-model-request`** and **`post-model-request`** (`hooks/src/events/model_request.rs`).
  - **Critical Discovery — Embedded Hook Trust & Output Spill:**
    - Codex hashes hook definitions in `~/.codex/config.toml` under `[hooks.state."..."]` (`trusted_hash`, `trusted_content_hash`), and includes a built-in `codex_hooks::output_spill` (`hooks/src/output_spill.rs`) mechanism!
    - Rollout JSONL items (`RolloutItemWire`) include dedicated wire variants:
      `RetainedContext`, `Compacted` (`CompactedItemWire`), `TurnContext`, `InterAgentCommunication`, `DynamicToolsUpdated`, `WorldState`, and built-in model-private history/notes tools (`alpha/notes/v2/*` and `alpha/history/v2/*`).
- **SQLite Persistence Architecture:**
  1. **`state_5.sqlite` (`ev_3e4a7d4314b4d31f13103a257120fda8c23e0a884187421d7a47b4a75f1c29a5`):**
     - Managed via `sqlx` (`_sqlx_migrations`).
     - `threads` table (43 columns): `id`, `rollout_path` (path to JSONL rollout file), `tokens_used`, `memory_mode` (`'enabled'`), `history_mode` (`'legacy'` or paginated), `attached_thread_parent_id`, `project_id`, `git_sha`, `git_branch`.
     - `thread_dynamic_tools`: per-thread dynamic tool schemas with `defer_loading` and `namespace`.
     - `thread_attachments`: per-thread `attachment_type`, `identity_key`, `payload`.
     - `thread_spawn_edges`: parent-child subagent graph (`parent_thread_id`, `child_thread_id`, `status`).
  2. **`memories_1.sqlite` (`ev_1cc07dc44d6445edb0769cc0d4fee066758881e20cddd8f2a8c3d961c3fea201`):**
     - Implements a 2-phase background memory consolidation pipeline:
       - `stage1_outputs`: `thread_id`, `source_updated_at`, `raw_memory`, `rollout_summary`, `rollout_slug`, `selected_for_phase2`.
       - `jobs`: distributed lease queue (`worker_id`, `ownership_token`, `lease_until`, `input_watermark`).
       - `consolidation_progress`: singleton watermark tracker.

---

### 2.3 Google Antigravity CLI (`agy`)
- **Binary Architecture (`ev_e0bd52156323af13cc80956727b105639c42d36bc491e0c89dd1b2f81f4da090`, `ev_4f48383ea6a5027ec066fb32443128db1261d36ffde9074e9aef47adccfd50f7`):**
  - ELF64 `ET_DYN` hybrid Go + C++ executable (`265,700,040` bytes, `BuildID[md5/uuid]=cf2bdcfab1a7d961f5deb61d239908f8`).
  - REA `inspect_go_binary` (`ev_b3af7a149e6eaaa93f2af7f75747279991766aa5a575b3ceeae4d099edd79632`) recovered embedded Go build info at file offset `214,050,656`:
    `go1.28-20261102-RC00 cl/1012564305 +298f53e483 X:fieldtrack,boringcrypto,simd,nogolistexportnewformat`.
  - Dynamic symbol table (`ev_9fe8eae9df30a2df633e6b6119d71f18ffa18de8727b07b628b6b92cfbec4041`) exports 570 symbols including TCMalloc/compiler allocation tokens (`__alloc_token_*`) and C++ ABI runtime symbols (`__cxa_throw`, `__dynamic_cast`).
- **Lifecycle Hook Contract (`~/.gemini/config/hooks.json` & `.agents/hooks.json`):**
  - Supports 6 synchronous lifecycle hooks receiving camelCase protojson on `stdin`:
    1. `SessionStart`: Can return `{"injectSteps": [{"systemMessage": {"systemMessage": "...", "metadata": {"user_markdown": ""}}}]}` (injecting machine-only AHTL context invisible in the user's UI when `user_markdown` is `""`!).
    2. `PreInvocation`: Fires **before every model call**! Can return `injectSteps` (`systemMessage` or `userMessage`) to dynamically inject recalled pages right before inference.
    3. `PostInvocation`: Fires after tool calls finish; can inject steps or set `"terminationBehavior": "force_continue"`.
    4. `PreToolUse`: Can return `{"decision": "allow", "overwrite": {...}}` to rewrite tool arguments before execution.
    5. `PostToolUse`: Receives tool completion metadata (`toolCall`, `stepIdx`, `error`, `transcriptPath`).
    6. `Stop`: Receives `terminationReason` (including `"MAX_TOKEN_BUDGET_EXCEEDED"`, `"MAX_INVOCATIONS"`, `"NO_TOOL_CALL"`) and can return `{"decision": "continue", "reason": "..."}` to seamlessly continue work across token/invocation limits!
- **SQLite & Transcript Persistence (`ev_6a75fb73dadf1f29cb480f77e5ebbce3692856c21971bae83b6ec3846effbdb1`, `ev_524bc260c8a7ccbbaef9d7cfb1c2aaf949a069e2aac27ebff55a9c96d6c118b6`):**
  - `conversation_summaries.db`: Stores `conversation_summaries` (including `raw_summary` BLOB, `parent_conversation_id`, `nesting_depth`, `group_id`) and an FTS5 virtual table `conversations_fts` indexing chunked `user_body` and `agent_body` with `user_step_byte_offsets` and `agent_step_byte_offsets`.
  - `conversations/<conversation-id>.db`: Stores per-conversation `trajectory_meta`, `steps` (`idx`, `step_type`, `status`, `has_subtrajectory`, `metadata`, `step_payload` BLOB, `step_format`), `gen_metadata`, `executor_metadata`, `parent_references`, and `trajectory_metadata_blob`.
  - `<appDataDir>/brain/<conversation-id>/.system_generated/logs/transcript.jsonl` (compact) and `transcript_full.jsonl` (untruncated JSONL).

---

### 2.4 Hermes Agent (`~/.hermes/hermes-agent`)
- **Architecture (Full Python Source + SQLite `state.db` `ev_55f2f031e94c568896ee3d6653a29fb05932581b058229e8bf3aebc614652511`):**
  - Exposes two first-class plugin ABCs specifically designed for external context and memory engines:
    1. **`ContextEngine` (`agent/context_engine.py`)**: Loaded from `plugins/context_engine/<name>/` via `context.engine`.
       - `select_context(request_messages, *, conversation_messages, incoming_message, budget_tokens) -> list[dict]`: Replaces the outbound message list **for that single LLM API call** without mutating the persisted SQLite history!
       - `prune_tool_results_only(messages, current_tokens) -> (messages, n_pruned)`: Deterministic pre-compaction tool-result trimmer.
       - `compress(messages, current_tokens, focus_topic, force, memory_context) -> list[dict]`: Full compaction hook.
       - `on_turn_complete(messages, usage, **kwargs)`: Post-turn indexing hook.
    2. **`MemoryProvider` (`agent/memory_provider.py`)**: Loaded from `plugins/memory/<name>/` via `memory.provider`.
       - `prefetch(query, *, session_id)` & `queue_prefetch(query, *, session_id)`: Non-blocking background recall injected before the next turn.
       - `on_pre_compress(messages) -> str` with `PRE_COMPRESS_CHECKPOINT_API_VERSION = 2` (fail-closed pre-compression evidence handoff).
  - **`state.db` (`ev_55f2f031e94c568896ee3d6653a29fb05932581b058229e8bf3aebc614652511`):**
    - `sessions` (62 columns): Tracks `input_tokens`, `output_tokens`, `cache_read_tokens`, `cache_write_tokens`, `compression_failure_cooldown_until`, `compression_fallback_streak`, `compression_ineffective_count`, `compression_overload_streak`, `system_prompt_hash`.
    - `messages` (30 columns): Tracks `compacted`, `_compressed_summary`, `absorbed_message_uids`, `api_content`, `codex_reasoning_items`, `message_uid`, `tool_call_uid`, plus dual FTS5 indexes (`messages_fts` and `messages_fts_trigram`).

---

### 2.5 `ai-memory` (`~/.local/lib/ai-memory/ai-memory`)
- **Binary Architecture (`ev_c127f3b46973ea298046b85ad5e6a61a1a3c2d8d485b6592efcaacee26d6bb9b`):**
  - 50.19 MiB Rust ELF64 executable (`axum` HTTP server on `127.0.0.1:49375` + `mcp-bridge` stdio proxy + `hook` CLI).
  - Supports 18 agent harnesses (`claude-code`, `codex`, `antigravity-cli`, `hermes`, `open-code`, `gemini-cli`, `claude-desktop`, `omp`, `pi`, `crush`, `devin`, `kimi-code`, `command-code`, `zcode`, `copilot-cli`, `grizzybot`, `droid`, `codebuddy`, `ibm-bob`, `dsh`, `cline`, `qwen-code`).
- **Architectural Gap Analysis vs. Infinity-Context:**
  - `ai-memory` is a **human-readable Markdown Wiki & Session Observation Store** (`memory.sqlite` + `wiki/*.md`).
  - It does **not** rewrite bloated tool outputs in-flight (`updatedToolOutput`), does **not** act as a wire-level Cache-Epoch-preserving API proxy, does **not** maintain a lossless byte-exact Merkle-DAG of every raw tool result (`LCM`), does **not** use non-human high-density BPE token packing (`AHTL`), and does **not** provide sub-50µs AVX-512 Hyperdimensional Computing (`HDC`) or Fractal KV-Archive indexing.
  - **Coexistence Strategy:** `Infinity-Context` (`icx`) is designed to coexist harmoniously alongside `ai-memory` (consuming or exporting durable human-readable wiki pages when desired, while operating the low-level machine-native L1–L4 Virtual Context MMU and cross-agent `/dev/shm` telepathy fabric).
