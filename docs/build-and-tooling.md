# Build, Toolchain & CI Configuration

**References:** [docs/architecture.md](architecture.md), [docs/testing-strategy.md](testing-strategy.md)

---

## 1. Toolchain Requirements

- **Rust Toolchain:** Stable Rust `1.85+` (Edition 2021 / 2024 compatible), pinned via `rust-toolchain.toml` with `clippy` and `rustfmt` components.
- **Target Platforms:**
  - Primary: `x86_64-unknown-linux-gnu` and `x86_64-unknown-linux-musl` (AVX2 / AVX-512 `VPOPCNTDQ` auto-vectorization enabled on host CPU).
  - Secondary: `aarch64-apple-darwin` (Apple Silicon M-series NEON) and `aarch64-unknown-linux-gnu`.

---

## 2. Canonical Workspace Commands

```bash
# 1. Check compilation across all workspace crates
cargo check --workspace --all-targets

# 2. Run the full unit and integration test suite
cargo test --workspace

# 3. Run the strict Clippy lint gate (warnings treated as errors)
cargo clippy --workspace --all-targets -- -D warnings

# 4. Verify formatting
cargo fmt --all -- --check

# 5. Build documentation
cargo doc --workspace --no-deps

# 6. Run the built-in Infinity-Context benchmark & verification suite
cargo run --release -p icx-cli -- benchmark

# 7. Run system health & agent integration diagnostics
cargo run --release -p icx-cli -- doctor
```

---

## 3. Workspace Lints & Licensing (`Cargo.toml`)

- **License:** `GPL-3.0-or-later` (`GNU General Public License v3.0 or later`, see root `LICENSE`).
- All workspace crates inherit strict lints from `[workspace.lints]`:
- `rust.unsafe_code = "forbid"` (overridden to `"warn"` only in `icx-bus` for POSIX `/dev/shm` `mmap` seqlock operations with mandatory `// SAFETY:` comments).
- `clippy.all = "warn"`
- `clippy.pedantic = "warn"`
