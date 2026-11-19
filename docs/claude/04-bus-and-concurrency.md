# 04 — Telepathy Bus and Concurrency (icx-bus)

Workstream W4.

## ICX-AUD-046 — "Lock-free `/dev/shm` seqlock" describes neither backend accurately (Medium, claims)

| Claim (docs) | Code reality |
| --- | --- |
| Lock-free `/dev/shm` shared-memory seqlock, zero-copy (README, `docs/wire-protocol-and-shm-bus.md`, ADR-0004) | Two backends (`ring.rs:244`): `InProcess(Arc<FabricMemoryLayout>)` is a real atomic seqlock but exists only inside one process. `SharedFile` uses `pread`/`pwrite` (`read_exact_at`/`write_all_at`) on an ordinary file: no mmap (impossible under `unsafe_code = "forbid"`), and no zero-copy. |
| `/dev/shm/icx-fabric-v1` | Default path is `.icx/icx-fabric-v1.shm` unless `ICX_SHM_PATH` is set (`icx-cli/src/main.rs:124-128`, `mmu.rs:1142-1150`) |
| Lock-free | Readers take no lock. Writers take an in-process `Mutex`, then `flock` (`File::try_lock`), retried up to 5,000 × 50 µs ≈ 250 ms (`ring.rs:686-747`) |
| "memory-mapped", `memmap2`/`libc` (research report :122, :144; `wire-protocol-and-shm-bus.md:13`) | Not present; contradicts `forbid(unsafe)` |
| `<300 ns` / `<500 ns` / `<800 ns` / `~45 ns` publish (four different documents) | The benchmark measures **only the in-memory backend**: 62.9 ns median. The cross-process file backend is not benchmarked |

**Remediation.** Describe the shipped design accurately: a file-backed ring with seqlock
validation, positional I/O, single-writer locking and lock-free readers. Benchmark the
`SharedFile` path cross-process. If true shared memory is a goal, it needs a small audited
`unsafe` mmap shim in one module, which conflicts with the project's permanent
`forbid(unsafe)` policy. That trade-off belongs in an ADR, not in marketing copy.

## ICX-AUD-054 — Silent event loss with success returned (Medium)

- **Write paths that report success without writing:**
  - A writer that finds its slot already advanced by a lapping writer returns `Ok(seq+1)` without
    writing (`ring.rs:631-633`).
  - A writer that spins `MAX_WRITER_SPIN_ITERS` (4,096) times on an odd slot gives up and returns
    `Ok(seq+1)` (`ring.rs:647-651`).
  - `publish` turns every error into `write_sequence()` (`ring.rs:753-775`).
- **Read path:**
  - `write_seq` advances before the slot commits. Readers retry 64 times (`ring.rs:795-799`), then
    move their cursor to the highest `seq` seen, so skipped sequence numbers are never reported.
- **PoC** (8 writer threads × 5,000 events, one polling reader):

```text
in-memory   8x5000: published=40000 (every call returned a seq)  observed=5141  lost=34859 (87.1%)
shared-file 4x2000: published=8000                                 observed=8000  lost=0
```

Part of the in-memory loss is plain ring overrun: 256 slots, and a reader slower than eight
writers. That is acceptable for a best-effort broadcast, **if it is signalled**. Neither the
writer nor the reader learns that anything was lost. The file backend loses nothing in this test
because writer serialization makes writers slower than the reader.

- **Test gap:** the shipped 8×8 stress test only asserts `total_observed > 0` (`ring.rs:1104`),
  so it cannot detect loss.
- **Remediation:**
  - `try_publish` returns `Err(Lapped | Contended)` and `publish` surfaces a counter.
  - `poll_since` returns `(events, gaps)` so consumers can fall back to the CAS/DAG.
  - Add a stress test that asserts `observed + reported_gaps == published`.

## ICX-AUD-055 — Cross-agent propagation latency is not measured cross-process (Low, claims)

The "4-agent < 100 µs telepathy" test (`crates/icx-adapters/src/lib.rs`, around lines 2360-2420)
uses **one thread and four handles on one file**, and times only `poll` (best of 3). It does not
measure propagation between processes, wake-up latency, or `flock` contention.

- **Remediation:** run separate writer and reader processes with a timestamp in the payload, and
  report the p50/p99 publish-to-observe latency.

## ICX-AUD-019 — Bus file open hardening (Low)

`open_shared_file` (`ring.rs:332-352`):

- has no `O_NOFOLLOW` and does not check that the file is owned by the effective uid;
- silently changes an *empty* group/world-accessible file to mode 0600;
- re-initializes the header when the first 8 bytes are zero (`ring.rs:376`).

Mitigations on a typical Linux host:

- `fs.protected_symlinks=1` and `fs.protected_regular=1` block the classic sticky-`/dev/shm`
  symlink or pre-creation attack across users.
- A non-empty file with the wrong magic is rejected (`InvalidMagic`), so an existing file is not
  overwritten unless it starts with 8 zero bytes.

Remediation: open with `O_NOFOLLOW | O_CLOEXEC`, `fstat` and require `uid == geteuid()` and
mode `0600`, and refuse rather than chmod.

## Memory-ordering review (in-process backend): no defect found

- **Writer:** `fetch_add` on `write_seq`, CAS the slot to odd (`AcqRel`), relaxed data stores,
  fence, then `store(even, Release)`.
- **Reader:** `load(Acquire)`, relaxed data loads, `fence(Acquire)`, reload, compare.

This matches the standard seqlock construction (Boehm, "Can Seqlocks Get Along with Programming
Language Memory Models?", MSPC 2012; Bos, *Rust Atomics and Locks*, ch. 9). Because the payload is
read with relaxed atomics rather than plain loads, the Rust data-race rules are respected. The
`SharedFile` reader's `s1 == slot == s2` check relies on `pwrite` ordering through the page
cache, which holds in practice on Linux but is not guaranteed by POSIX. Document this assumption.
