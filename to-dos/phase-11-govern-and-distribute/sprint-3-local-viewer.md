# Sprint 3 — Local Viewer

**Phase:** Phase 11 — Govern and Distribute  
**Release:** v1.9.0  
**Sprint Goal:** Ship a read-only local web viewer for stubs, the timeline, the policy and the
savings ledger, served on loopback behind a token, replacing the Phase 8 optional dashboard.  
**Estimated Duration:** 1.5 weeks

## Research gate

Before any ticket starts:

- Re-read the Phase 8 dashboard and `icx stats` implementation, and decide what moves into the
  viewer and what is retired.
- Check the current HTTP server crate already used by `icx-proxy` (or the candidate) via Context7,
  to avoid a second server stack.
- Review local-web-server attacks: DNS rebinding, cross-origin requests from a browser, token
  leakage through referrers or logs.
- Confirm the Phase 10 savings-ledger format the savings view reads.

### Research notes

_None recorded yet._

## Tickets

### T-11-3-001 — Loopback-only viewer server with a launch token

**Description:** `icx view` starts an HTTP server bound to `127.0.0.1`/`::1` only, with a random
per-launch token required on every request, a strict `Host` header check, and no write endpoints.

**Research:** Confirm the server crate's binding and header APIs, and test a DNS-rebinding request
against the prototype before finalising.

**Acceptance Criteria:**

- [ ] Refuses non-loopback binds, missing or wrong tokens, and foreign `Host` headers (tested).
- [ ] No endpoint mutates state (tested by method and route inventory).
- [ ] The token never appears in logs.
- [ ] Threat model updated in `docs/security-and-threat-model.md`.

**Dependencies:** T-11-1-002  
**Reference:** Plan Phase 11 Sprint 3; `docs/security-and-threat-model.md`  
**Estimated Complexity:** M

---

### T-11-3-002 — Views: stubs, timeline, policy and savings

**Description:** Pages for recent stubs with deref, the timeline around a handle, the active policy
file, and the savings ledger. All reads go through the policy layer and the redactor.

**Research:** Confirm the timeline op output (T-11-2-001) and the savings-ledger schema from
Phase 10 Sprint 4 are stable enough to render.

**Acceptance Criteria:**

- [ ] Each view renders from real data in an integration test.
- [ ] Denied and `<private>` content never renders (tested).
- [ ] Pages work without external network resources (no CDN).

**Dependencies:** T-11-3-001, T-11-2-001, T-10-4-001  
**Reference:** Plan Phase 11 Sprint 3  
**Estimated Complexity:** M

---

### T-11-3-003 — Absorb the Phase 8 optional dashboard

**Description:** Move the Phase 8 dashboard's content into the viewer and remove the separate
dashboard, keeping one local UI.

**Research:** List what the Phase 8 dashboard shows and who relies on it; confirm nothing is lost.

**Acceptance Criteria:**

- [ ] Every Phase 8 dashboard metric is available in the viewer.
- [ ] The old dashboard entry point is removed or redirects, with a `CHANGELOG.md` note.

**Dependencies:** T-11-3-002, T-83-004  
**Reference:** Plan Phase 11 Sprint 3 ("absorbs the Phase 8 optional dashboard")  
**Estimated Complexity:** S

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Coverage >= 95% (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Docs and `CHANGELOG.md` (`[Unreleased]`) updated in the same change.
- [ ] Upstream re-scan for Sprint 4 done, its tickets adjusted, and findings recorded in its
  Research notes.
