# Security Policy

## Architectural Security Invariants

1. **Mandatory Pre-Ingestion Secret Redaction:** All inputs passing into `Infinity-Context` are sanitized by `SecretRedactor` (`crates/icx-codec/src/redact.rs`) before BLAKE3 chunking or `/dev/shm` publication.
2. **Cryptographic Integrity Verification:** Every Content-Addressed Storage (`CAS`) read recomputes the 256-bit `BLAKE3` digest and fails closed on any byte mismatch.
3. **AHTL Delimiter Escaping:** Untrusted inputs cannot inject unescaped `⟪`, `⟫`, `⟦`, or `⟧` delimiters to forge system invariants.
4. **Loopback / UID Isolation:** Wire proxy and shared-memory segments are restricted to the local user UID (`0600` permissions).

See [docs/security-and-threat-model.md](docs/security-and-threat-model.md) for the complete STRIDE threat model.
