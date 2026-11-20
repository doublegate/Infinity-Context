# Security Policy

## Supported Versions

| Version | Supported |
| :--- | :--- |
| `1.1.x` | Yes |
| `1.0.x` | No. Upgrade to 1.1.0, which fixes the findings in [`docs/claude/`](docs/claude/README.md). |

## Reporting a Vulnerability

Report vulnerabilities privately through GitHub's
[private vulnerability reporting](https://github.com/doublegate/Infinity-Context/security/advisories/new)
(**Security → Report a vulnerability**). Do not open a public issue for a suspected vulnerability.

Include the affected version or commit, the component (`icx-proxy`, hooks, MCP server, workspace
store, bus), reproduction steps, and the impact you observed. You should get an acknowledgement
within 7 days. Fixes are released as a patch version with a GitHub security advisory crediting the
reporter unless you ask otherwise.

## Security Invariants (v1.1.0)

1. **Secret redaction before storage.** `SecretRedactor::redact` (`crates/icx-codec/src/redact.rs`)
   removes recognized credentials before content is chunked, hashed or published on the bus. It is
   pattern- and entropy-based, so it reduces but cannot guarantee the absence of secrets.
2. **Lossless, integrity-checked storage.** Redacted content is stored byte-exact in a BLAKE3
   content-addressed store; reads re-verify the digest and fail closed. A 64-bit handle collision
   is refused instead of overwriting.
3. **Untrusted content stays data.** Text returned to a model (deref, slice, grep, tail slots) is
   Tier-0 escaped and framed with a `trust=untrusted_data` provenance header. icx structural
   markers found anywhere in conversation input are escaped unless they are byte-identical to a
   block icx itself emitted.
4. **Workspace trust.** Workspace stores live under `$XDG_STATE_HOME/icx/workspaces/` (0700
   directories, 0600 files), are sealed with a keyed BLAKE3 MAC under a per-install key, and are
   refused if they are not owned by the current user, are group/world-accessible, or fail the seal.
   A repository-local `.icx/` is never loaded implicitly; `icx trust --dir` adopts one explicitly.
5. **Local-only proxy with authentication.** `icx proxy` binds loopback addresses only unless
   `--allow-remote` is given, and requires a per-install token (`/t/<token>` path prefix or
   `x-icx-token` header) unless `--insecure-no-auth` is given.
6. **Daemon and bus isolation.** The resident daemon listens on a Unix socket in a 0700 runtime
   directory and checks the peer UID with `SO_PEERCRED`. The file-backed bus refuses symlinks,
   files owned by another user, and group/world-accessible modes.
7. **No `unsafe` code.** `unsafe_code = "forbid"` applies to every crate.

See [`docs/security-and-threat-model.md`](docs/security-and-threat-model.md) for the threat model.
