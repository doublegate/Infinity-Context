# Security & Threat Model Specification

**Applies to:** `icx` v1.2.0 development candidate
**References:** [docs/adr/0005-v1-1-0-workspace-trust-store-and-daemon.md](adr/0005-v1-1-0-workspace-trust-store-and-daemon.md), [docs/wire-protocol-and-shm-bus.md](wire-protocol-and-shm-bus.md), Master-Core Module 60 (Security & Input Validation)

This document describes controls implemented in the v1.2.0 candidate. Each control in §4
names its implementation; final qualification is tracked in [the release record](releases/v1.2.0/README.md). Anything `icx` does not defend
against is listed in §5 (residual risks) or §6 (out of scope). The v1.0.0 version of this document
described controls that the code did not have (audit finding ICX-AUD-017); this rewrite replaces
it.

Defects are described at the defect, impact and fix level. Reproduction detail is not published
here; report vulnerabilities through the channel in `SECURITY.md`.

---

## 1. System Summary

`icx` runs as the invoking user. It is started by agent harnesses (`icx hook`, `icx mcp-serve`),
by the user (`icx deref`, `icx status`, `icx doctor`, `icx trust`), or as a long-lived process
(`icx daemon`, `icx proxy`). All persistent state lives in a per-project **workspace**: a private
directory holding one SQLite store (`icx.sqlite3`), an `OWNER` seal, a lock file and, by default,
the file-backed telepathy ring (`icx-fabric-v1.shm`).

---

## 2. Assets

| Asset | Where it lives | Why it matters |
| :--- | :--- | :--- |
| **Workspace store** | `icx.sqlite3` in the workspace directory (default `$XDG_STATE_HOME/icx/workspaces/<16 hex>`) | Holds every virtualized tool output (after redaction), the Merkle-DAG, the bi-temporal graph and the HDC index. Its content is injected into agent context, so tampering with it is a prompt-injection path. |
| **Install key** | `$XDG_CONFIG_HOME/icx/install.key` (mode `0600`, 32 random bytes) | Root secret for the workspace `OWNER` seal, the store MAC and the proxy token. Anyone holding it can forge seals for this install. |
| **Proxy token** | Derived from the install key (`derived_key(.., "proxy-token")`); printed by `icx proxy` at startup | Authorizes use of the local proxy, which forwards requests carrying the user's provider API keys. |
| **Provider API keys** | In the agent's request headers; transiently in a curl header file (see §5) | Billing and data access at the provider. |
| **CAS content** | Chunk and artifact rows in the store | Redacted copies of tool output and file reads. May contain secrets the redactor did not recognize (§5). |
| **Agent context** | Hook responses, MCP results, proxy-rewritten requests | Whatever `icx` emits is read by a model with tool access; forged instructions here are the main integrity risk. |

---

## 3. Trust Boundaries

| Boundary | Trust level | Notes |
| :--- | :--- | :--- |
| **Agent processes** (Claude Code, Codex, `agy`, Hermes, Muse, MCP clients) | Trusted to send well-formed hook JSON; their *content* is untrusted | Hook stdin is written by the agent itself, but it carries tool output and file content the agent read. |
| **Repository content**, including cloned third-party repositories | Untrusted | A repository can ship files intended to be read by `icx`, including a prepared `.icx/` directory. |
| **Tool output** (shell commands, file reads, web fetches, logs) | Untrusted | May contain secrets, and may contain text that imitates `icx` packet delimiters (`⟦ ⟧ ⟪ ⟫`) or instructions. |
| **Network peers** of the proxy | Untrusted | Any process or host that can reach the proxy port. |
| **Upstream provider APIs** | Trusted for transport (TLS via `curl`); response content untrusted | Streamed responses are parsed for inline deref directives. |
| **Other local users** | Untrusted | Must not read or alter workspaces, the install key, the bus file or the daemon socket. |
| **Other processes of the same user** | Trusted (out of scope, §6) | They can read the user's files and keys regardless of `icx`. |

---

## 4. Threats and Controls

### 4.1 Workspace selection and integrity

| Threat | Control | Implementation |
| :--- | :--- | :--- |
| A cloned repository ships a populated `.icx/` that `icx` loads and injects into agent context (ICX-AUD-010). | The default workspace lives outside the repository, at `<state_dir>/workspaces/<first 16 hex of blake3(canonical project root)>`; the project root is the nearest ancestor containing `.git`. A repository-local `.icx/` is never loaded implicitly; `icx` prints a one-time warning that it is ignored. | `crates/icx-adapters/src/workspace.rs` (`project_root`, `default_workspace_dir`, `resolve`); `crates/icx-cli/src/main.rs` |
| A directory selected with `--dir` or `ICX_DIR` was prepared by someone else. | Every workspace must be a directory owned by the effective uid with no group/other permission bits, and must carry a valid `OWNER` seal: a keyed BLAKE3 MAC of its canonical path under the install key. An existing unsealed directory is refused until the user runs `icx trust --dir <path>`, which sets mode `0700`, rewrites `.gitignore` to `*`, writes the seal, re-stamps the store, and warns if the directory is inside a git work tree. A seal does not survive copying or moving the directory. | `workspace.rs` (`verify`, `prepare`, `trust`, `check_dir_security`, `compute_seal`) |
| The store file inside a trusted directory is replaced (for example by `git pull` in a directory the user trusted). | Every save MAC-stamps the store: a keyed BLAKE3 MAC over the canonical path, the store `generation`, and a digest of all persisted state (chunk config, graph, MMU state, HDC snapshot and event count, and every artifact id with its full digest). Sealed loads reject a store whose MAC does not verify. | `crates/icx-adapters/src/store.rs` (`state_digest`, `store_seal`) |
| No private location can be determined (`HOME` and XDG variables unset or relative). | Only absolute environment paths are honoured. With no usable root, trust operations fail closed; there is no fallback to a shared directory such as `/tmp`. | `WorkspaceRoots::from_env` in `workspace.rs` |
| Other local users read workspace content (ICX-AUD-013). | Workspace directories are created `0700`, files `0600`; the workspace `.gitignore` is always `*`. The install key file and config directory ownership and mode are verified on every use, including the creation race. | `workspace.rs` (`private_dir`, `write_private_file`, `install_key`, `check_private_file`); `store.rs` (`0o600` open) |
| Stored bytes are corrupted or altered below the MAC (bit rot, partial writes). | Every CAS chunk and reassembled artifact is verified against its full 256-bit BLAKE3 digest on load and on every dereference; mismatch fails closed with `IcxError::IntegrityViolation`. Each save is one SQLite transaction (WAL), so a crash cannot leave a mixed-generation store. | `crates/icx-core/src/cas.rs` (`deref_bytes`, `deref_lines`, `from_parts`, `verify_all_integrity`); `store.rs` |
| Two different payloads share a 64-bit handle (ICX-AUD-022, ICX-AUD-023). | A collision returns `IcxError::HashCollision`; the payload is not virtualized and nothing is overwritten. DAG inserts likewise refuse to overwrite a different node. | `cas.rs` (`put_bytes_checked`); `crates/icx-core/src/dag.rs` |

### 4.2 Secrets in tool output

| Threat | Control | Implementation |
| :--- | :--- | :--- |
| A secret in tool output is stored in the CAS, indexed, published on the bus or echoed back to a model. | `SecretRedactor::redact` runs on every ingested payload before BLAKE3 hashing, CAS insertion, HDC indexing and bus publication. It removes private-key blocks (multi-line, single-line JSON-escaped and unterminated, with a 200-line cap), vendor token formats, JWTs, URL user-info passwords for every scheme, CLI secret flags, `Bearer`/`Basic` headers, sensitive key/value assignments (including multi-line JSON/YAML values after a sensitive key), and high-entropy tokens. Redacted artifacts set `FLAG_IS_REDACTED` (bit 7 of hologram byte `B[7]`). | `crates/icx-codec/src/redact.rs`; call site `InfinityContextMmu::bound_and_sanitize_raw` in `crates/icx-adapters/src/mmu.rs`; regression fixtures `crates/icx-codec/tests/redact_recall_and_losslessness.rs` |
| Redaction corrupts non-secret content (v1.0.0 escaped delimiters and normalized bytes in storage; ICX-AUD-021, ICX-AUD-016). | Storage is lossless after redaction: `redact` removes secrets only and preserves all other bytes, line endings, trailing newlines and literal delimiters. For secret-free input, `deref(ingest(x)) == x` (property-tested). | `redact.rs`; `crates/icx-codec/tests/properties.rs` |

### 4.3 Prompt injection through `icx` output

| Threat | Control | Implementation |
| :--- | :--- | :--- |
| Untrusted text contains `⟦…⟧` or `⟪…⟫` sequences that imitate an `icx` packet, epoch header or invariant (ICX-AUD-011). | Model-facing text is passed through `SecretRedactor::sanitize` (redact plus Tier-0 delimiter escaping). The MMU records BLAKE3 digests of every block it emits; a delimiter-bearing string anywhere in any non-system message, including the latest, is treated as `icx`'s own only on an exact byte match with an emitted block, and is otherwise Tier-0 escaped. | `redact.rs` (`sanitize`); `mmu.rs` (`is_forged_icx_block`); `crates/icx-proxy/src/wire.rs`; `crates/icx-adapters/src/hermes.rs` |
| A slice hint or clause chain inside a stream is used to forge or break packet structure (ICX-AUD-025, ICX-AUD-026). | `slice_hint` must match the `SliceMap` production (`L<a>-<b>` or `B<a>:<b>`); invalid hints are never rendered and are rejected on parse. Clause chains are parsed iteratively and materialize at most 64 `⇒` links (`MAX_CLAUSE_CHAIN`). | `crates/icx-codec/src/ahtl.rs` (`is_valid_slice_map`, `MAX_CLAUSE_CHAIN`) |
| Dereferenced content is mistaken by the model for instructions from `icx`. | MCP `deref` output always starts with `⟪ICX:PROVENANCE│id=…│redacted=…│exactness=post_redaction_cas│trust=untrusted_data⟫` followed by the escaped body. `icx deref` on the CLI is framed the same way by default; `--raw` prints exact stored bytes. | `crates/icx-adapters/src/mcp.rs`; `crates/icx-cli/src/main.rs` |
| A hook response overrides the user's own agent policy (v1.0.0 echoed decision fields from hook input; ICX-AUD-009, ICX-AUD-007). | PreToolUse responses carry no permission decision (Claude Code, Codex); `agy` PreToolUse always returns `{"decision":"allow"}` (no opinion). `agy` Stop always returns `{"decision":"stop"}` and PostInvocation never sets `force_continue`. Claude Code SubagentStop returns `{}`. | `crates/icx-adapters/src/claude.rs`, `codex.rs`, `antigravity.rs` |

### 4.4 Local proxy

| Threat | Control | Implementation |
| :--- | :--- | :--- |
| Another process or host uses the proxy to spend the user's API keys or read CAS data through `/health` or `/sse-expand` (ICX-AUD-012). | Every route requires the install-derived token, as a base-URL prefix `/t/<token>` or an `x-icx-token` header, compared in constant time; requests without it receive 401 before any body is read. The token header is stripped before forwarding. `--insecure-no-auth` disables this and prints a warning. | `crates/icx-proxy/src/wire.rs` (token check, header filter); `crates/icx-proxy/tests/proxy_auth.rs` |
| The proxy is exposed on a network interface. | Default listen address is `127.0.0.1:49380`. Non-loopback binds are refused unless `--allow-remote` is given. | `crates/icx-cli/src/main.rs` (proxy argument handling) |
| API keys are left on disk in curl header files (ICX-AUD-020). | Header files are created `create_new` with mode `0600` in a directory verified to be a real directory owned by the current user with no group/other access (`$XDG_RUNTIME_DIR/icx`, else `<tmp>/icx-<uid>`), removed by a drop guard after each request, and stale files older than 60 s are swept at proxy startup and every minute while it runs. | `wire.rs` (`curl_scratch_dir`, `sweep_stale_curl_header_files`) |
| Header or method injection into the upstream request. | Header names must be valid HTTP tokens; values containing CR, LF or NUL are dropped; hop-by-hop and internal headers are not forwarded; the method is sanitized. | `wire.rs` (`forward_https_upstream`) |

### 4.5 Local IPC: daemon socket and telepathy bus

| Threat | Control | Implementation |
| :--- | :--- | :--- |
| Another user connects to the resident daemon. | The socket `ws-<hash>.sock` lives in `$XDG_RUNTIME_DIR/icx` (mode `0700`); every connection is authenticated with `SO_PEERCRED` and accepted only if the peer uid equals the daemon's. Socket paths over the 103-byte `sun_path` limit are rejected with an error. | `crates/icx-cli/src/daemon.rs`; `workspace.rs` (`daemon_socket_path`, `ensure_runtime_dir`) |
| A planted symlink, FIFO or foreign file is used as the bus file (ICX-AUD-019). | The bus file is opened with `O_NOFOLLOW` and `O_CLOEXEC`, created `0600`, and refused if it is not a regular file, is owned by another uid, or has any group/other permission bits. | `crates/icx-bus/src/ring.rs` (`open_shared_file`) |
| A FIFO or symlink as `agy` `transcriptPath` hangs or redirects the hook (ICX-AUD-018). | The transcript is opened non-blocking without following symlinks, and only regular files are read. | `crates/icx-adapters/src/antigravity.rs` |

### 4.6 Resource exhaustion

| Threat | Control | Implementation |
| :--- | :--- | :--- |
| Oversized hook input exhausts memory (v1.0.0 buffered all of stdin before applying the cap; ICX-AUD-014). | Hook and `proxy-transform` stdin is read through a bound of 64 MiB (`ICX_MAX_INGEST_BYTES`) plus a 64 KiB envelope; over-limit input fails open (the agent's original output is left unchanged). Daemon requests have the same bound. Stored payloads are capped at 64 MiB. | `crates/icx-cli/src/main.rs` (`read_bounded`, `MAX_STDIN_BYTES`); `daemon.rs` (`MAX_REQUEST_BYTES`); `crates/icx-core/src/cas.rs` (`check_ingest_size`) |
| Oversized or deeply nested MCP input. | MCP stdio lines are bounded at 4 MiB; at most 64 nested `fold begin:` branches (`MAX_FOLD_DEPTH`). | `main.rs` (`MAX_MCP_LINE_BYTES`); `mcp.rs` (`MAX_FOLD_DEPTH`); `crates/icx-adapters/tests/mcp_bounds.rs` |
| Quadratic SSE re-scanning on chunked responses. | Chunked SSE line splitting is linear (scan cursor, one drain per chunk); SSE lines are bounded at 16 MiB. | `wire.rs` (`MAX_SSE_LINE_BYTES`) |
| A hostile HDC snapshot in the store exhausts memory on load or forges derived state. | The decoder reads `ICXHDC03`, `ICXHDC04` and `ICXHDC05`, bounds counts before allocation and validates finite/range and derived-state consistency. New snapshots use 05; the MMU rebuilds legacy 03/04 postings from preserved events. A fuzz target covers decoding. | `crates/icx-hdc/src/snapshot.rs`; `fuzz/fuzz_targets/hdc_snapshot.rs` |

### 4.7 Memory safety and supply chain

The v1.2.0 redactor recognizes sensitive YAML literal/folded block headers, optional indentation
and chomping indicators, and same-line tag/anchor properties. Indented block contents are
removed before comment, mapping or PEM detection. Indented plain scalar continuations are also
redacted, and `=` is scalar content rather than a YAML mapping separator. Dedented public
siblings and actual nested mappings remain intact. This follows the block indentation rules in
[YAML 1.2.2 §8.1](https://yaml.org/spec/1.2.2/#81-block-scalar-styles), rather than claiming a full
YAML parser or universal secret detection. These rules apply to new ingestion; previously
stored artifacts are not silently scrubbed or changed. The minimized idempotence regressions
and real MMU CAS tests are in `redact_regression_fixtures.rs` and `redaction_before_storage.rs`.

Profile A public skeletons use the same ASCII delimiter neutralization as ingestion. Literal
packet/header glyphs are a lossy skeleton view; CAS dereference remains the exact post-redaction
source. Ordinary existing Profile A skeletons and percent text remain unchanged. Nested raw
framing markers are rejected by the standalone parser.

| Concern | Control | Implementation |
| :--- | :--- | :--- |
| Memory-safety defects in first-party code. | `unsafe_code = "forbid"` in `[workspace.lints]`, inherited by all 8 crates; CI checks it. Dependencies (for example `blake3`, bundled SQLite through `rusqlite`) contain `unsafe` or C code, as expected. | `Cargo.toml`; `.github/workflows/ci.yml` |
| Parsers crash on hostile input. | cargo-fuzz targets for the AHTL stream parser, the HDC snapshot decoder, MCP JSON-RPC handling and the redactor. | `fuzz/fuzz_targets/` |
| Vulnerable or incompatibly licensed dependencies; mutable CI inputs. | `cargo deny check` (advisories, licenses, bans, sources) runs in CI; the toolchain is pinned to `1.100.0`; every GitHub Action is pinned to a commit SHA; gitleaks scans history. | `deny.toml`; `rust-toolchain.toml`; `.github/workflows/ci.yml` |

---

## 5. Residual Risks and Limits

These limits also apply to the v1.2.0 candidate.

1. **Redaction is pattern-based and can miss secrets.** The redactor recognizes known formats,
   sensitive key names and high-entropy tokens. A secret in an unrecognized format, a
   low-entropy password with no sensitive key nearby, or a secret split across unusual
   structure can be stored in the CAS and shown to a model. Do not rely on `icx` as the only
   control keeping secrets out of agent context.
2. **The redactor can also over-redact.** High-entropy non-secrets (hashes, identifiers) may be
   removed. Such artifacts are marked redacted; dereferencing returns the redacted bytes, not the
   original.
3. **Same-user processes are trusted.** Any process running as the user can read the install key,
   workspaces, bus file and daemon socket, and can therefore forge seals and store MACs. The seal
   and MAC defend against content shipped by repositories and against other users, not against
   malware running as the user.
4. **Content `icx` returns is still untrusted data.** Provenance headers and delimiter escaping
   tell the model what the content is; they cannot prevent a model from following instructions
   contained in tool output or files it reads. The agent's own permission model remains the
   control for what a model may do.
5. **Tier-0 escaping covers only `icx`'s delimiters.** It does not neutralize other markup or
   natural-language instructions.
6. **Pass-through above the size bound.** Hook input above 64 MiB is passed through unmodified
   (fail open), so it reaches the agent without virtualization or redaction by `icx`.
7. **Curl header files.** Provider API keys are written to a `0600` file for the duration of each
   upstream request. If the proxy is killed (SIGKILL, abort), the file remains until the next
   proxy start sweeps it. Without `XDG_RUNTIME_DIR`, the file is written to the system temporary
   directory.
8. **The bus is not authenticated beyond file permissions.** Any same-user process can publish
   events on the file-backed ring; readers treat bus payloads as untrusted. Events can be lost
   when the ring laps; this is reported (§2.3 of the bus specification), not prevented.
9. **`--allow-remote` and `--insecure-no-auth`** remove the corresponding proxy controls. The
   proxy speaks plain HTTP to its clients; with `--allow-remote`, the token travels in clear text
   on the network.
10. **Dereference integrity is relative to the store.** BLAKE3 verification proves that bytes
    match what was stored, not that what was stored matches the original tool output (see items 1
    and 2).

---

## 6. Out of Scope

- Malware or a compromised account running as the same user, and root.
- Compromise of an agent harness, the provider API, the TLS stack used by `curl`, or the
  operating system.
- Confidentiality of prompts and responses at the provider.
- Physical access, disk encryption and backups of the state directory.
- Multi-tenant or networked deployments: `icx` is designed for a single user on a single
  workstation.
- Correctness of model behaviour given `icx` output.
