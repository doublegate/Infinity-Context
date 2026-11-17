<!-- SPDX-FileCopyrightText: 2026 DoubleGate -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# GitHub Copilot & Multi-Agent Review Instructions — Infinity-Context (`icx`)

## 1. Architectural Ground Truth

**Infinity-Context (`icx`)** is an 8-crate Rust Virtual Context Memory Management Unit (MMU) and symbolic compression architecture providing unlimited effective context within and across AI coding and research agents (**Claude Code**, **OpenAI Codex CLI**, **Google Antigravity CLI (`agy`)**, **Hermes Agent**, and MCP clients):

1. **Plane 1 — Alien Holographic Token Lattice (`crates/icx-codec`):**
   - Non-human BPE-aligned symbolic codec (`AHTLv1`), bijective Unicode Braille-256 (`U+2800..U+28FF`) 64-bit hypervector sketching, single-pass lexical AST/diagnostic skeletonizer (`StructuralSkeletonizer`), and mandatory pre-hash credential/injection sanitizer (`SecretRedactor`).
2. **Plane 2 — Virtual Context MMU & 4-Tier Lossless Merkle-DAG (`crates/icx-core`):**
   - Rolling 64-bit Gear-hash `FastCdcChunker`, 128-bit `BLAKE3` (`#icx:<16-hex>`) content-addressed store (`CasStore` with `zstd` compression and fail-closed 256-bit verification on `deref`/`slice_lines`), 4-tier `MerkleContextDag` (`L0 RawChunk` -> `L1 EpisodeNode` -> `L2 FoldedSubtask` -> `L3 EpochRoot`), and `CacheEpochController`.
3. **Plane 3 — 10,240-Bit Hyperdimensional, Contractive IFS & Bi-Temporal PPR Index (`crates/icx-hdc`, `crates/icx-store`):**
   - `Hypervector([u64; 160])` hardware `POPCNT` Hamming associative memory, Infini-attention Delta-Rule orthogonality bundler (`HdcBundler`), 16-D Contractive Iterated Function System (`FractalCoord`), Titans test-time surprise momentum gate (`TitansSurpriseGate`), and contradiction-invalidating `BiTemporalGraph` with HippoRAG `PersonalizedPageRank` (`PPR`).
4. **Plane 4 — Prefix-Cache-Locked Wire Proxy (`crates/icx-proxy`):**
   - `WireProxyTransformer` managing the 4-zone prompt memory map (`Zone 0` Pinned System/Tools, `Zone 1` Sealed `L3` Cache Epochs, `Zone 2` Active Window, `Zone 3` Ephemeral Dynamic Recall Tail) for Anthropic 4-breakpoint `cache_control` and OpenAI 1,024-token prefix caching.
5. **Plane 5 — Lock-Free `/dev/shm` Seqlock Telepathy Bus & Multi-Harness Adapters (`crates/icx-bus`, `crates/icx-adapters`, `crates/icx-cli`):**
   - `SeqlockTelemetryBus` (`512 x 2,048-byte` `#[repr(C, align(64))]` slots in `/dev/shm/icx-fabric-v1.shm` with zero-stack-overflow heap fallback), `ClaudeCodeAdapter`, `CodexAdapter`, `AntigravityAdapter`, `HermesAdapter`, and Single-Tool Polymorphic `McpPagerServer` (`icx`, 138 tokens).

---

## 2. Hard Safety & Engineering Invariants

- **Workspace-Wide `#[forbid(unsafe_code)]`:** Enforced in 7 of the 8 crates (`icx-core`, `icx-hdc`, `icx-codec`, `icx-store`, `icx-adapters`, `icx-proxy`, `icx-cli`).
- **Isolated Seqlock Shared Memory (`crates/icx-bus`):** `unsafe` is permitted **only** in `crates/icx-bus/src/lib.rs` for POSIX `shm_open`/`mmap` and volatile seqlock operations over `#[repr(C, align(64))]` types, and every `unsafe` block must document its invariant in a `// SAFETY:` comment.
- **Antigravity (`agy`) Exit-Code-0 Guard:** `AntigravityAdapter::exit_code()` in `crates/icx-adapters` must **always** return `0` on all code paths (binary forensics verified that exit code `2` triggers an emergency abort in `agy`).
- **Pre-Hash Secret Redaction:** `SecretRedactor::sanitize()` must run **before** `CasStore::put_bytes()` or `SeqlockTelemetryBus::publish()`. Synthetic unit-test fixtures in `crates/icx-codec/src/redact.rs` are intentional test vectors allowlisted in `.gitleaks.toml`.
- **Fail-Closed Untrusted Input Handling:** Never `unwrap()` or `panic!()` on untrusted external input (tool outputs, JSON-RPC arguments, or corrupted CAS blobs).
- **REUSE 3.3 & Multi-Tier Licensing:** All files must pass `uvx reuse lint` under `REUSE.toml` (`GPL-3.0-or-later` for core code/scripts, `GPL-3.0-or-later AND AGPL-3.0-or-later` for `icx-proxy` and `icx-adapters`, and `CC-BY-NC-ND-4.0 AND LicenseRef-InfinityContext-Commercial` for specifications, research synthesis, and artwork).

---

## 3. Multi-Bot Pull Request Review Coordination

Pull requests in this repository are reviewed collaboratively by four automated systems plus CI/Codecov:

1. **Antigravity PR Review (`agy`)** — Runs on the dedicated self-hosted runner (`[self-hosted, linux, x64, agy]`) via `.github/workflows/antigravity-review.yml` and `.github/agy-review.md`, maintaining a single sticky comment (`<!-- antigravity-pr-review -->`).
2. **OpenAI Codex (`@codex review`)** — Performs deep dataflow, GraphQL/API contract, and concurrency analysis.
3. **CodeRabbit AI (`@coderabbitai`)** — Configured via `.coderabbit.yaml` (`chill` profile) for walkthroughs, static analysis (`clippy`, `shellcheck`, `actionlint`, `gitleaks`), and inline diffs.
4. **GitHub Copilot Code Review** — Configured via this file (`.github/copilot-instructions.md`) and enabled on push/PR via the `Protect (Default)` repository ruleset.
5. **Codecov (`codecov.yml`)** — Enforces project/patch coverage thresholds and posts per-crate coverage deltas across all 8 workspace crates.

When reviewing a pull request, focus on **substantive correctness, memory safety, concurrency, cryptographic integrity, and security findings**, and avoid duplicating nitpicks already covered by linters or companion review bots.
