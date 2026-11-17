# Sprint 3 — Single-Tool Polymorphic MCP Pager & Cache-Epoch Wire Proxy

**Phase:** Phase 4 — Cross-Agent Interception  
**Sprint Goal:** Implement the single-tool polymorphic MCP server (`icx`) in `crates/icx-adapters` and the Cache-Epoch wire transformer in `crates/icx-proxy`.  
**Estimated Duration:** 1 week

## Tickets

### T-43-001 — Implement Single-Tool Polymorphic MCP Pager (`icx`)

**Description:** Implement JSON-RPC 2.0 MCP handling (`initialize`, `tools/list`, `tools/call`) exposing the single `icx` tool supporting `op: "deref" | "slice" | "grep" | "recall" | "fold" | "xagent" | "status"`.

**Acceptance Criteria:**
- [x] `tools/list` returns a single tool schema under 180 tokens.
- [x] `op: "deref"` and `op: "slice"` losslessly return full or line-sliced CAS artifacts by `#icx:<hex>` handle.
- [x] `op: "grep"` searches cold storage and returns line-numbered matches.
- [x] `op: "recall"` queries HDC + PPR + Fractal indices.

**Dependencies:** T-41-001, T-42-001  
**Reference:** `docs/wire-protocol-and-shm-bus.md` §3  
**Estimated Complexity:** M

---

### T-43-002 — Implement Cache-Epoch Wire Transformer (`icx-proxy`)

**Description:** Implement outbound request transformation for Anthropic (`/v1/messages`) and OpenAI (`/v1/responses`, `/v1/chat/completions`) payloads in `crates/icx-proxy`, injecting `cache_control: {"type": "ephemeral"}` at the sealed Cache-Epoch boundary and appending the Ephemeral Tail Slot.

**Acceptance Criteria:**
- [x] Outbound message prefix through the latest sealed Cache Epoch remains 100% byte-identical across consecutive turns.
- [x] `cache_control` breakpoint is placed accurately on the final block of the sealed epoch.

**Dependencies:** T-12-003, T-43-001  
**Reference:** `docs/agent-adapters-and-interception.md` §6  
**Estimated Complexity:** M
