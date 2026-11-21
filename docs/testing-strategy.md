# Testing & Verification Strategy

**Applies to:** `icx` v1.2.0 development candidate
**References:** Master-Core Module 20 (Test-Driven Accuracy), [docs/performance.md](performance.md), [docs/build-and-tooling.md](build-and-tooling.md)

---

## 1. Verification Layers

1. **Layer 1: In-module unit and invariant tests (`cargo test --workspace`)**
   - **`icx-core`:** FastCDC determinism and chunk bounds, CAS round-trip
     (`put_bytes` then `deref_bytes`, byte equality), `deref_lines` boundaries, `grep_cold`,
     DAG epoch sealing, tamper detection (`IcxError::IntegrityViolation`), and collision handling
     (`IcxError::HashCollision`, no overwrite in CAS or DAG).
   - **`icx-hdc`:** hypervector quasi-orthogonality, XOR self-inverse, permutation invertibility,
     Delta-rule bundling (including no counter saturation), fractal suffix depth, surprise-gate
     momentum, and snapshot encode/decode.
   - **`icx-codec`:** Braille-256 bijection, packet render/parse round-trip, delimiter escaping,
     redaction coverage, and compression ratios on synthetic logs.
   - **`icx-store`:** functional-edge invalidation and PPR convergence.
   - **`icx-bus`:** concurrent publish/read consistency on both backends.
   - **`icx-adapters`:** wire-JSON conformance per agent and the MCP tool.

2. **Layer 2: Per-crate integration tests (`crates/*/tests/`)**

   | File | Covers |
   | :--- | :--- |
   | `crates/icx-adapters/tests/workspace_trust.rs` | Default location outside the repository, repo-local `.icx/` never selected, refusal of unsealed, copied, permissive or foreign-key workspaces, fail-closed roots, store replacement and row edits rejected by the store MAC |
   | `crates/icx-adapters/tests/workspace_store.rs` | SQLite round-trip, one generation per save, non-destructive legacy JSON migration, owner-only files, wall-clock timestamps |
   | `crates/icx-adapters/tests/claude_contract.rs` | Claude Code `updatedToolOutput` shapes, MCP string replacement, PreCompact/PostCompact, SubagentStop |
   | `crates/icx-adapters/tests/hardening.rs` | FIFO and symlink `agy` transcript paths; bus file refusing symlinks and permissive pre-existing files |
   | `crates/icx-adapters/tests/mcp_bounds.rs` | MCP line and fold-depth bounds, protocol versions |
   | `crates/icx-bus/tests/loss_signalling.rs` | `SlotLapped`/`SlotContended`, `dropped_publishes`, `poll_since_with_loss`, two-handle file-backed propagation latency |
   | `crates/icx-codec/tests/codec_v110.rs` | Hex epoch ids, `SliceMap` validation, bounded clause chains |
   | `crates/icx-codec/tests/redact_v110.rs` | Redaction recall fixtures and losslessness |
   | `crates/icx-hdc/tests/recall_v110.rs` | Associative recall gate: recall@5 `>= 95%` at n = 300 |
   | `crates/icx-proxy/tests/proxy_auth.rs` | Every route requires the install token |
   | `crates/icx-proxy/tests/cache_placement.rs` | Anthropic breakpoint on a content block, client breakpoints kept within the 4-breakpoint limit, 1h TTL inheritance, byte-stable folded history, forged-marker neutralization |
   | `crates/icx-cli/tests/cli_bin.rs` | Binary entry point exit codes |

   A cross-agent test in `crates/icx-adapters/src/lib.rs` has one agent's `PostToolUse` publish
   an event that a second MMU in the same workspace receives in its tail slot and dereferences.

3. **Layer 3: Property tests and fuzzing**
   - **`proptest` (`crates/icx-codec/tests/properties.rs`):** secret-free `redact` is the
     identity; `redact` is idempotent; `sanitize` removes every delimiter; packets built from
     hostile field text render to exactly one packet and round-trip; clause parsing is total.
   - **cargo-fuzz (`fuzz/fuzz_targets/`):** `ahtl_stream`, `hdc_snapshot`, `mcp_jsonrpc`,
     `redact`. The `fuzz/` crate is excluded from the workspace and needs a nightly toolchain.
     Three initial v1.2.0 parser campaigns completed 900+ seconds. Redactor campaigns found
     additional idempotence and UTF8 counterexamples; failing artifacts are retained as
     regressions and renewed qualification remains open. See the dated logs in
     [the release evidence](releases/v1.2.0/evidence/). Fuzzing is not part of CI.

4. **Layer 4: Long-horizon simulations (synthetic)**
   - A 500-turn simulation (`sprint_5_1_500_turn_two_million_token_nih_oolong_and_prefix_cache_simulation`
     in `crates/icx-adapters/src/lib.rs`, more than 8 MB of generated tool output with heavy
     shared filler) plants passkeys at four turns and asserts that the active window stays under
     32,768 estimated tokens. This historical cache fixture explicitly configures a131,072-token
     total window to retain its large pinned head; it also asserts the mean byte-prefix ratio over turns that do not seal
     an epoch is `>= 94%`, and that every passkey is found by associative recall and dereferenced
     exactly.
   - `icx benchmark` runs a 50-turn synthetic transcript and gates on byte-weighted prefix reuse
     over **all** turns (`>= 90%`). Current JSON reports and repeated-process measurements
     identify build flags, fixtures and dispersion rather than treating historical timings as current.
   - Both figures are byte-level proxies on generated input. They are not measurements of a
     provider's prompt-cache hit rate.

5. **Layer 5: Quality gates (CI)**
   - Format: `cargo fmt --all -- --check`
   - Zero-`unsafe` policy: workspace-wide `unsafe_code = "forbid"`, checked in
     `.github/workflows/ci.yml`
   - Lint: `cargo clippy --workspace --all-targets -- -D warnings`
   - Test: `cargo test --workspace`
   - Coverage (`>= 95%` lines): `cargo llvm-cov --workspace --all-targets --lcov --output-path lcov.info --fail-under-lines 95`
   - Docs: `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
   - Dependencies: `cargo deny --workspace check`
   - Benchmark SLOs: `cargo run --release -p icx-cli -- benchmark`

All CI jobs use the pinned `1.100.0` toolchain and SHA-pinned actions
([docs/build-and-tooling.md](build-and-tooling.md) §3).

## 2. v1.2.0 qualification additions

New integration fixtures cover YAML scalar and redaction counterexamples, Profile A framing,
independent numerical vectors, snapshot corruption and 03→04 upgrades, exact lexical candidates,
configured context budgets, strict MCP input, and daemon durable-ACK crash/reopen behavior.
The separately ignored 10k turn production test must be explicitly included during candidate
qualification; ordinary workspace tests alone do not establish this result.

Mutation samples are deliberately limited. The latest reconciled sample has 135 executions:
80 caught, 43 missed, 7 unviable and 5 timeouts. Directed experiments and later tests are separate;
they are not added as disjoint coverage. [Mutation triage](releases/v1.2.0/mutation-triage.md)
records survivors, meaningful oracle gaps, equivalents and follow-up evidence. Passing line
coverage or adjudication does not prove exhaustive defect detection.
