# Infinity-Context (`icx`) Antigravity Review Style Guide

These are the project rules the Antigravity reviewer enforces on **Infinity-Context (`icx`)** pull requests.
`Infinity-Context` is an 8-crate Rust systems architecture (`icx-core`, `icx-hdc`, `icx-codec`, `icx-store`, `icx-bus`, `icx-adapters`, `icx-proxy`, `icx-cli`) providing a Cross-Agent Virtual Context Memory Management Unit (MMU), Alien Holographic Token Lattice (`AHTL`) codec, lossless BLAKE3 Merkle-DAG (`LCM`), 10,240-bit Hyperdimensional (`HDC`) + Contractive IFS (`FCGA`) + Bi-Temporal Personalized PageRank (`PPR`) index, Prefix-Cache-Locked Epoch Controller, and `/dev/shm` Seqlock Telepathy Bus.

Authoritative context:

- `AGENTS.md`: workspace map, architecture summary, and quality gates.
- `docs/architecture.md` and `docs/adr/`: architectural invariants and decisions already made.
- `REUSE.toml` and `LICENSING.md`: REUSE 3.3 file licensing map (`GPL-3.0-or-later`, `AGPL-3.0-or-later`, `CC-BY-NC-ND-4.0`, `LicenseRef-InfinityContext-Commercial`).

## Priorities (in order)

1. Lossless data integrity (`#icx:<16-hex>` BLAKE3 content-addressed storage and fail-closed digest verification on read).
2. Memory safety and soundness (`unsafe_code = "forbid"` in all crates except `icx-bus`, where every `unsafe` block requires a `// SAFETY:` invariant comment).
3. Pre-hash secret redaction and prompt-injection boundary sanitization (`SecretRedactor` must run before `CasStore` hashing or `/dev/shm` broadcast).
4. Cross-agent hook and wire invariants (e.g., `AntigravityAdapter::exit_code() == 0` so `agy` never triggers an emergency abort on exit code `2`; byte-frozen sealed epochs in `CacheEpochController`).
5. Honest documentation: every metric or claim in `README.md` or `docs/` must be backed by code and reproducible via `cargo test --workspace` or `cargo run --release -p icx-cli -- benchmark`.

## What to flag as BLOCKING

### Memory safety and `unsafe` policy

- Any `unsafe` block outside `crates/icx-bus/src/`.
- Any `unsafe` block in `crates/icx-bus/src/` that lacks an explicit `// SAFETY:` comment stating the upheld invariant, or that violates 64-byte alignment / seqlock ordering (`Acquire` / `Release` fences).
- Constructing `FabricMemoryLayout` (~1 MiB) on the stack instead of heap-allocating via `alloc_zeroed`.

### Lossless CAS, AHTL codec, and security boundaries

- Bypassing `SecretRedactor::sanitize` before hashing raw tool outputs into `CasStore` or publishing to `SeqlockTelemetryBus`.
- Returning unverified bytes from `CasStore::get_bytes` without verifying the 256-bit BLAKE3 digest against the `#icx:<16-hex>` handle.
- Mutating sealed epochs (`Zone 0` or `Zone 1`) mid-session in `CacheEpochController` or `WireProxyTransformer`, or exceeding Anthropic's 4-breakpoint `cache_control` limit.
- Returning exit code `2` from `AntigravityAdapter` on a normal `BeforeModel`, `AfterTool`, or `PreCompress` hook invocation.

### Licensing and REUSE 3.3 compliance

- Any new file that breaks `uvx reuse lint` (REUSE 3.3).
- Any reference to non-copyleft permissive code licenses on first-party code.

## What to keep as SUGGESTION / NITPICK

- Naming, structure, and readability.
- Missing tests for non-critical helper paths.
- Micro-optimizations without a benchmark measurement from `icx-cli benchmark`.

## Project conventions

- **Commits:** Conventional Commits (`feat|fix|docs|refactor|test|chore|perf|build|ci(scope):`).
- **No emojis** in code, comments, commits, or documentation.
- **Quality gates:**
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
  - `uvx reuse lint`
  - `bash scripts/agy-review-selftest.sh`
