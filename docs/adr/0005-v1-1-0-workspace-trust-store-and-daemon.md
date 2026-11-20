# ADR 0005: Private Sealed Workspaces, a Single SQLite Store, a Resident Daemon, and Honest Bus Semantics (v1.1.0)

## Status

Accepted (2026-11-18). Supersedes in part ADR-0004 (the "lock-free, zero-copy `/dev/shm`" description of the file-backed bus and its latency figures).

## Context

The v1.0.0 audit (`docs/claude/`) found four structural problems:

1. **Repository-local trust (ICX-AUD-010, ICX-AUD-013).** The default workspace was `.icx/` inside
   the repository, created with mode `0755` and not git-ignored. A cloned repository could ship a
   populated `.icx/`, and `icx` would load it and inject its content into agent context.
2. **Persistence (ICX-AUD-001, ICX-AUD-047).** State was spread over three JSON files
   (`cas_store.json`, `mmu_state.json`, `bitemporal_graph.json`) that were rewritten in full and
   could not be replaced atomically as a set. Every hook reloaded them and re-encoded the HDC
   index, so hook cost grew with workspace size (940 ms at 338 artifacts).
3. **Per-hook process cost.** Even with faster persistence, a short-lived hook process must load
   the workspace on every call.
4. **Bus claims (ICX-AUD-046, ICX-AUD-054, ICX-AUD-055).** ADR-0004 and the specification described
   a lock-free, memory-mapped, zero-copy `/dev/shm` ring. The cross-process backend uses positional
   file I/O under a writer mutex and file lock on a file in the workspace, and dropped events were
   reported as successful publishes.

## Decision

1. **Per-user, sealed workspaces outside the repository.** The default workspace is
   `<state_dir>/workspaces/<first 16 hex of blake3(canonical project root)>`, with `state_dir` from
   `ICX_STATE_HOME`, `$XDG_STATE_HOME/icx` or `~/.local/state/icx`. Every workspace (default,
   `--dir`, `ICX_DIR`) must be owned by the effective uid, have no group/other permission bits, and
   carry an `OWNER` seal: a keyed BLAKE3 MAC of its canonical path under a per-install key
   (`$XDG_CONFIG_HOME/icx/install.key`, `0600`). A repository-local `.icx/` is never loaded
   implicitly; `icx trust --dir <path>` adopts an existing directory explicitly. No shared-directory
   fallback is used when no private root can be determined.
2. **A single SQLite store with a MAC.** All workspace state lives in one `icx.sqlite3` file (WAL,
   `0600`): append-only CAS chunk and artifact rows, plus `meta` rows for derived state, including
   a binary HDC snapshot (`ICXHDC03`). Every save is one transaction and increments a
   `generation`. Every save writes a store seal, a keyed BLAKE3 MAC over the canonical path, the
   generation and a digest of all persisted state; a sealed load rejects a store whose MAC does not
   verify. v1.0.x JSON workspaces are migrated on first load and left in place.
3. **A resident daemon.** `icx daemon` keeps one workspace in memory and serves `icx hook`
   requests over a Unix socket in `$XDG_RUNTIME_DIR/icx` (`0700`), accepting only peers with the
   same uid (`SO_PEERCRED`). It replies first and flushes after 50 ms without requests (at most
   1 s), holding the workspace lock while it has unsaved changes, so it is the single writer.
   `icx hook` uses the daemon when one is running and falls back to in-process handling otherwise.
4. **Honest bus semantics.** The specification describes two backends: an in-process atomic
   seqlock ring, and a file-backed ring using `pread`/`pwrite` under a writer mutex and advisory file
   lock with optimistic lock-free readers. It is not memory-mapped and not zero-copy, and it is on
   `/dev/shm` only if `ICX_SHM_PATH` points there. Publishing can fail (`SlotLapped`,
   `SlotContended`); `publish` counts drops (`dropped_publishes`), and readers can learn how many
   events they missed (`poll_since_with_loss`). The bus file is opened with `O_NOFOLLOW |
   O_CLOEXEC` and refused if not a private regular file owned by the user. `unsafe_code = "forbid"`
   is kept, which rules out `mmap`.

## Consequences

- **Positive:** A cloned repository can no longer supply trusted context, and other local users
  cannot read workspaces. Saves are atomic; editing any persisted row of a sealed store is
  detected. Measured hook cost: 338 artifacts 940 ms to 14 ms in-process; about 2,000 artifacts
  69 to 72 ms in-process and a median of 3.1 ms through the daemon. Bus documentation matches the
  code, and event loss is observable.
- **Negative:** Workspaces are no longer visible next to the project, and a copied or moved
  workspace must be re-trusted because the seal binds the canonical path. Existing v1.0.x `.icx/`
  directories need an explicit `icx trust`. The daemon is an additional long-lived process, and a
  request that overlaps a flush takes about 19 ms. File-backed bus writers can wait on the file
  lock (up to about 250 ms before failing), and measured file-backed propagation is tens of
  microseconds (worst 38 µs over 200 events), not the sub-microsecond figures in ADR-0004.
- **Unchanged limits:** Same-user processes are trusted; they can read the install key and forge
  seals. See [docs/security-and-threat-model.md](../security-and-threat-model.md) §5.
