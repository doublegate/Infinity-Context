# Sprint 8 — Format Break

**Phase:** Phase 12 — Architecture Release  
**Release:** v2.0.0  
**Sprint Goal:** Make the deliberate v2.0 format break: full 256-bit internal keys, a keyed MAC on
emitted stubs, an optional encrypted raw tier, versioned policy file and savings ledger, and a
one-way migration tool documented in ADR-0008.  
**Estimated Duration:** 2.5 weeks

## Research gate

Before any ticket starts:

- Re-read `docs/claude/11-enhancements-and-optimizations.md` A3 (keyed stub MAC) and A5 (encrypted raw tier), and the current key
  truncation in `crates/icx-core` and the stub format in `crates/icx-codec`.
- Check current RustCrypto `blake3` keyed mode, AEAD crate (for example `chacha20poly1305`) and
  key-storage options via Context7.
- Collect real v1.9 stores (redacted, consented) as migration fixtures.
- Re-read the Phase 11 policy file and Phase 10 savings-ledger formats.

### Research notes

_None recorded yet._

## Tickets

### T-12-8-001 — ADR-0008: the v2.0 format break and one-way upgrade

**Description:** Write ADR-0008 (Context, Decision, Consequences) covering the key width, stub
MAC, encrypted tier, policy and ledger versioning, and the deliberate one-way upgrade.

**Research:** List every on-disk and on-wire format affected.

**Acceptance Criteria:**

- [ ] `docs/adr/0008-*.md` committed and linked from ADR-0005 and `docs/data-model.md`.

**Dependencies:** None  
**Reference:** Plan Phase 12 Sprint 8; module 40 (ADRs)  
**Estimated Complexity:** S

---

### T-12-8-002 — Full 256-bit internal keys

**Description:** Use the full 256-bit BLAKE3 digest as the internal key everywhere a truncated
key is used today; stub handles may stay short for display but resolve against full keys.

**Research:** Inventory every truncated-key site and its collision guard.

**Acceptance Criteria:**

- [ ] No internal lookup uses a truncated key (tested by an inventory test).
- [ ] Short-handle collisions resolve deterministically or error (tested).

**Dependencies:** T-12-8-001  
**Reference:** Plan Phase 12 Sprint 8; `crates/icx-core/src/cas.rs`  
**Estimated Complexity:** L

---

### T-12-8-003 — Keyed MAC on emitted stubs

**Description:** Attach a keyed MAC (BLAKE3 keyed mode) to every emitted stub so `icx` can reject
forged or altered stubs pasted back into context.

**Research:** Decide key storage and rotation, and confirm the added stub size is acceptable.

**Acceptance Criteria:**

- [ ] Forged or altered stubs are rejected on deref (tested).
- [ ] Stub size change measured and published in `docs/numbers.md`.

**Dependencies:** T-12-8-001  
**Reference:** Plan Phase 12 Sprint 8; `docs/claude/11-enhancements-and-optimizations.md` A3  
**Estimated Complexity:** M

---

### T-12-8-004 — Optional encrypted raw tier

**Description:** Encrypt raw CAS content at rest with an AEAD, opt-in, with the key outside the
store.

**Research:** Confirm the AEAD crate and nonce scheme, and the performance cost on recall.

**Acceptance Criteria:**

- [ ] Encrypted store round-trips; tampering is detected (tested).
- [ ] Recall cost measured and published in `docs/numbers.md`.
- [ ] Off by default; default build unchanged.

**Dependencies:** T-12-8-001  
**Reference:** Plan Phase 12 Sprint 8; `docs/claude/11-enhancements-and-optimizations.md` A5  
**Estimated Complexity:** L

---

### T-12-8-005 — Version the policy file and the savings ledger

**Description:** Bump and enforce format versions for `.icx/policy.toml` and the savings ledger,
with readers that refuse unknown versions cleanly.

**Research:** Confirm the current version fields from Phase 10 and Phase 11.

**Acceptance Criteria:**

- [ ] Both formats carry a version; unknown versions are refused with a clear error (tested).

**Dependencies:** T-12-8-001, T-11-1-002, T-10-4-001  
**Reference:** Phase 12 overview ("The format break also versions the policy file and the savings
ledger")  
**Estimated Complexity:** S

---

### T-12-8-006 — One-way migration tool

**Description:** `icx migrate` upgrades a v1.9 store to v2.0: dry-run by default, a mandatory
backup step, an audit entry, and a refusal to run twice.

**Research:** Run the tool against the collected real v1.9 fixtures.

**Acceptance Criteria:**

- [ ] Every fixture migrates; deref of every pre-migration handle succeeds afterwards (tested).
- [ ] Backup is written and verified before any change.
- [ ] Upgrade note in `CHANGELOG.md` and the docs site.

**Dependencies:** T-12-8-002, T-12-8-003, T-12-8-005  
**Reference:** Plan Phase 12 Sprint 8; ADR-0008  
**Estimated Complexity:** L

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Coverage >= 95% (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Docs and `CHANGELOG.md` (`[Unreleased]`) updated in the same change.
- [ ] Upstream re-scan for Sprint 9 done, its tickets adjusted, and findings recorded in its
  Research notes.
