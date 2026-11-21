# 03 — Integrity, persistence, workspace and daemon security

Audited revision: `a996d59f1eb468e7c386747e15f4c7e58d08410e`. This is a source review and isolated experiment report, not a production patch. The source tree and live user stores were not modified.

## Result

The current SQLite transaction, CAS verification, workspace location, install-key creation and keyed seal mechanisms materially improve the v1.0.0 design. A successful daemon hook response still does **not** establish durable storage. Treat crash recovery, transaction atomicity, and durability as three separate properties.

## ICX-CODEX-030 — Acknowledged daemon artifacts can disappear before persistence

| Field | Assessment |
| --- | --- |
| Category / severity / confidence | Durability limitation and failure reporting; Medium; High |
| Location | `crates/icx-cli/src/daemon.rs:155`, `:169`, `:233`, `:245`, `:319`; `crates/icx-adapters/src/store.rs:169` |
| Prerequisites | Resident daemon enabled; process death during write-behind, or a flush failure; no attacker required |
| Impact | An agent can receive a successful replacement containing a CAS handle that is absent after restart. Five failed flushes clear dirty tracking; a competing generation can replace unsaved state; final-flush failure is logged while `serve` returns its prior result, potentially `Ok(())`. |
| Reproduction | `evidence/state-probe.rs`, `daemon_ack_before_flush_crash`: the actual daemon source serves a synthetic ingest, parent reads `ok:true`, immediately kills only that isolated child, then reloads its temporary SQLite workspace. The artifact is absent. `daemon_final_flush_failure` additionally makes the isolated database path temporarily unopenable after ingest: response remains `ok:true`, the daemon exits successfully despite logging final-flush failure, and the acknowledged artifact is absent from the preserved pre-save database. See `state-probe-results.jsonl`. |
| Source proof | `handle_connection` writes the reply before `serve` starts/updates the dirty window. On retry the lock is released; `flush` can reload another writer's newer generation. After five failures the loop sets `first_dirty = None`. Final flush does not propagate its error into `result`. |
| Counterevidence | Write-behind is explicitly documented in `docs/architecture.md:140` and ADR-0005. Ordinary SQLite saves are atomic, and flush retries retain resident state when the generation is unchanged. This finding does not allege a mixed-generation transaction or make a documented asynchronous design a synchronous contract. |
| Recommendation | Define durable versus resident acknowledgment explicitly. Offer a durable-ack mode or journal the accepted operation before returning a handle. Propagate terminal flush failure; keep unsaved state quarantined/recoverable rather than silently treating it as clean. |
| Acceptance | Repeated kill-before/after-ack tests must recover every durable-acknowledged artifact. Inject a save failure and a competing writer; assert no acknowledged operation is silently discarded, and terminal failure is observable by supervision. |
| Compatibility | An additive durable mode and acknowledgment metadata preserve existing fast mode; changing the default affects latency. Journal format and replay idempotency need versioning. |
| Related prior IDs | ICX-AUD-001, ICX-AUD-047 (atomic save repaired; acknowledgment durability remains separate) |

SQLite is opened in WAL mode with `synchronous=NORMAL`. SQLite documents that this retains consistency while allowing recent committed transactions to be lost on power failure. No power-cut experiment was performed; do not extrapolate the process-kill reproduction to hardware power-loss safety. Primary reference: [SQLite synchronous pragma](https://sqlite.org/pragma.html#pragma_synchronous), accessed 2026-11-21.

## ICX-CODEX-033 — Extreme public chunker configuration panics

| Field | Assessment |
| --- | --- |
| Category / severity / confidence | Public API robustness; Low; High |
| Location | `crates/icx-core/src/chunker.rs:75` (`min_size + 64`) |
| Prerequisites | Caller controls `ChunkerConfig.min_size`, or an unsealed library snapshot supplies such a configuration; default CLI configuration is safe |
| Impact | `min_size = usize::MAX` panics on integer addition in checked builds. The infallible API has no configuration validation boundary. This is not established as a remote or cloned-repository exploit of the sealed CLI. |
| Reproduction | `chunk_config_extreme_min` in `state-probe.rs` catches the panic while processing an eight-byte payload. |
| Counterevidence | Default min/avg/max are 2,048/8,192/65,536. Sealed stores require a MAC; same-user processes can already read the key and are outside the threat model. Input payload bounds alone do not validate configuration arithmetic. |
| Recommendation | Validate min/avg/max in a checked constructor; use checked/saturating arithmetic and define an upper bound before calculating pivots. |
| Acceptance | Boundary vectors covering `0`, `usize::MAX-63`, `usize::MAX` and inverted ranges produce a typed error or documented bounded normalization without panic in debug and release. |
| Compatibility | Add a checked constructor while retaining a documented default constructor; returning `Result` from the existing function is an API change. |
| Related prior IDs | None; not the repaired spread-mask issue ICX-AUD-027 |

## Independent control review and rejected leads

| Component | Evidence / disposition |
| --- | --- |
| Checked CAS ingest | Full digest is compared on artifact and chunk ID hits; precomputed chunk candidates are checked before mutation. MMU uses the checked path and does not fall back to unchecked insertion on collision. |
| CAS load / line metadata | `CasStore::from_parts` rehashes chunks, verifies artifact IDs/digests, reconstructs all bytes, recomputes line spans, byte offsets, byte lengths and total lines, and applies the 64 MiB bound. A proposed metadata-only MAC bypass is rejected: metadata is constrained by these checks. |
| `put_bytes` | Remains a documented compatibility API that truncates above 64 MiB; callers needing fail-closed size/collision behavior must use `put_bytes_checked`. Do not describe the old public API as universally repaired. |
| Exact recovery | Four independent MMU fixtures cover CRLF, trailing LF, literal packet delimiters and multilingual text. Byte equality is checked through CAS, rather than through model-facing escaped output. Line-slice APIs deliberately return textual lines and normalize CRLF; they are not arbitrary byte-range recovery APIs. |
| SQLite save | `store::save` writes CAS and derived state in one transaction, checks the expected generation, increments it only on success, and updates persisted-ID caches after commit. No SQL injection path was found in these parameterized operations. The internal table-name interpolation is a fixed programmer-controlled set. |
| Seals | Store MAC covers derived JSON/snapshot/configuration, generation, canonical path, artifact identity and emitted-block digests. CAS hashes transitively validate payload bytes. Owner seals bind canonical path. These are same-install integrity checks, not user-to-user authorization tokens. |
| Install key | Metadata checked before reading, regular/private/owned file required; random temporary contents synced before atomic hard-link publication. The prior first-use torn-key race has explicit concurrent tests. |
| Workspace roots | Absolute roots required, private workspace outside the repository by default. Existing unsealed directories are refused. An explicitly adopted repository directory remains an owner-authorized trust decision. |
| Unsealed/legacy DAG | CAS is checked, but `apply_loaded` does not call `verify_all_merkle_integrity_with_cas`; sealed derived state is MAC protected. Legacy adoption and unsealed library use have weaker semantic validation. `doctor` provides additional DAG verification; do not equate successful deserialization with full DAG validation. |
| Slow local client | Daemon is single-threaded and holds the workspace lock while handling a connection; 10-second socket timeouts limit inactivity, not a complete request deadline. Same-user clients are trusted by policy. Absolute deadlines remain a hardening opportunity, not a demonstrated cross-user bypass. |
| Filesystem races | Path metadata checks do not constitute directory-FD-based race freedom. No hostile other-user race through a securely private directory was demonstrated. Same-user replacement is outside the documented boundary. |

## Prior-audit reassessment (this workstream)

Statuses mean independently inspected evidence at this revision; a source-based status is not a new live exploit test.

| Prior ID | Status | Current evidence and limit |
| --- | --- | --- |
| ICX-AUD-010 | Confirmed fixed for default CLI trust path | `workspace::resolve`, `prepare`, `verify`, owner/store seals; repository `.icx` is not selected implicitly. Existing trust tests exercise replacement and modified rows. |
| ICX-AUD-013 | Confirmed fixed | Private directory/file creation and `.gitignore` rewrite; adoption tightens owned regular files. |
| ICX-AUD-014 | Partially fixed / bounded | CLI hook/MCP bounds and fold depth exist. Single-request limits are not a complete concurrency or elapsed-time budget. |
| ICX-AUD-017 | Partially fixed | Current threat model now names implemented controls and residual risks. Its blanket guarantees require the qualifications above and the bus/integration reports. |
| ICX-AUD-021 | Confirmed fixed for secret-free full-byte storage | Independent four-fixture equality probe plus separate storage `redact`/presentation escaping paths; redaction itself is intentionally lossy. |
| ICX-AUD-022 | Partially fixed | MMU checked-to-unchecked fallback removed; documented unchecked/truncating public API remains. |
| ICX-AUD-023 | Confirmed fixed for silent overwrite | DAG collision guard and checked CAS insertion reject divergent digest/node identity. No practical 64-bit collision search was attempted. |
| ICX-AUD-027 | Confirmed fixed in source | Spread high-bit masks and non-single-chunk fixture exist. Independent repository-wide deduplication benchmark was not repeated here. |
| ICX-AUD-042 | Confirmed fixed in source | Tail candidates deduplicate artifact IDs; scale/recall quality belongs to report 02. |
| ICX-AUD-047 | Partially fixed | Single transaction repairs multi-file atomicity; sealed MAC repairs untrusted derived state for CLI. Unsealed/legacy DAG load remains less validated; daemon acknowledgment durability is ICX-CODEX-030. |
| ICX-AUD-048 | Confirmed fixed as documented opt-in | `use_wall_clock` and monotonic clock implementation; deterministic library default remains intentional. |

## Coverage and limits

Deep targeted source reads: complete `store.rs`, `workspace.rs`, `mcp.rs`, `daemon.rs`; CAS ingestion, resolve, dereference, persistence and `from_parts`; chunker production implementation; MMU ingestion/load/save/locking/emission trust/clock/tail paths. DAG insertion/verification/folding and workspace tests were inspected by symbol and relevant sections. This is not a claim that every test assertion or every one of the 16,669 lines in the combined state/integration crates received the same depth.

Document reads: `docs/security-and-threat-model.md`, `docs/virtual-memory-and-dag-engine.md`, `docs/architecture.md`, ADR-0005, prior Claude integrity/security/bus/integration reports and remediation entries. Data-model/interfaces/deployment and agent forensic references were cross-searched at relevant fields; full line-by-line review of their entire text is not claimed by this worker. Linux local behavior was exercised; macOS/Windows, hardware power failure, hostile different-UID races, and every crash point in SQLite were not exercised.
