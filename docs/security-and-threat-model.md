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
| **Information Disclosure** | `.env` file or `sk-...` API key read during a session gets cached in CAS or broadcast over `/dev/shm`. | Mandatory **Pre-Ingestion `SecretRedactor`** (`crates/icx-codec/src/redact.rs`) strips API keys, PEM blocks, bearer tokens, and secret assignments *before* `CasStore::put_bytes()` or `SeqlockRing::publish()`. |
| **Denial of Service** | A runaway command outputs 2 GB of stdout into `PostToolUse` stdin. | Bounded streaming ingestion cap (`ICX_MAX_INGEST_BYTES = 64 MiB` per single tool event) with ring-buffer head/tail truncation beyond the cap. |
| **Elevation of Privilege (Prompt Injection)** | Untrusted tool output contains `⟪ICX:v1...⟫` or `⟦Ω:INVARIANT...⟧` delimiter spoofing to forge a system invariant. | **Delimiter Escaping at Ingestion:** Any raw input containing literal `⟪`, `⟫`, `⟦`, or `⟧` characters has those delimiters escaped to `\<` / `\>` prior to skeletonization so untrusted data can never forge a top-level `AHTL` packet header or `Ω` invariant! |

---

## 3. Unsafe Code & Memory Safety Policy

- `crates/icx-core`, `crates/icx-codec`, `crates/icx-hdc`, `crates/icx-store`, `crates/icx-adapters`, `crates/icx-proxy`, and `crates/icx-cli` forbid `unsafe` (`#[forbid(unsafe_code)]`).
- `crates/icx-bus` confines `unsafe` strictly to the POSIX `mmap` shared-memory ring buffer (`SeqlockRing`), with every `unsafe` block accompanied by an explicit `// SAFETY:` invariant proof verifying alignment, bounds, and atomic seqlock memory ordering.
