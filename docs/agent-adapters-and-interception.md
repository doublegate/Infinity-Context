# Agent Adapters, Wire Proxy & Interception Specification

**References:** [ref-docs/agent-internals/rea-binary-and-sqlite-forensics.md](../ref-docs/agent-internals/rea-binary-and-sqlite-forensics.md), [ref-docs/agent-internals/cross-agent-hook-and-wire-matrix.md](../ref-docs/agent-internals/cross-agent-hook-and-wire-matrix.md)  
**Target Crates:** `crates/icx-adapters`, `crates/icx-proxy`

---

## 1. Purpose

`crates/icx-adapters` and `crates/icx-proxy` connect the **Infinity-Context (`icx`)** Virtual Context MMU to every agent CLI and IDE on the workstation. They implement four concrete native adapters (**Claude Code**, **OpenAI Codex CLI**, **Google Antigravity CLI (`agy`)**, **Hermes Agent**), a generic fallback adapter for the remaining 14+ MCP/hook harnesses, an **HTTP/2 & SSE Cache-Epoch Reverse Proxy**, and a **SQLite WAL / JSONL Live Tailer**.

---

## 2. Claude Code Adapter (`ClaudeCodeAdapter`)

**Verified Binary Target:** `~/.local/share/claude/versions/2.1.326` (`ev_be2e252614342586f9de78e5f08eb70de8465f18310bc03a948abe986a60ef72`)

### 2.1 Hook Event Handling (`icx hook --agent claude-code --event <event>`)

1. **`SessionStart`:**
   - Reads `{session_id, cwd, transcript_path}` from `stdin`.
   - Queries `/dev/shm/icx-fabric-v1` and `icx-store` for the workspace's active Cache Epochs and cross-agent telepathy commits.
   - Emits:
     ```json
     {
       "hookSpecificOutput": {
         "hookEventName": "SessionStart",
         "additionalContext": "⟪ICX:LEGEND│Δ=code_mut│Ψ=tool_stub│Ω=invariant│Θ=xagent│Φ=folded_branch│↦=deref_via_icx_tool⟫\n⟪ICX:v1│ep=0001│root=9f4ac1e2│τ=1794880800⟫..."
       }
     }
     ```
2. **`UserPromptSubmit`:**
   - Reads `{session_id, prompt, cwd}` from `stdin`.
   - Computes 10,240-bit HDC query hypervector + HippoRAG PPR walk + `/dev/shm` unseen sibling agent commits.
   - Returns the **Ephemeral Tail Slot** AHTL frame in `hookSpecificOutput.additionalContext`.
3. **`PostToolUse` (In-Flight Tool Output Virtualization):**
   - Reads `{tool_name, tool_input, tool_response, tool_use_id, session_id}` from `stdin`.
   - If serialized `tool_response` length $> 2,048$ bytes:
     1. Redacts secrets via `SecretRedactor`.
     2. Chunks and commits raw bytes into `CasStore` -> obtains `#icx:<hex16>`.
     3. Runs `StructuralSkeletonizer` + `TitansSurpriseGate` + `AhtlEncoder` -> produces compact `AHTL` stub (`<= 120` tokens).
     4. Publishes summary delta to `/dev/shm/icx-fabric-v1`.
     5. Returns:
        ```json
        {
          "hookSpecificOutput": {
            "hookEventName": "PostToolUse",
            "updatedToolOutput": "⟪ICX:STUB│#icx:7a2e91f0ab12cd34│raw=64,120B→298B│lines=1..1420⟫\n⟦Ψ:Bash⊢exit=0⋮✓pass=84⋮↦#icx:7a2e91f0ab12cd34(L1-1420)⋮Ψ:⠞⠧a9f1⟧"
          }
        }
        ```
4. **`PreCompact` & `PostCompact`:**
   - On `PreCompact`: Checkpoints all unsealed `L1` turns into a sealed `L3 CacheEpochNode` in the Merkle-DAG.
   - On `PostCompact`: Injects the lossless Merkle-DAG Epoch Frame + active `#icx:<hash>` table of contents via `systemMessage` so zero exact identifiers or line numbers are lost across compaction.

---

## 3. OpenAI Codex CLI Adapter (`CodexAdapter`)

**Verified Binary Target:** `~/.codex/packages/standalone/releases/0.169.0-x86_64-unknown-linux-musl/bin/codex` (`ev_7cda5928199ea58224bdedbbd14892ff224515db64f7679457f633d5e7d816d3`)

### 3.1 Hook & Wire Integration
1. **`session-start` & `user-prompt-submit`:**
   - Validates against Codex's embedded `session-start.command.output` and `user-prompt-submit.command.output` JSON schemas (`hookSpecificOutput.additionalContext`, `systemMessage`, `continue: true`).
2. **`post-tool-use`:**
   - Uses `hookSpecificOutput.updatedMCPToolOutput` for MCP tools and `hookSpecificOutput.additionalContext` + `systemMessage` for native unified exec tools, while storing full outputs in CAS.
3. **`pre-compact` & `post-compact`:**
   - Emits `{"continue": true, "systemMessage": "⟪ICX:LCM_CHECKPOINT│...⟫"}` matching `pre-compact.command.output`.
4. **SQLite & Rollout Tailer (`CodexWalTailer`):**
   - Watches `~/.codex/state_5.sqlite` (`ev_3e4a7d4314b4d31f13103a257120fda8c23e0a884187421d7a47b4a75f1c29a5`) for active `threads.rollout_path` and ingests `RolloutItemWire` JSONL entries in real time.

---

## 4. Google Antigravity CLI (`agy`) Adapter (`AntigravityAdapter`)

**Verified Binary Target:** `~/.local/bin/agy` (`ev_e0bd52156323af13cc80956727b105639c42d36bc491e0c89dd1b2f81f4da090`, `ev_4f48383ea6a5027ec066fb32443128db1261d36ffde9074e9aef47adccfd50f7`)

### 4.1 camelCase Protojson Hook Contract
1. **`SessionStart` & `PreInvocation`:**
   - Parses `{conversationId, workspacePaths, transcriptPath, artifactDirectoryPath, modelName, lastUserInput, hookEventName, cwd}`.
   - Runs HDC/PPR/Fractal recall + `/dev/shm` cross-agent poll.
   - Emits UI-silent `systemMessage` (`metadata.user_markdown: ""`):
     ```json
     {
       "injectSteps": [
         {
           "systemMessage": {
             "systemMessage": "⟪ICX:TAIL_SLOT│conv=983dcb21⟫\n⟦Θ:codex-desktop@crates/icx-core/src/dag.rs⊢⊕fn:seal_epoch↦#icx:19fa0042⟧",
             "metadata": {
               "user_markdown": ""
             }
           }
         }
       ],
       "terminationBehavior": ""
     }
     ```
2. **`PreToolUse`:**
   - Supports `"overwrite"` to bound runaway command outputs (e.g., automatically piping unbounded log dumps through `icx capture-pipe` when appropriate) while returning `{"decision": "allow"}`.
3. **`PostToolUse`:**
   - Reads the latest step from `transcriptPath` (`transcript_full.jsonl` or `conversations/<id>.db` `steps.step_payload`), commits to `CasStore`, and broadcasts to `/dev/shm`.
4. **`Stop` (Infinite Horizon Continuation):**
   - If `terminationReason == "MAX_TOKEN_BUDGET_EXCEEDED"` or `"MAX_INVOCATIONS"` and `!continuationActive`, folds the current trajectory epoch into `MerkleContextDag` and returns:
     ```json
     {
       "decision": "continue",
       "reason": "⟪ICX:EPOCH_FOLDED│#icx:88ef1209│Resume from sealed AHTL state⟫"
     }
     ```

---

## 5. Hermes Agent Adapter (`HermesAdapter`)

**Verified Target:** `~/.hermes/hermes-agent/agent/context_engine.py` & `memory_provider.py` (`ev_55f2f031e94c568896ee3d6653a29fb05932581b058229e8bf3aebc614652511`)

- Provides a Python shim plugin (`plugins/context_engine/infinity_context/`) that delegates `select_context()`, `prune_tool_results_only()`, `compress()`, and `on_turn_complete()` over Unix Domain Socket / `/dev/shm` to `icxd`.
- `select_context()` replaces historical message blocks with **Cache-Epoch-Pinned AHTL Frames** + the **Ephemeral Tail Slot** on every provider request without mutating Hermes's `state.db` `messages` table.

---

## 6. Wire-Level Reverse Proxy (`crates/icx-proxy`)

`icx-proxy` listens on `127.0.0.1:49380` (and `/run/user/$UID/icx-proxy.sock`) and exposes three upstream-compatible routes:
- `POST /anthropic/v1/messages` (for `ANTHROPIC_BASE_URL=http://127.0.0.1:49380/anthropic`)
- `POST /openai/v1/responses` & `POST /openai/v1/chat/completions` (for `OPENAI_BASE_URL=http://127.0.0.1:49380/openai/v1`)
- `POST /gemini/v1beta/models/*` (for `GEMINI_API_BASE_URL`)

### Proxy Transformations
1. **Outbound Request Pass:**
   - Identifies the conversation/session from headers or system prompt hash.
   - Replaces raw oversized middle-history tool results with their cached `AHTL` Epoch Frames (`Zone 1`), ensuring `Zone 0` and `Zone 1` remain 100% byte-identical across turns.
   - Injects Anthropic `cache_control: {"type": "ephemeral"}` on the last block of the latest sealed Cache Epoch.
   - Appends the `Ephemeral Tail Slot` (`<= 1,024` tokens) containing HDC/PPR/Fractal recall and `/dev/shm` cross-agent telepathy updates to the final user message.
2. **Inbound Streaming SSE Pass:**
   - Monitors streaming assistant deltas for inline dereference directives (`⟪!icx:deref(#icx:...)⟫`) if direct proxy expansion is enabled.
