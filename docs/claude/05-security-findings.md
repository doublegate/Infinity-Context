# 05 — Security Findings (Threat-Model Driven)

Workstream W5.

> **Disclosure note.** This repository is public and the findings below affect the released
> v1.0.0. This document gives the defect, its impact and its fix, and describes reproductions
> only at a high level. Exploit-level reproduction steps and PoC code are kept outside the
> repository and can be supplied through a GitHub private security advisory. Do not add them
> here until the fixes ship.

## 1. Threat-model claims vs code (`docs/security-and-threat-model.md`, `SECURITY.md`)

| Claim (location) | Status | Evidence |
| --- | --- | --- |
| Shared memory and a UDS socket live under `$XDG_RUNTIME_DIR/icx` (0700), with `SO_PEERCRED` uid verification (threat model :19) | **Absent** | No UDS, no `XDG_RUNTIME_DIR`, no `SO_PEERCRED` anywhere in `crates/`. The bus defaults to the repo-relative `.icx/icx-fabric-v1.shm` (`main.rs:124-128`) |
| Proxy "must bind strictly to loopback or a 0600 UDS" (:11); "restricted to the local user UID" (SECURITY.md:8) | **Absent** | `--listen` / `ICX_PROXY_BIND` accept any address, including `0.0.0.0` (`main.rs:410-460`). No client authentication on any route (ICX-AUD-012) |
| Every CAS read recomputes BLAKE3 and fails closed (:20) | **Present** | `cas.rs:279-305` |
| Every edge and packet stamps `AgentKind`, `session_id`, `t_tx_start`, provenance (:21) | **Partial** | Timestamps come from a synthetic counter (ICX-AUD-048). Recall and deref output carry no provenance for non-redacted artifacts (`wire.rs:350-365`) |
| Mandatory 4-tier redaction *before* CAS, HDC and bus (:22, SECURITY.md:5) | **Present but leaky** | Placement is correct (`mmu.rs:246-278`); recall gaps in ICX-AUD-015 and ICX-AUD-016 |
| "Bounded streaming ingestion cap … 64 MiB" (:23) | **Partial** | The cap applies only *after* the whole of stdin is read into memory (`main.rs:36,50`, ICX-AUD-014) |
| Tier-0 delimiter escaping means "untrusted data can never forge a top-level AHTL packet" (:24, SECURITY.md:7) | **Bypassed** | Escaping is skipped for content that *begins with* icx's own markers (ICX-AUD-011) |
| Zero `unsafe` in all 8 crates (§3) | **Present** | `cargo geiger`: 0 unsafe in first-party crates. Dependencies (blake3 SIMD, serde_json, proc-macro2, syn) contain unsafe, as expected |

## 2. Findings

### ICX-AUD-010 — A repository-shipped `.icx/` workspace is trusted and injected into agent context (High)

- **Defect:**
  - Every hook resolves its workspace to the *relative* path `.icx` in the agent's current
    directory (`resolve_workspace_dir`, `main.rs:130-143`).
  - If that directory exists, it is loaded (`execute_hook`, `main.rs:274-296`) with no check of
    ownership, permissions, or whether it is tracked by git.
  - On `SessionStart` and `UserPromptSubmit`, recalled entries from that store are inserted into
    the model's `additionalContext`. That includes their subject labels, which are attacker-chosen
    text (about 56 characters each).
  - Their handles dereference to full attacker-authored bodies, through the MCP pager and
    `icx deref`, with no untrusted-content marker.
- **Reproduction (summary):** a repository that contains a crafted `.icx/` directory, once cloned
  and opened in an agent with icx hooks installed, injects attacker text into the first turn of
  every session. Verified end-to-end with the release binary at `c176d03`.
- **Impact:** persistent indirect prompt injection with first-turn placement, delivered by
  `git clone`. It defeats the Tier-0 guarantee, because the content never passes through
  ingestion.
- **Remediation:**
  1. Store workspaces outside the repository by default (for example
     `$XDG_STATE_HOME/icx/<blake3(canonical repo root)>`).
  2. Refuse a workspace directory that is git-tracked, not owned by `geteuid()`, or
     group/world-writable.
  3. Validate label length and charset on *load*, not only at ingest.
  4. Wrap every recalled or dereferenced body in an explicit untrusted-data envelope.

### ICX-AUD-011 — In-band trust markers: icx recognizes "its own" content by prefixes an attacker can type (Medium-High)

- **Proxy:** message strings and text blocks that start with `⟦` or `⟪ICX:` skip *both*
  virtualization *and* redaction (`wire.rs:225`, `wire.rs:256`). They reach the model unchanged
  and indistinguishable from genuine icx packets.
- **Hermes adapter:**
  - Content starting with `⟪ICX:STUB│`, `⟪ICX:HERMES_EPOCH│`, `⟪ICX:HERMES_COMPACT│` or
    `⟪ICX:TAIL_SLOT│` (or a message named `icx_tail_slot`) is treated as icx-synthesized
    (`hermes.rs:79-89`).
  - Embedded `⟦…⟧` lines in such messages keep their structural delimiters; only the segments
    between separators are sanitized (`hermes.rs:92-118`).
  - The lines are stored with the unchecked `put_bytes` (`hermes.rs:165`).
  - No role or provenance check applies, so a tool result or fetched page can forge `Ω` invariant
    packets.
- **Evidence:** code-path confirmation at the cited lines. A dynamic Hermes PoC was not run.
- **Remediation:**
  - Establish provenance out of band: a per-install secret and a keyed MAC (BLAKE3 keyed mode)
    over every icx-emitted block, or a side table of emitted artifact IDs.
  - Never trust a content prefix. Always apply Tier-0 escaping to content of non-icx origin.

### ICX-AUD-012 — The proxy has no client authentication; `/health` and `/sse-expand` serve CAS data (Medium)

- **Defect:**
  - The proxy (`serve_http_listener_with_config`, `wire.rs:2139`) accepts any client.
  - `GET /health` discloses store statistics.
  - `POST /sse-expand` expands `⟪!icx:deref(#icx:…)⟫` directives against the CAS and returns
    the full stored bodies (`wire.rs:1981-2009`).
  - Non-loopback binds are accepted without warning.
- **Reproduction (summary):** with the proxy running, any client that can reach the port and
  knows a handle receives the stored content. Verified locally.
- **Exploitability:** handles are 64-bit, or 48-bit prefixes, and cannot be guessed. Real
  exposure needs a leaked handle (transcripts, logs, shared screens), plus either a different
  local user on a multi-user host or a non-loopback bind. A browser page can POST to the
  loopback port but cannot read the response (no CORS headers).
- **Remediation:**
  - Require a per-install bearer token (a 0600 file) on every route.
  - Refuse non-loopback binds unless `--allow-remote` is given.
  - Remove `/sse-expand` from the production listener, or gate it behind the token.

### ICX-AUD-013 — `icx init` does not ignore the workspace; directory mode 0755 (Medium, privacy)

- **Defect:** `icx init` creates `.icx/` (mode 0755) with no `.gitignore`. In a git repository
  the store shows up as untracked files (`cas_store.json`, `mmu_state.json`, the bus file).
  A routine `git add -A` therefore commits the user's session tool-output history, which the
  redactor has only partly scrubbed (ICX-AUD-015).
- **Remediation:** write `.icx/.gitignore` containing `*`, and create the directory with mode
  0700. Better still, apply ICX-AUD-010 item 1 and keep the workspace out of the repository
  entirely.

### ICX-AUD-014 — Unbounded hook input; the 64 MiB cap applies after full buffering (Medium, DoS)

- **Defect:** `hook` and `proxy-transform` call `read_to_string` on stdin with no limit
  (`main.rs:36,50`).
- **Measured:** a 300 MiB tool output made a single synchronous `PostToolUse` hook use
  **1.12 GB RSS and 4.32 s**. The stored artifact was silently truncated to 64 MiB.
- **Other unbounded or quadratic paths, from code reading:**
  - MCP stdio line length (`main.rs:348`);
  - unlimited nested `fold begin:` branches (`mcp.rs:310`);
  - an O(n²) chunked-SSE rescan (`wire.rs:1340-1356`);
  - up to 64 proxy threads × 16 MiB request bodies.
- **Remediation:**
  - Read stdin through `Read::take(ICX_MAX_INGEST_BYTES + 1)` and reject or stream the overflow.
  - Cap MCP line length and fold depth.
  - Make SSE parsing incremental.

### ICX-AUD-015 — Secret redactor recall gaps (Medium)

- **Leaks (verified):**

| Input | Result |
| --- | --- |
| `git clone https://alice:<password>@github.com/…` | **leaked**: `http`, `https`, `ftp` and `git` URL user-info is skipped (`redact.rs:408`); the URL rule covers only database and broker schemes (`redact.rs:480-490`) |
| `SENDGRID=SG.<22>.<43>` | **leaked**: the entropy gate excludes any token containing `.` (`redact.rs:669-677`) |
| Pretty-printed JSON with the key and the secret on different lines | **leaked**: redaction is line-based |
| `Authorization: Bearer ya29.…` | redacted (Bearer rule) |
| `glpat-…` | redacted (entropy gate) |
| `ENCRYPTION_KEY: <32 hex>` | redacted (entropy gate) |

- **Static review:**
  - There are no explicit rules for `glpat-`, `xapp-`, `dop_v1_`, `shpat_`, Slack webhook URLs,
    Azure keys or Twilio `SK…`; some are caught by the entropy gate only by chance.
  - Sensitive key names `PASS`, `PWD`, `DSN`, `COOKIE`, `SESSION`, `CREDENTIALS` and the bare
    `KEY` are missing.
- **Remediation:**
  - Import the gitleaks rule corpus (`.gitleaks.toml` already exists in CI) as a test fixture,
    and measure recall per rule.
  - Redact URL user-info for every scheme.
  - Handle multi-line JSON and YAML values with a small structural pass.
- **Regression gate:** ≥ 95% recall on the gitleaks rule fixtures.

### ICX-AUD-016 — A single-line PEM block makes the redactor delete the rest of the input (Medium, data loss)

- **Defect:** a JSON-escaped private key on one line
  (`"-----BEGIN RSA PRIVATE KEY-----\n…\n-----END…"`) opens the PEM block. The END marker is
  checked only on *later* lines (`redact.rs:24-34`), so every following line is dropped. In the
  PoC, a build error on line 3 disappeared. An unterminated `BEGIN` behaves the same way.
- **Remediation:** detect BEGIN and END on the same line; cap the block at a reasonable number
  of lines (for example 200) and resume afterwards.

### ICX-AUD-017 — The security documentation overstates the shipped controls (Medium)

This rolls up the "Absent", "Partial" and "Bypassed" rows of §1. A threat model that documents
controls which do not exist misleads users' own risk decisions.

- **Remediation:** rewrite `docs/security-and-threat-model.md` and `SECURITY.md` against the
  code. Mark planned controls as planned. Add a reporting contact (ICX-AUD-037).

### ICX-AUD-018 — The `agy` hook blocks forever on a FIFO `transcriptPath` (Low)

- **Defect:** `read_transcript_step` calls `File::open` before checking `is_file()`
  (`antigravity.rs:155-158`). Opening a FIFO for reading blocks, so the hook hung until it was
  killed (`timeout 5` gave exit 124). Arbitrary regular files are read but yield nothing unless
  they contain `agy` JSONL step records, so this is not a content-disclosure primitive. The
  payload is written by `agy` itself.
- **Remediation:** `symlink_metadata` first, open with `O_NONBLOCK`, and restrict the path to
  the `agy` transcript directory.

### ICX-AUD-020 — The curl header temp file holds API keys and survives a crash (Low)

- **Defect:** for HTTPS upstreams, client headers, `x-api-key` and `Authorization` included, are
  written to a temp file in `$TMPDIR` (`create_new`, mode 0600) for `curl -H @file`
  (`wire.rs:1718-1751`). The file is removed by a drop guard, which does not run on SIGKILL or
  abort.
- **Remediation:** pass headers to curl through stdin (`-H @-`) or a pipe, or link a TLS client
  (`rustls`).

### ICX-AUD-009 — The PreToolUse "echo" is dead code (Info)

- **Defect:** `claude.rs:71-80` and `codex.rs:79-85` copy `permissionDecision` and
  `updatedInput` from the hook's stdin into its output. Per the Claude Code hook reference,
  these are *output-only* fields and never appear in PreToolUse input, so the code never fires.
  It is not exploitable, because the stdin is written by the agent.
- **Remediation:** delete the code, or implement a real policy.

## 3. Supply chain and static analysis

| Check | Result |
| --- | --- |
| `cargo audit` (RustSec, 1,533 advisories) | 0 vulnerabilities in 30 crates |
| `osv-scanner` on `Cargo.lock` | No issues |
| `cargo deny check` | advisories, bans and sources OK; **licenses FAILED because the repository has no `deny.toml`** (no allow-list policy). See ICX-AUD-040 |
| `cargo geiger` | 0 unsafe in the 8 first-party crates |
| `semgrep` (`p/rust`, `p/secrets`) | 20 hits: 18 `temp-dir` (17 in tests; 1 production, ICX-AUD-020), 1 `env::args` (expected for a CLI), 1 PEM test fixture |
| gitleaks (CI) | pinned 8.30.1 with SHA-256 verification: good practice |
