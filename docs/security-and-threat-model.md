# Security & Threat Model Specification (STRIDE)

**References:** [ref-docs/research-report.md](../ref-docs/research-report.md) §6.4, Master-Core Module 60 (Security & Input Validation)

---

## 1. Trust Boundaries

1. **Untrusted Tool Output Boundary:** External files, web fetches, `git log` messages, and terminal command outputs ingested via `PostToolUse` may contain secrets (API keys, private keys, `.env` values) or indirect prompt-injection payloads.
2. **Cross-Agent Shared Memory Boundary (`/dev/shm/icx-fabric-v1`):** Multiple agents running under the user's UID attach to the shared memory bus. Workspace scoping (`workspace_hash`) prevents unrelated repositories (e.g., a public OSS project vs. a private commercial project) from bleeding context unless explicitly cross-linked.
3. **Wire Proxy Boundary (`127.0.0.1:49380`):** Handles upstream bearer tokens (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`). Must bind strictly to loopback (`127.0.0.1` / `::1`) or a `0600` Unix Domain Socket and never log authorization headers.

---

## 2. STRIDE Threat Matrix & Mitigations

| Threat Category | Concrete Attack Vector | Mitigation in `Infinity-Context` |
| :--- | :--- | :--- |
| **Spoofing** | External process on multi-user host connects to `/dev/shm` or UDS socket. | Shared memory file and UDS socket are created under `$XDG_RUNTIME_DIR/icx` (`chmod 0700`, file mode `0600`) with `SO_PEERCRED` UID verification matching `geteuid()`. |
| **Tampering** | Corrupted or mutated cold storage chunk alters dereferenced source code. | Every `L0` chunk and `L3` epoch is addressed by its 256-bit cryptographic `BLAKE3` digest. `CasStore::deref_bytes()` recomputes `blake3::hash(&bytes)` on every read and fails closed on mismatch (`IcxError::IntegrityViolation`). |
| **Repudiation** | Unknown which agent or tool call introduced a poisoned invariant into the Bi-Temporal Graph. | Every `BiTemporalEdge` and `AhtlPacket` stamps `AgentKind`, `session_id`, `t_tx_start`, and the immutable `provenance: ArtifactId` (`#icx:<hash>`). |
| **Information Disclosure** | `.env` file, `sk-...` API key, JWT, DB connection URL, or high-entropy unprefixed token read during a session gets cached in CAS or broadcast over `/dev/shm`. | Mandatory **4-Tier Pre-Ingestion `SecretRedactor`** (`crates/icx-codec/src/redact.rs`) strips PEM/PGP private key blocks (`Tier 1`), 28 vendor prefixes, 3-segment JWTs, database/broker URL passwords, CLI secret flags, and `.env`/JSON/YAML secret assignments (`Tier 2`), plus unprefixed high-entropy Base64/hex tokens via Shannon character entropy $H(S)$ (`Tier 3`) *before* `CasStore::put_bytes()`, `HdcAssociativeIndex`, or `SeqlockTelemetryBus::publish()`, and sets `FLAG_IS_REDACTED` (`1 << 7`) on `ChannelBHologram` byte `B[7]`. |
| **Denial of Service** | A runaway command outputs 2 GB of stdout into `PostToolUse` stdin. | Bounded streaming ingestion cap (`ICX_MAX_INGEST_BYTES = 64 MiB` per single tool event) enforced by `CasStore::put_bytes_checked`. |
| **Elevation of Privilege (Prompt Injection)** | Untrusted tool output contains `⟪ICX:v1...⟫` or `⟦Ω:INVARIANT...⟧` delimiter spoofing to forge a system invariant. | **Delimiter Escaping at Ingestion (`Tier 0`):** `SecretRedactor::escape_ahtl_delimiters` escapes literal `⟪` (`<<`), `⟫` (`>>`), `⟦` (`[[`), and `⟧` (`]]`) characters prior to skeletonization so untrusted data can never forge a top-level `AHTL` packet header or `Ω` invariant. |

---

## 3. Unsafe Code & Memory Safety Policy

- **Permanent Workspace-Wide `#[forbid(unsafe_code)]` (`std` Allowed, Zero `unsafe`):** All 8 workspace crates (`crates/icx-core`, `crates/icx-codec`, `crates/icx-hdc`, `crates/icx-store`, `crates/icx-bus`, `crates/icx-adapters`, `crates/icx-proxy`, and `crates/icx-cli`) inherit `[workspace.lints]` with `unsafe_code = "forbid"` permanently enforced through `v1.0.0` and beyond.
- `crates/icx-bus` implements `FabricHeader` (64 B) and `SeqlockSlot` (4,096 B) using 100% safe `AtomicU32` and `AtomicU64` word arrays and safe `/dev/shm` positional file I/O (`std::os::unix::fs::FileExt::{read_exact_at, write_all_at}` and OS-managed `File::try_lock()`), eliminating all `unsafe` blocks and raw pointer operations.
