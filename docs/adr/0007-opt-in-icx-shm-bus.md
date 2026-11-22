# ADR 0007: Opt-in memory-mapped seqlock bus (`icx-shm`)

Status: proposed, 2026-11-22 UTC, for a future release. Tracked in
[issue #32](https://github.com/doublegate/Infinity-Context/issues/32). Builds on ADR 0004, whose
zero-copy parts ADR 0005 superseded, and does not change ADR 0005 for the default build.

## Context

ADR 0004 designed a lock-free, zero-copy `/dev/shm` seqlock ring for cross-agent events. The
shipped bus (`crates/icx-bus/src/ring.rs`) has two variants:

- an in-process ring of 256 slots of 4,096 bytes on atomics (about 58 ns per publish);
- a file-backed variant that uses positional I/O (`pread`/`pwrite`) under a writer mutex and a
  file lock. It is not memory-mapped, lock-free or zero-copy. Its publish-to-visible worst case
  between two handles is 38 µs over 200 events.

The gap exists because every crate compiles with `unsafe_code = "forbid"`. Mapping a file
(`memmap2::MmapMut::map_mut`) and reading and writing the mapped words as atomics that another
process also writes both require `unsafe`. The README's patent summary still describes the
designed bus, so the README now carries a note that points to the shipped implementation.

The bus is not a bottleneck today. A hook through the resident daemon takes about 3.1 ms at
the median, so the bus is roughly 1% of a hook, and agents work on a scale of seconds per turn.

## Decision (proposed)

Add an **opt-in** crate, `icx-shm`, that implements the cross-process bus as a memory-mapped
seqlock ring behind a Cargo feature (for example `shm-bus`) that is off by default:

1. `icx-shm` is the only crate allowed to use `unsafe`, with a `// SAFETY:` comment on every
   block. Every other crate keeps `forbid`. Because a crate attribute cannot relax an inherited
   `forbid` (E0453), `crates/icx-shm/Cargo.toml` declares its own `[lints.rust]` table
   (`unsafe_code = "deny"`, allowed only on the audited items) instead of
   `[lints] workspace = true`, and CI checks that no other crate drops the workspace lints.
2. The ring keeps a sequence number per slot, odd while it is being written. Every data word is
   read and written as an atomic: plain copies of concurrently written memory would be data
   races, and so undefined behaviour in Rust's memory model. A reader retries when the sequence
   is odd or changed during the read.
3. Several writers claim slots without a lock, using a compare-and-swap ticket per slot. A slot
   left odd by a writer that died is recovered after a deadline. Loss is reported exactly as
   today (`SlotLapped`, `SlotContended`, `dropped_publishes`, `poll_since_with_loss`).
4. The mapping gets the same hardening as the current bus file: `O_NOFOLLOW`, owner-only mode,
   refusal of foreign, symlinked or permissive files, a size check, and a clean fallback when
   `/dev/shm` is missing or full.
5. At run time the shm backend is used only when the feature is compiled in and the platform
   supports it. Otherwise the file-backed bus is used, unchanged.
6. Qualification: `loom` models of the memory ordering, multi-process stress tests (torn reads,
   writer crash, wrap-around) and fuzzing of slot decoding. Miri cannot check memory-mapped
   files, so the multi-process tests are the release gate. Cross-process latency is measured and
   published in `docs/numbers.md` against the file-backed bus.

## Consequences

**Benefits:**

- sub-microsecond cross-process visibility;
- readers that never wait on a writer's file lock;
- a shipped implementation that matches the ADR 0004 design and the patent summary;
- headroom for future high-frequency cross-agent streaming.

**Costs:**

- The policy "no `unsafe` code anywhere" becomes "`unsafe` confined to `icx-shm`, opt-in", and
  the README, Security section, `AGENTS.md` and CI's unsafe gate must say so precisely.
- Cross-process seqlock ordering is subtle. A single wrong ordering produces torn reads that
  tests rarely catch, so the code needs careful review and the `loom` and multi-process suites.
- `/dev/shm` is Linux-specific: macOS needs `shm_open`, containers default to a 64 MB
  `/dev/shm`, and sandboxed agents may not see it. The fallback path must stay first-class.
- Security does not improve: any process running as the same user can still write the bus, as
  today.
- It is more code to maintain for a path that is about 1% of hook time today.

**Unchanged:** with the feature off, builds, tests, benchmarks and the shipped binaries are
identical to the current release, following the project's rule for additive features.

**Alternatives considered:**

- Keep only the safe file-backed bus and document it accurately (the status quo since v1.1.0).
- Route cross-agent events through the resident daemon's Unix socket instead of a shared file.
- Use a third-party shared-memory crate: its `unsafe` would still be ours to audit.
