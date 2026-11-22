# Sprint 1 — Multi-Million-Token Needle-in-a-Haystack & Prefix-Cache Simulator

**Phase:** Phase 5 — Verification, Benchmarks & Release  
**Sprint Goal:** Build the end-to-end long-horizon Needle-in-a-Haystack passkey retrieval and Cache-Epoch prefix-hit simulator in integration tests and `icx benchmark`.  
**Estimated Duration:** 1 week

## Tickets

### T-51-001 — Implement Multi-Turn Needle-in-a-Haystack & OOLONG Passkey Verification Suite

**Description:** Simulate a long-horizon coding session with planted high-surprise passkeys, memory addresses, and failing assertions buried amidst hundreds of kilobytes of passing build logs and file reads.

**Acceptance Criteria:**

- [x] Active context window stays strictly within budget while ingesting `> 500 KB` of raw tool outputs.
- [x] 100% of planted passkeys and panic locations are retrieved via `query_associative` / `PPR` and losslessly dereferenced from `CasStore`.
- [x] Prefix-cache hit ratio across turns is `>= 94.0%`.

**Dependencies:** T-43-002  
**Reference:** `docs/testing-strategy.md` §1 (Layer 3)  
**Estimated Complexity:** M

---

### T-51-002 — Implement Cross-Agent Telepathy Integration Verification

**Description:** Verify end-to-end cross-agent state propagation from `ClaudeCodeAdapter` (`PostToolUse`) -> `SeqlockTelemetryBus` -> `AntigravityAdapter` (`PreInvocation`) & `CodexAdapter` (`UserPromptSubmit`) -> `McpPagerServer` (`deref`).

**Acceptance Criteria:**

- [x] Latency from publish to sibling agent read is `< 100 µs`.
- [x] Sibling agent receives the exact `⟦Θ:...⟧` telepathy frame and dereferences the raw artifact byte-for-byte.

**Dependencies:** T-13-002, T-42-001, T-43-001  
**Reference:** `docs/testing-strategy.md` §1 (Layer 2)  
**Estimated Complexity:** M
