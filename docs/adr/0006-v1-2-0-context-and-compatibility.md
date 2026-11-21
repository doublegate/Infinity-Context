# ADR 0006: v1.2.0 context limits, wire compatibility and evidence

Status: accepted for implementation, 2026-11-21 UTC. Supersedes quantitative capability claims
in ADR 0001–0003, and conflicting Q1.14 labels in the AHTL grammar/PDF. Historical decisions
remain available; this record states the current contracts.

## Context and decisions

The independent audit at a996d59 found credential scalar leaks, poor candidate recall,
unbounded assembled history, unsupported Codex output replacement, volatile daemon receipts
and contradictory specifications. See `docs/codex/`; implementation status lives outside that
frozen audit in `to-dos/v1.2.0-remediation.md` and `PROJECT-STATUS.md`.

1. CAS guarantees exact **post-redaction** bytes. Skeletonization is lossy. Unicode glyphs are
   not certified single BPE tokens, and byte ratios are not token compression or model recall.
2. Unversioned Profile A `fc` preserves the shipped signed Q15 scale 32,767, big-endian words.
   Q1.14 scale 16,384 is an explicit arithmetic API only. Independent literal vectors pin both.
   The frozen PDF's Q1.14 wire label is an erratum, not authorization to reinterpret old packets.
3. Profile A raw skeleton framing glyphs normalize to ASCII pairs just like ingestion;
   ordinary existing percent literals are unchanged. Raw nested framing is rejected on parse.
4. Exact token postings augment approximate HDC candidates and receive lexical precedence.
   HDC/fractal scoring orders ties and supplies nonlexical candidates. This changes ranking,
   not CAS identities. ICXHDC04 persists postings; 03 remains readable and is rebuilt from CAS
   events by the MMU. Old binaries cannot read 04; retain a pre-upgrade backup for downgrade.
5. Prompt selection is automatically bounded. Default accounting is 32,768 total units,
   4,096 external-input/tool-schema reserve and 4,096 completion reserve, using one UTF-8 byte
   per token allowance (`utf8-bytes-upper-bound-v1`). It is conservative estimation, not a
   measured provider tokenizer. Callers must reserve actual external content and model limits.
   The newest contiguous run of frames is kept (all active turns, then the newest sealed epochs)
   without deleting cold history. At saturation the oldest epochs are evicted in one step to half
   of the history allowance and the eviction point is persisted, so the retained prefix only
   grows by appending between steps; one `ICX:HISTORY_OMITTED` frame, which changes only at
   those steps, names the omitted range and its handles. Structured tail selection preserves its header, best recall and fitting complete
   bus/PPR frames; recoverable oversized events use handles and unsent bus events stay queued.
   The generic oversized raw-tail fallback is a bounded diagnostic, never a partial packet.
   A model-policy change can invalidate cache prefixes;
   byte identity is guaranteed only for retained unchanged frames, never an unlimited prefix.
6. Codex hook capture and supported start/prompt context remain useful, but PostToolUse output
   replacement is unavailable in the checked contract. Do not emit unsupported replacement
   fields or claim that capture suppresses billed original tool output.
7. Daemon success follows successful persistence under the workspace lock. Save failure is
   an error, not a volatile success. WAL synchronization is FULL. Durability remains subject to SQLite/filesystem
   synchronization and hardware behavior; no user data is purged as part of remediation.
8. Retrieval latency, IPC latency and cache utilization require measurements on named workloads.
   Synthetic byte-prefix reuse is not a provider cache-hit counter. Hardware SLO gates stay
   explicit; failed runs cannot become successes through revised prose.

## Consequences and qualification

Compatibility is backward **readability**, not an assurance that an older executable can read
new derived snapshots. Keep source artifacts and upgrade backups; never change archived
coordinate interpretation. The release must qualify all regressions, >10k turn window behavior,
restart/migration, representative recall and real packaged binaries. Remaining unsupported
platforms, live provider/model effectiveness and rights questions stay explicit release gaps.
