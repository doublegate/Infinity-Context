# 05 — Agent contracts, MCP and HTTP/SSE proxy

Revision `a996d59f1eb468e7c386747e15f4c7e58d08410e`. Contracts checked against public primary documentation accessed 2026-11-21. No agent session, provider billing call, credential/configuration change or live user-store read was needed.

## ICX-CODEX-031 — MCP accepts invalid envelopes and inputs as successful operations

| Field | Assessment |
| --- | --- |
| Category / severity / confidence | Protocol/input-validation correctness; Low; High |
| Location | `crates/icx-adapters/src/mcp.rs:146`, `:271`, `:306`, `:470` |
| Prerequisites | MCP peer sends invalid JSON-RPC envelope or tool arguments; stdio is the existing trusted-client boundary |
| Impact | Missing/wrong `jsonrpc` and object-valued IDs are accepted. Missing `op` and unknown enum values dispatch to successful `status`. Invalid requests become ordinary successes rather than actionable protocol or tool-validation errors. No authority escalation is demonstrated. |
| Reproduction | `state-probe.rs` emits five exact request/response pairs: missing and wrong JSON-RPC version, object ID, missing op and `TYPO` op. Responses are retained in `state-probe-results.jsonl`. |
| Independent contract | JSON-RPC 2.0 §4 requires the version and a permitted ID type; MCP tools require input validation. Published `tool_schema` requires `op` and restricts it to an enum. |
| Counterevidence | JSON syntax failures yield `-32700`; unknown methods yield `-32601`; unknown tool names and missing dereference IDs yield errors; output counts and fold depth are bounded. Legacy negotiation fallback is permitted and is not itself a defect. |
| Recommendation | Validate request shape/version/ID before notification classification. Validate name and arguments against the advertised contract; distinguish unknown op from explicit status. For legacy streams, track initialization state or document a compatibility exception. |
| Acceptance | The five fixtures return `-32600`/`-32602` or the appropriate `isError:true` validation response; valid legacy and modern clients retain current successful operations. |
| Compatibility | Strict rejection changes behavior only for nonconforming requests; callers relying on missing-op status need an explicit compatibility path or migration notice. |
| Related prior IDs | ICX-AUD-008 (version negotiation repaired, complete conformance still not established) |

## Rejected lead 034 — Continuously streaming curl survives client cancellation indefinitely

The initial source-only lead overlooked ownership: the scoped relay's `up_reader` owns curl's stdout pipe and drops before the subsequent `child.wait()`. The next write by real curl therefore fails with EPIPE/SIGPIPE or its write-error handling. A continuously streaming provider does not establish indefinite worker retention.

`evidence/state-curl-cancel.py` ran actual curl against an isolated loopback SSE server, read the first data event, closed only the stdout pipe and waited. Curl exited with status 23 in **16.093 ms**, reporting a write error (`state-curl-cancel.json`, command exit 0). This directly falsifies the continuous-stream premise. The fixture uses HTTP locally to isolate stdout cancellation; it does not claim a full TLS proxy or every upload/error path was tested.

The ID `ICX-CODEX-034` is reserved/rejected and is **not a confirmed finding or included in the findings register**. Explicit child termination and an absolute request deadline could make cancellation budgets easier to reason about, but remain hardening suggestions without a demonstrated remaining resource-exhaustion path. Curl's existing low-speed timeout bounds an idle upstream separately.

## ICX-CODEX-035 — Codex adapter still emits an explicitly unsupported replacement field

| Field | Assessment |
| --- | --- |
| Category / severity / confidence | Integration capability gap; Medium; High for emitted field and upstream documentation; live effect not exercised |
| Location | `crates/icx-adapters/src/codex.rs:139`; `docs/agent-adapters-and-interception.md` §3 |
| Prerequisites | A non-pager `mcp__*` output exceeds virtualization threshold and the Codex PostToolUse hook runs |
| Impact | Adapter emits `updatedMCPToolOutput`, although current Codex documentation says it is unsupported and causes the hook run to be marked failed while the original tool result continues. The output is stored but is not replaced on this path. |
| Reproduction/source proof | Follow `PostToolUse` into `tool_name.starts_with("mcp__") && res.was_virtualized`; generated hso contains the field. Upstream contract checked at the URL below. No paid live run was performed. |
| Counterevidence | Current project documentation already discloses unsupported replacement and recommends the proxy. Built-in output no longer duplicates an AHTL packet via additionalContext. Ingestion/recall remains useful; this is a remaining gap, not a newly introduced confidentiality issue. |
| Recommendation | Omit unsupported output fields unless a version/capability check proves support. Document capture/recall separately from replacement; test actual hook acceptance against a pinned Codex version. |
| Acceptance | A real local fake-tool fixture under pinned Codex completes without a failed hook diagnostic, preserves original built-in behavior, and uses replacement only on a supported capability. |
| Compatibility | Removing an ignored/failed field is wire-output compatible for current Codex; future support can be feature-detected. |
| Related prior IDs | ICX-AUD-005 (still partially fixed) |

## Contract review

| Surface | Current evidence | Verification boundary |
| --- | --- | --- |
| Claude Code | `additionalContext` for start/prompt/subagent start; tool-shape-preserving PostToolUse; `last_assistant_message`; plain PreCompact stdout capped at 9,000 chars; PostCompact side effects only | Source and upstream align for inspected fields. Generic largest-string-leaf replacement is heuristic; no claim every built-in tool shape is validated by a live fixture. |
| Codex | PascalCase names, no built-in additionalContext duplication, MCP field remains unsupported | Public contract refreshed; no live harness started. Tailer is bounded JSONL library code, not CLI-wired and lacks zstd support. |
| Antigravity | Stop returns stop, never force-continues; transcript open is nonblocking/no-follow/regular-file restricted | API source inspected; UI invisibility via `metadata.user_markdown` is still not established by public contract or live UI. |
| Hermes | Rust context-engine operations preserve selected message groups; no Python shim ships | No end-to-end installed plugin claimed. User-written shim is a deployment prerequisite; public Rust logic is not a working native Hermes plugin by itself. |
| MCP legacy | Advertises supported legacy revisions and negotiates accepted version/fallback | No connection-scoped initialized state in `McpPagerServer`; full lifecycle conformance not established. |
| MCP modern | `server/discover`, request `_meta` version check, result metadata | 2026-07-28 is a real modern revision; do not incorrectly flag absence of a legacy initialize handshake for modern requests. |
| HTTP | Request body 16 MiB; SSE line 16 MiB; non-SSE response 64 MiB; 256 header lines, each up to 65,536 bytes; 64 workers | Limits are finite but can still aggregate substantial memory. Per-read timeouts are inactivity budgets, not complete operation deadlines. No unbounded-header finding is accepted. |
| Auth/routing | Token before body use; internal token stripped; loopback default; remote override explicit; loopback-only header upstream override | Same-user controls are not sandbox boundaries; no demonstrated unauthenticated CAS route remains. |
| Anthropic caching | Client breakpoints retained; at most one inserted block breakpoint while count <4; 1h inheritance from later breakpoint | Source/fixtures align with the inspected rules. Byte-prefix stability does not prove provider cache hits. Actual usage counters require an explicitly authorized provider experiment. |
| SSE | Streaming deref and per-path pending fragments, bounded line handling | Source/fixture inspection, not exhaustive provider-event-version compatibility. HTTP/1.1 listener and curl `--http1.1` are shipped; HTTP/2 advertisement belongs to historical design. |

## Prior-audit reassessment

| Prior ID | Status | Evidence / qualification |
| --- | --- | --- |
| ICX-AUD-003 | Confirmed fixed for identified placement/stripping defects | Content-block insertion, preservation/count of client breakpoints and TTL ordering logic; original broad prefix-rewrite allegation was overstated. Provider hit effectiveness remains unverified. |
| ICX-AUD-004 | Confirmed fixed for inspected event fields | Shape-aware Bash object, PreCompact stdout, proper stop input; source and public docs. Every generic tool schema has not been exercised live. |
| ICX-AUD-005 | Partially fixed | Built-in duplicate context removed and configuration docs corrected; unsupported MCP replacement persists (035), rollout zstd/CLI wiring absent. |
| ICX-AUD-006 | Partially fixed | Invalid ready-to-run shim claims removed, absence of Python integration disclosed; no shim ships. |
| ICX-AUD-007 | Partially fixed | Stop/budget override repaired, named config locations corrected; undocumented UI-hidden behavior remains unverified. |
| ICX-AUD-008 | Partially fixed | Legacy negotiation and modern discovery added; malformed input validation still incomplete (031). |
| ICX-AUD-009 | Confirmed fixed | PreToolUse no longer echoes output-only permission fields for Claude/Codex. |
| ICX-AUD-011 | Confirmed fixed for prefix-marker bypass | Exact emitted-block digest membership plus Tier-0 escaping replaces prefix-based trust. This cannot prevent natural-language prompt injection; that limitation is documented. |
| ICX-AUD-012 | Confirmed fixed for default CLI route | Token check on all routes, loopback bind restriction in CLI; opt-out flags explicitly weaken controls. |
| ICX-AUD-018 | Confirmed fixed in source/tests | Nonblocking no-follow transcript open plus regular-file check. |
| ICX-AUD-020 | Partially fixed | Private scratch files, RAII cleanup and periodic/startup sweeps exist. SIGKILL residue persists until a sweep as documented. The additional continuous-stream cancellation lead 034 was rejected after real-curl countertesting. |

## Primary-source ledger

All accessed 2026-11-21; source references are paraphrased and version-scoped.

| Source | Supporting section / result |
| --- | --- |
| [JSON-RPC 2.0](https://www.jsonrpc.org/specification) | §4 envelope version/ID types and §5 errors; oracle for 031 |
| [MCP 2025-11-25 tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools) | Input validation and error handling; tool schema must match executable validation |
| [MCP 2025-11-25 lifecycle](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle) | Legacy initialization/version negotiation |
| [MCP 2026-07-28 versioning](https://modelcontextprotocol.io/specification/2026-07-28/basic/versioning) | Modern per-request metadata and discovery; redirects from old lifecycle URL |
| [Claude Code hooks](https://code.claude.com/docs/en/hooks) | PostToolUse shape compatibility, MCP replacement, PreCompact stdout cap and PostCompact input |
| [Codex hooks](https://developers.openai.com/codex/hooks) | Redirected to learn.chatgpt.com/docs/hooks; unsupported MCP replacement and diagnostic-only model-request hooks |
| [Anthropic prompt caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching) | Breakpoint scope/limits and TTL ordering; no real cache-read test performed |

## Coverage limits

Complete targeted reads of MCP and Claude adapter; production Codex hook branches and rollout tailer contract; Antigravity transcript/stop paths; Hermes/proxy conversation transformation and forge checks; HTTP parsing/auth/routing, forwarders, SSE relay and listener cleanup. Tests and the deployment/forensics corpus were inspected by relevant field/symbol, not asserted as fully replayed historical evidence. Source locations and existing tests establish the conclusions above; fresh baseline suite results are in report 06. No live four-harness compatibility, TLS cancellation reproduction, HTTP/2 client implementation, runtime model effectiveness, or formal protocol-schema validator run is claimed.
