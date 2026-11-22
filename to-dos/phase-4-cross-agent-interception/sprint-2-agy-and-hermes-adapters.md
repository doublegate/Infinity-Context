# Sprint 2 — Antigravity CLI (`agy`) & Hermes ContextEngine Adapters

**Phase:** Phase 4 — Cross-Agent Interception  
**Sprint Goal:** Implement `AntigravityAdapter` (`injectSteps`, `overwrite`, `Stop` continuation) and `HermesAdapter` (`select_context` / `prune_tool_results_only`) in `crates/icx-adapters`.  
**Estimated Duration:** 1 week

## Tickets

### T-42-001 — Implement `AntigravityAdapter` (`PreInvocation` UI-Silent `injectSteps` & `Stop` Continuation)

**Description:** Implement `AntigravityAdapter` for `SessionStart`, `PreInvocation`, `PostInvocation`, `PreToolUse`, `PostToolUse`, and `Stop` using camelCase protojson keys (`conversationId`, `injectSteps`, `metadata.user_markdown: ""`, `terminationBehavior`).

**Acceptance Criteria:**

- [x] `PreInvocation` injects recalled AHTL frames inside `systemMessage` with `metadata.user_markdown: ""` so the human UI remains clean while the model receives the machine context.
- [x] `Stop` with `terminationReason: "MAX_TOKEN_BUDGET_EXCEEDED"` and `continuationActive: false` folds the epoch and returns `{"decision": "continue", ...}`.

**Dependencies:** T-41-001  
**Reference:** `docs/agent-adapters-and-interception.md` §4  
**Estimated Complexity:** M

---

### T-42-002 — Implement `HermesAdapter` (`ContextEngine.select_context` & `MemoryProvider` Bridge)

**Description:** Implement `HermesAdapter` transforming OpenAI-style `request_messages` into Cache-Epoch-aligned AHTL frames + Ephemeral Tail Slot for Hermes's `ContextEngine.select_context()`.

**Acceptance Criteria:**

- [x] Preserves the system head and `protect_last_n` tail messages verbatim while replacing middle-history tool bloat with sealed AHTL epochs.

**Dependencies:** T-42-001  
**Reference:** `docs/agent-adapters-and-interception.md` §5  
**Estimated Complexity:** M
