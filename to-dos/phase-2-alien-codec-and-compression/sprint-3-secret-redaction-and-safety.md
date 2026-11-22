# Sprint 3 — Pre-Ingestion Secret Redactor & Delimiter Escaping

**Phase:** Phase 2 — Alien Holographic Token Lattice (`AHTL`)  
**Sprint Goal:** Implement `SecretRedactor` and `AHTL` delimiter-injection neutralization in `crates/icx-codec/src/redact.rs`.  
**Estimated Duration:** 1 week

## Tickets

### T-23-001 — Implement Pre-Ingestion `SecretRedactor` & Delimiter Escaping Gate (COMPLETED)

**Description:** Implement the 4-Tier pre-ingestion `SecretRedactor` and `AHTL` delimiter-injection neutralization gate in `crates/icx-codec/src/redact.rs` (`Tier 0` delimiter escaping `⟪`, `⟫`, `⟦`, `⟧`, `Tier 1` multi-line PEM/PGP private key stripping, `Tier 2` 28 vendor prefixes including `sk-ant-`, `sk-proj-`, `sk-svcacct-`, `sk_live_`, `whsec_`, `ghp_`, `github_pat_`, `aim_`, `AKIA`, `ASIA`, `AIza`, `hf_`, `npm_`, `pypi-`, `xox[bparso]-`, 3-segment JWT tokens, database/broker URL passwords, CLI secret flags, and `.env`/JSON/YAML assignments, and `Tier 3` Shannon character entropy $H(S)$ filter for unprefixed Base64/hex secrets) plus `FLAG_IS_REDACTED` (`1 << 7`) hologram byte tagging in `InfinityContextMmu::ingest_payload`.

**Acceptance Criteria:**

- [x] Zero secret values survive `SecretRedactor::sanitize` across all 4 tiers (`Tier 0`..`Tier 3`), including multi-key single-line JSON, escaped quotes, and connection URLs.
- [x] Untrusted text containing `⟦Ω:INVARIANT⊢...⟧` or `⟪ICX:v1│...⟫` cannot forge an AHTL packet or epoch header.
- [x] `SecretRedactor::shannon_entropy` computes exact character Shannon entropy $H(S)$ in bits/char and redacts unprefixed Base64 ($H(S) > 4.35$, $|S| \ge 24$) and hex ($H(S) > 3.50$, $|S| \ge 32$) secrets without false-positiving on source code identifiers, file paths, or `#icx:` handles.
- [x] `InfinityContextMmu::ingest_payload` sets `FLAG_IS_REDACTED` (`1 << 7`) on `hologram_bytes[7]` whenever pre-ingestion sanitization modifies the input payload.
- [x] Unit and end-to-end MMU tests cover all secret families and prompt-injection delimiter attacks with 0% secret leakage across CAS, DAG, HDC, Graph, and Bus.

**Dependencies:** T-21-002  
**Reference:** `docs/security-and-threat-model.md` §2, `docs/ahtl/05-structural-ast-skeletonization-and-redaction.md` §3  
**Estimated Complexity:** M
