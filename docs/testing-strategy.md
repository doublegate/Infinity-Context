# Testing & Verification Strategy

**References:** Master-Core Module 20 (Test-Driven Accuracy), [docs/performance.md](performance.md)

---

## 1. Four-Layer Verification Pyramid

1. **Layer 1: In-Module Unit & Algebraic Invariant Tests (`cargo test --workspace`)**
   - **`icx-core`:** FastCDC chunk determinism, BLAKE3 CAS round-trip (`put_bytes` -> `deref_bytes` -> byte-for-byte equality), `deref_lines` exact boundary slicing, `grep_cold` accuracy, `MerkleContextDag` epoch sealing, and integrity-tamper detection (`IcxError::IntegrityViolation`).
   - **`icx-hdc`:** 10,240-bit `Hypervector` orthogonality (`sim(A, B) < 0.05` for distinct seeds), XOR self-inverse (`(A ^ B) ^ B == A`), permutation invertibility, Delta-Rule bundling convergence, `FractalCoord` suffix depth theorem (`shared_suffix_depth` matches exact common suffix length), and `TitansSurpriseGate` momentum response.
   - **`icx-codec`:** Braille-256 bijection (`0..=255`), `AhtlPacket` render/parse round-trip, delimiter-spoofing neutralization (`⟪`/`⟦` escaping), `SecretRedactor` coverage across all credential formats, and `>= 12x` compression ratio on realistic build/test logs.
   - **`icx-store`:** Bi-Temporal functional edge invalidation (`t_invalid` stamped when contradicted) and HippoRAG Personalized PageRank multi-hop convergence.
   - **`icx-bus`:** Lock-free `SeqlockRing` concurrent publish/read consistency across threads and processes.
   - **`icx-adapters`:** Exact wire-JSON schema conformance for Claude Code (`updatedToolOutput`, `PreCompact`), Codex (`updatedMCPToolOutput`, `additionalContext`), Antigravity CLI (`injectSteps` with `user_markdown: ""`, `Stop` continuation), Hermes (`select_context`), and MCP (`icx` polymorphic tool).

2. **Layer 2: Cross-Crate Integration & Multi-Agent Telepathy Suite (`tests/`)**
   - Simulates Claude Code discovering a compiler panic (`PostToolUse` -> `AHTL` stub + CAS commit + `/dev/shm` publish) and verifies that an Antigravity CLI (`PreInvocation`) and Codex (`UserPromptSubmit`) session running in the same workspace immediately receive the `⟦Θ:...⟧` telepathy frame in their Ephemeral Tail Slot and can losslessly dereference `#icx:<hash>` via the MCP `icx` tool.

3. **Layer 3: Long-Horizon Needle-in-a-Haystack & Prefix-Cache Simulation**
   - Simulates a 500-turn trajectory (> 2,000,000 raw tokens of tool outputs and code reads) with planted hex passkeys and line numbers at turns 3, 47, 189, and 412.
   - Asserts:
     - Active window stays strictly under the configured token ceiling (e.g., 32,768 tokens).
     - Prompt prefix-cache hit ratio across all 500 turns is `>= 94.0%`.
     - Every planted passkey is retrieved into the Ephemeral Tail Slot via HDC + PPR recall and 100% losslessly dereferenced via `#icx:<hash>`.

4. **Layer 4: Quality Gates**
   - Format gate: `cargo fmt --all -- --check`
   - Zero-`unsafe` policy gate: workspace-wide `unsafe_code = "forbid"` enforced across all 8 crates in `.github/workflows/ci.yml`
   - Lint gate: `cargo clippy --workspace --all-targets -- -D warnings`
   - Test gate: `cargo test --workspace`
   - Coverage gate (`>= 95%` floor): `cargo llvm-cov --workspace --all-targets --lcov --output-path lcov.info --fail-under-lines 95`
   - Doc gate: `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
