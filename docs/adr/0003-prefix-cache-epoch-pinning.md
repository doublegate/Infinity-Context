# ADR 0003: Prefix-Cache Epoch Pinning and Ephemeral Tail Slot

## Status

Historical decision (2026-11-16); unlimited pinning and provider-cache/TTFT claims superseded by
[ADR0006](0006-v1-2-0-context-and-compatibility.md). Preserve the original rationale below.

## Context

Per `arXiv:2610.28577` (*Cache Engineering for Agents*), mutating or trimming messages in the middle of a conversation array invalidates every subsequent token in the provider's KV prefix cache (Anthropic `cache_control`, OpenAI automatic 1024-token prefix caching, Gemini context caching), increasing input token cost by up to 10x and Time-To-First-Token (TTFT) by 4x–8x.

## Decision

We enforce **Append-Only Cache-Epoch Pinning (`CacheEpochController`)**:
1. Historical turns are sealed into immutable, cryptographically hashed **Cache Epochs (`E_0 .. E_{k-1}`)** only when the active unsealed window exceeds `epoch_rollover_threshold` (default `16,384` tokens).
2. Once sealed, an epoch's byte sequence is never modified on subsequent turns.
3. All per-turn dynamic associative recall (`HDC`, `FCGA`, `PPR`) and cross-agent `/dev/shm` telepathy packets are placed **exclusively in the Ephemeral Tail Slot** at the very end of the outbound prompt after the cached prefix.

## Consequences

- **Positive:** Sustains `>= 94%` provider prompt-cache hit rates across 500+ turn sessions while still delivering dynamic per-turn recall.
- **Negative:** A low-surprise turn remains in the active unsealed window until the next 16K-token epoch boundary (though bulky tool outputs $> 2\text{ KiB}$ are still stubbed immediately at `PostToolUse` before entering the window).
