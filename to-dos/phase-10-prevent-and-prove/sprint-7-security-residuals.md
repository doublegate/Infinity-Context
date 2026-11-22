# Sprint 7 — Security Residuals

**Phase:** Phase 10 — Prevent and Prove  
**Release:** v1.8.0  
**Sprint Goal:** Close or reduce the residual risks listed in the threat model (§5 items 3, 6, 7,
8 and 9) and tighten the dependency policy.  
**Estimated Duration:** 2 weeks

## Research gate

Re-check before any ticket starts:

- **External docs and versions:** current `rustls`, `hyper`/`h2` (or the chosen client),
  `rustls-native-certs`/`webpki-roots`, `keyring` and a MAC primitive (BLAKE3 keyed mode)
  (Context7); RustSec advisories for each; `cargo-deny` `[bans]` options.
- **Installed agent behaviour:** whether any installed agent connects to the proxy remotely or
  through a non-loopback address today, and how keyring prompts would appear on each platform.
- **Project evidence:** `docs/security-and-threat-model.md` §5; the curl upstream path and its
  header temp files; the 64 MiB pass-through bound; the bus event format; the current `deny.toml`
  (`[bans]` with `multiple-versions = "warn"`) and the duplicate-version list from
  `cargo deny check bans`.

### Research notes

_None recorded yet._

## Tickets

### T-10-7-001 — Native rustls upstream client (residual #7)

**Description:** Replace the `curl` subprocess and its header temp files with a native rustls
HTTP client, so provider API keys never touch disk.

**Research:** Confirm the client supports HTTP/2 (T-82-003), streaming and cancellation, and that
it builds without `unsafe` in the workspace. Re-run the fake-provider suite against it.

**Acceptance Criteria:**

- [ ] No provider key is written to any file (test that watches the temp directory during a run).
- [ ] All fake-provider tests pass on the new client.
- [ ] `curl` is no longer required by `icx doctor`.
- [ ] Threat model §5 item 7 is updated.

**Dependencies:** T-82-001, T-82-003  
**Reference:** `docs/security-and-threat-model.md` §5 item 7  
**Estimated Complexity:** L

---

### T-10-7-002 — Streaming redaction above 64 MiB (residual #6)

**Description:** Redact hook input above 64 MiB in a streaming pass instead of passing it through
unmodified.

**Research:** Confirm which detectors need look-behind across chunk boundaries and size the overlap
window; coordinate with the `Detector` trait (T-10-6-004).

**Acceptance Criteria:**

- [ ] Streaming output equals whole-buffer redaction on inputs below the bound (property test).
- [ ] Secrets spanning chunk boundaries are redacted (test).
- [ ] Peak memory stays bounded on inputs above the bound, measured and published in
  `docs/numbers.md`.
- [ ] Threat model §5 item 6 is updated.

**Dependencies:** T-10-6-004  
**Reference:** `docs/security-and-threat-model.md` §5 item 6  
**Estimated Complexity:** L

---

### T-10-7-003 — Optional TLS on an `--allow-remote` listener (residual #9)

**Description:** Allow TLS (certificate and key from files) on the proxy listener when
`--allow-remote` is set, and warn when remote access is enabled without it.

**Research:** Confirm whether any installed client connects remotely, and the expected certificate
workflow.

**Acceptance Criteria:**

- [ ] TLS is optional and off by default; loopback behaviour is unchanged.
- [ ] Remote access without TLS logs a warning at startup.
- [ ] Threat model §5 item 9 is updated.

**Dependencies:** T-10-7-001  
**Reference:** `docs/security-and-threat-model.md` §5 item 9  
**Estimated Complexity:** M

---

### T-10-7-004 — MAC on bus events (residual #8)

**Description:** Authenticate bus events with a MAC keyed by the per-install key, so a same-user
process without the key cannot publish accepted events.

**Research:** Measure the MAC cost on the publish/poll path with the Phase 8 bus benchmark; confirm
the format change is versioned for the `icx-shm` work (#32).

**Acceptance Criteria:**

- [ ] Events without a valid MAC are rejected and counted (test).
- [ ] The bus format version is bumped and older readers fail closed.
- [ ] Publish/poll cost with the MAC is measured and published in `docs/numbers.md`.
- [ ] Threat model §5 item 8 is updated.

**Dependencies:** T-83-002  
**Reference:** `docs/security-and-threat-model.md` §5 item 8; issue #32  
**Estimated Complexity:** M

---

### T-10-7-005 — Optional OS-keyring install key (residual #3)

**Description:** Optionally store the install key in the OS keyring instead of a file, keeping the
file as the default and as the fallback when no keyring is available.

**Research:** Confirm keyring support and prompts on Linux (Secret Service), macOS (Keychain) and
Windows (Credential Manager), and the behaviour in headless sessions.

**Acceptance Criteria:**

- [ ] The keyring option is off by default and documented per platform.
- [ ] Headless sessions fall back to the file with a clear message.
- [ ] Threat model §5 item 3 is updated to state what the keyring does and does not protect
  against.

**Dependencies:** T-93-002, T-93-003  
**Reference:** `docs/security-and-threat-model.md` §5 item 3  
**Estimated Complexity:** M

---

### T-10-7-006 — Explicit `[bans]` policy in `deny.toml`

**Description:** Extend the `[bans]` section in `deny.toml` with an explicit deny list and set
`multiple-versions = "deny"` where feasible, with documented `skip` entries for duplicates that
cannot be removed yet.

**Research:** Run `cargo deny check bans` and list every duplicate with its cause.

**Acceptance Criteria:**

- [ ] `multiple-versions = "deny"` is set, or the reason it is not is documented in `deny.toml`.
- [ ] Every `skip` entry has a comment naming its cause.
- [ ] `cargo deny check` passes in CI.

**Dependencies:** None  
**Reference:** `deny.toml`; plan Phase 10 Sprint 7  
**Estimated Complexity:** S

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Line coverage is `>= 95%` (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Affected docs, the threat model and `CHANGELOG.md` `[Unreleased]` are updated.
- [ ] Upstream re-scan for Phase 11 Sprint 1 is done and its tickets are adjusted.
