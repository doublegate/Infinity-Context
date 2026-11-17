# Sprint 3 — Pre-Ingestion Secret Redactor & Delimiter Escaping

**Phase:** Phase 2 — Alien Holographic Token Lattice (`AHTL`)  
**Sprint Goal:** Implement `SecretRedactor` and `AHTL` delimiter-injection neutralization in `crates/icx-codec/src/redact.rs`.  
**Estimated Duration:** 1 week

## Tickets

### T-23-001 — Implement Pre-Ingestion `SecretRedactor` & Delimiter Escaping Gate

**Description:** Scan raw inputs for API keys (`sk-`, `ghp_`, `github_pat_`, `aim_`, `AKIA`), PEM private keys, bearer tokens, and `.env` assignments (`*_SECRET=`, `*_API_KEY=`, `*_PASSWORD=`), replacing secret values with `[REDACTED:<kind>]`, and escape any untrusted `⟪`, `⟫`, `⟦`, `⟧` delimiters.

**Acceptance Criteria:**
- [x] Zero secret values survive `SecretRedactor::sanitize`.
- [x] Untrusted text containing `⟦Ω:INVARIANT⊢...⟧` cannot forge an AHTL packet header.
- [x] Unit tests cover all secret families and prompt-injection delimiter attacks.

**Dependencies:** T-21-002  
**Reference:** `docs/security-and-threat-model.md` §2  
**Estimated Complexity:** M
