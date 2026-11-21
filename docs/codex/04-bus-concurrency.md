# 04 — Bus, sequence consistency and cross-process propagation

Revision `a996d59f1eb468e7c386747e15f4c7e58d08410e`; source: `crates/icx-bus/src/ring.rs`, MMU bus cursors and `crates/icx-bus/tests/loss_signalling.rs`. Normative comparison: `docs/wire-protocol-and-shm-bus.md` §§2.3–2.6.

## ICX-CODEX-032 — Reader accepts coherent slots outside its expected sequence

| Field | Assessment |
| --- | --- |
| Category / severity / confidence | Concurrency/specification discrepancy; Low; High for missing invariant, Medium for production race impact |
| Location | `crates/icx-bus/src/ring.rs:839`, corresponding SharedFile branch around `:916`; `poll_since_with_loss` at `:807` |
| Prerequisites | Slot is reused after the reader samples the head, or corrupt slot metadata; a deterministic probe supplies an inconsistent version through the public in-process atomic layout |
| Impact | Reader checks only `committed_seq > since_seq` and matching even versions. It can return a record beyond the sampled head instead of the expected `seq + 1`; advancing a consumer cursor to that sequence may skip intervening records. The loss wrapper can report zero loss for a future record against a head of one. |
| Reproduction | `state-probe.rs` publishes one event and changes its coherent even sequence version to 514. `poll_since_with_loss(0)` returns `seq=257` while `write_sequence()` is 1. This deterministically demonstrates missing validation, not an observed naturally scheduled 256-publish race. |
| Counterevidence | Seqlock parity and double-read checks prevent accepting a changed/torn payload. Events are sorted and deduplicated; private file permissions limit outside writers. Reinitialization/corruption is not authenticated and same-user writers are trusted. No cross-user exploit is claimed. |
| Recommendation | Require the slot version to match the expected sequence and define a stable captured-head interval; count skipped numbers against that interval. Return an explicit restart/generation signal after fabric reset. |
| Acceptance | A deterministic pause-between-head-and-slot-read test lets a writer lap the ring, then asserts every returned sequence lies in the captured interval or produces an explicit gap/retry result; malformed/future versions are refused. |
| Compatibility | Additional validation preserves event format but may expose more loss/retry outcomes. A fabric-generation field requires a versioned layout migration. |
| Related prior IDs | ICX-AUD-054; current loss APIs improve observability but do not establish this stronger invariant |

## Positive results and qualified observations

- In-process data words use atomics, with writer CAS, release commit and reader acquire/version validation. No data-race or memory-safety claim follows merely from a name containing “seqlock”; bounded retry can drop events.
- SharedFile writers use a mutex and OS file lock. The header advances after the slot's even version. The transport is positional I/O, not mapped memory, and is only on tmpfs when configured there.
- File open uses `O_NOFOLLOW | O_CLOEXEC`, rejects non-regular, foreign-owned or group/other-accessible files. Invalid nonzero magic/version is rejected.
- `try_publish` now returns `SlotLapped`/`SlotContended`; `publish` counts failure locally. `poll_since_with_loss` exists. Ordinary `poll_since`/workspace-filtered consumers do not automatically receive loss information or repair it from CAS.
- A truncated pre-existing file is extended and initialized as a new fabric, even when it previously held valid traffic. The probe records head reset to zero. This is an observed recovery policy, not a new confidentiality vulnerability. A generation/diagnostic would make reset visible.
- Payloads beyond 3,968 UTF-8 bytes are truncated on a character boundary. This is documented; full-fidelity packet transport is not promised for oversized inline records.

## Separate-process measurement

`state-probe.rs` starts a distinct child writer, opens a second file handle in the parent, sends 100 events paced at 1 ms, and polls every approximately 50 µs. The payload carries a UNIX nanosecond timestamp immediately before publish. `state-probe-results.jsonl` records the count, p50/p99/max publish-to-observe intervals. The measurement includes scheduler wake-up, polling delay and timestamp cost; it is not an isolated syscall benchmark or a four-agent end-to-end bound. Wall-clock adjustment can affect this simple local measurement. No sub-100-µs guarantee is inferred.

## Prior-audit reassessment

| Prior ID | Status | Evidence and remaining limit |
| --- | --- | --- |
| ICX-AUD-019 | Confirmed fixed | File descriptor flags, `fstat` ownership/regular-file/mode checks are present. Metadata/path race freedom against same-user writers is outside scope. |
| ICX-AUD-046 | Partially fixed | Main bus documentation now describes file I/O and writer locks accurately; stale module/API titles and diagram/ADR language survive (see report 08). |
| ICX-AUD-054 | Partially fixed | Typed publish failures and loss counters exist; expected sequence invariant still missing (ICX-CODEX-032). The MMU uses ordinary workspace-filtered polling, not a complete loss-repair protocol. |
| ICX-AUD-055 | Partially fixed | Existing test uses two handles in one process. This audit adds a real two-process measurement; it does not demonstrate all four real harnesses or establish their p99 propagation bound. |

## Coverage limits

Production bus source read by layout, attach, root-seqlock, publish, poll and loss paths; tests reviewed for oracle strength. The deterministic corrupted-version and truncated-fabric probes are not randomized process-death campaigns. No formal weak-memory model checker, syscall fault injector, 64-bit sequence-wrap test, or BSD/macOS positional-I/O test was run. POSIX does not by itself establish the full Linux page-cache ordering assumption described by the project.
