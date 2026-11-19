# 02 — Integrity and "100% Lossless" Claims (icx-core, icx-store, icx-adapters MMU)

Workstream W2.

## ICX-AUD-021 — The store is not lossless: distinct inputs collapse to one artifact (High, claims)

- **Claim:** "100% lossless BLAKE3 Merkle-DAG" (README, `AGENTS.md` §1, `docs/virtual-memory-and-dag-engine.md`), and "lossless `#icx:<hash>` dereferencing".
- **Reality:** `InfinityContextMmu::ingest_payload_for_subagent` (`mmu.rs:569-590`) stores the output of `SecretRedactor::sanitize`, not the raw bytes. `sanitize` (`redact.rs:17-52`):
  - splits on `str::lines()` and re-joins with `\n`, which rewrites CRLF and drops the trailing newline;
  - replaces `⟪⟫⟦⟧` with `<< >> [[ ]]` (Tier 0, `redact.rs:55-66`), which cannot be inverted.
- **PoC** (pairs ingested into a fresh MMU):

```text
"a\r\nb\r\n" -> id 16222543602915972710 ; "a\nb"     -> same id ; deref = "a\nb"
"x ⟪y⟫"      -> id 1208660692214277781  ; "x <<y>>"  -> same id ; deref = "x <<y>>"
"tail\n"     -> id 10642850172088438137 ; "tail"     -> same id ; deref = "tail"
```

- **Test evidence:** one shipped test asserts this lossy behaviour (`crates/icx-adapters/src/lib.rs`, around line 2475: `assert_eq!(deref_out, panic_log.trim_end_matches('\n'))`).
- **Impact:** the store is not lossless:
  - Patches and diffs that depend on line endings, and files with significant trailing newlines, cannot be restored byte-exactly.
  - Two different tool outputs become indistinguishable.
  - The "lossless passkey recovery" benchmarks only prove that substrings survive.
- **Remediation:** store the raw bytes in an encrypted-at-rest or access-controlled tier, or store the sanitized form *and say so*. Escape delimiters reversibly (percent-escape `%`, then the delimiters), and keep line endings with `split_inclusive('\n')`. Rename the guarantee to "lossless modulo redaction" everywhere until then.
- **Regression test:** property test with `deref(ingest(x)) == x` for every `x` that contains no secrets. It covers CRLF, a lone `\r`, a trailing `\n`, the delimiter glyphs, and literal `<<`.

## ICX-AUD-022 — Over-size input is silently truncated; the collision check is bypassed (Medium)

- `CasStore::put_bytes` (`cas.rs:197-206`) silently truncates input to 64 MiB, and returns the existing artifact on an ID hit without comparing digests.
- `put_bytes_checked` (`cas.rs:130-158`) is correct: it returns `PayloadTooLarge` and checks the full 256-bit digest for collisions.
- The MMU defeats it with `.put_bytes_checked(..).unwrap_or_else(|_| self.cas.put_bytes(..))` (`mmu.rs:585-588`). Any error, a collision included, silently degrades to the unchecked path.
- `hermes.rs:165` and `mmu.rs:1046` call `put_bytes` directly.
- **PoC:**

```text
put_bytes_checked(64 MiB + 10) -> Err(PayloadTooLarge { size: 67108874, max: 67108864 })
put_bytes(64 MiB + 10)         -> stored 67108864 of 67108874 bytes (no error)
```

- **Hook path:** `bound_and_sanitize_raw` (`mmu.rs:246-279`) truncates first and sets `was_redacted = true`. The truncation is therefore *flagged* there, but still returned as a successful ingest.
- **Remediation:**
  - Remove the fallback and propagate `IcxError::HashCollision` to the caller.
  - Make `put_bytes` private, or make it return `Result`.
  - On over-size input, store a typed "truncated" marker and refuse to call the deref of a partial artifact "lossless".

## ICX-AUD-023 — 64-bit identifiers: collision posture and DAG overwrite (Low)

- **Identifier widths:**
  - `ArtifactId` is the first 8 bytes of BLAKE3 (`lib.rs:46-55`). It is the primary key of the CAS, the chunk map and the DAG.
  - Short handles are 48 bits (`lib.rs:72-80`).
  - Epoch roots in headers are truncated to 32 bits (`dag.rs:734`).
- **Random collision probability** (birthday bound p ≈ n²/2^(b+1)):

  | Stored artifacts n | 64-bit ID | 48-bit short-handle ambiguity |
| --- | --- | --- |
  | 10⁵ | 2.7×10⁻¹⁰ | 1.8×10⁻⁵ |
  | 10⁶ | 2.7×10⁻⁸ | 1.8×10⁻³ |
  | 10⁷ | 2.7×10⁻⁶ | 0.16 |

- **Adversarial cost:**
  - Forcing a collision between *two attacker-chosen* inputs costs about 2³² BLAKE3 evaluations, which is feasible.
  - Colliding with a *specific victim* artifact is a 64-bit second-preimage problem, which is infeasible.
  - Blocking a victim's 48-bit short handle needs a 48-bit prefix preimage (~2⁴⁸). The pre-audit lead "2²⁴ work" was **incorrect**: that is the birthday bound, which only yields random pairs.
- **Silent overwrite:** `MerkleContextDag` inserts with `nodes.insert(id, …)` (`dag.rs:510, 526, 584, 751, 849`), so on a 64-bit collision the DAG silently overwrites a node.
- **Remediation:** key internal maps by the full 32-byte digest and keep 64- and 48-bit forms for display and handles only, or check before inserting into the DAG. Update `docs/ahtl/06` with the adversarial-pair cost, which is missing.

## ICX-AUD-047 — Workspace persistence is not atomic and is trusted on load (Medium)

- **Not atomic:** `save_workspace_dir_unlocked` (`mmu.rs:1299-1320`) writes `cas_store.json`, `mmu_state.json` and `bitemporal_graph.json` with three *separate* temp-file + rename steps. A crash between renames leaves a mixed-generation workspace.
- **Trusted on load:** `load_workspace_dir_unlocked` re-verifies CAS digests (`cas.rs:491-570`). It does **not** verify the Merkle DAG or the epochs; only `icx doctor` calls `verify_all_merkle_integrity_with_cas`. A corrupted or hand-edited `mmu_state.json` is therefore accepted on every hook.
- **Not reproduced:** I did not run a crash-injection PoC (it needs a fault-injection seam). This finding rests on code inspection of the cited lines.
- **Remediation:**
  - Write a single generation directory (`gen-N/`) and atomically swap a `CURRENT` pointer, or use SQLite in WAL mode, which also fixes ICX-AUD-001.
  - Verify the Merkle DAG on load, at least for the epochs referenced by the tail slot.

## ICX-AUD-048 — "Bi-temporal" timestamps are synthetic (Medium, claims)

- `clock_ms` starts at the constant `1_794_880_800_000` (`mmu.rs:120`) and advances by 10 per ingest (`mmu.rs:578`). No wall clock is consulted.
- Every "valid time" and "transaction time" in `BiTemporalGraph`, along with the `τ=` and timestamp fields in bus events, is therefore a counter dressed as a time.
- The default `workspace_hash` is the constant `0x4943585F574F524B` (`mmu.rs:111`) until a workspace is loaded.
- **Remediation:** inject a `Clock` trait so the system takes wall time in production and a deterministic counter in tests (master-core determinism rule: keep wall time out of the core, inject it at the edge). Document the semantics.

## ICX-AUD-027 — Chunking: the benchmark input yields one chunk; mask choice (Low)

- **Single chunk:** the "FastCDC + BLAKE3 CAS Ingest (64K)" SLO input is one 98-byte line repeated 660 times, and it produces **1 chunk**. A 64 KiB xorshift stream produces 9 chunks. The SLO therefore measures BLAKE3 hashing plus bookkeeping, not content-defined chunking.
- **Mask choice:** the Gear-hash masks select the low bits, `(1<<(bits+1))-1` (`chunker.rs:59-60`). With `fp<<1` accumulation, low bit *k* depends only on the last *k*+1 bytes, so the effective window is about 14 bytes. FastCDC (Xia et al., USENIX ATC '16) uses spread masks over high bits specifically to avoid this. Expect worse boundary stability than FastCDC under insertions.
- **Remediation:** use the published FastCDC masks, or the `fastcdc` crate. Benchmark dedup ratio under 1-byte insertions (the property CDC exists for) on real repositories.

## ICX-AUD-042 — The tail slot repeats the same artifact (Low, functional)

Three PostToolUse events with identical content produced a SessionStart tail slot that lists the same `#icx:` handle three times, each with a different `suffix=` value. Recall does not deduplicate by `ArtifactId`, so the slot's token budget goes on duplicates.

- **Fix:** deduplicate recall candidates by `ArtifactId` before ranking.

## Positive results

- **Integrity checks:** `deref_bytes` verifies chunk and artifact digests, and `load_from_dir` re-verifies the CAS. Ambiguous 48-bit prefixes fail closed.
- **No panics:** no panic was found on the CAS or Braille parse paths.
