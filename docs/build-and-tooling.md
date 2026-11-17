# Build, Toolchain & CI Configuration

**References:** [docs/architecture.md](architecture.md), [docs/testing-strategy.md](testing-strategy.md)

---

## 1. Toolchain Requirements

- **Rust Toolchain:** Stable Rust `1.100+` (Edition `2024`, `rust-version = "1.100"`), with `clippy`, `rustfmt`, and `llvm-tools-preview` components.
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
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

# 6. Run workspace line coverage gate (>= 95% floor)
cargo llvm-cov --workspace --all-targets --lcov --output-path lcov.info --fail-under-lines 95

# 7. Run the built-in Infinity-Context benchmark & verification suite
cargo run --release -p icx-cli -- benchmark

# 8. Run system health & agent integration diagnostics
cargo run --release -p icx-cli -- doctor
```

---

## 3. Workspace Lints & Licensing (`Cargo.toml`)

- **License:** `GPL-3.0-or-later` (`GNU General Public License v3.0 or later`, see root `LICENSE`).
- All 8 workspace crates (`icx-core`, `icx-hdc`, `icx-codec`, `icx-store`, `icx-bus`, `icx-adapters`, `icx-proxy`, `icx-cli`) inherit strict lints from `[workspace.lints]` via `[lints] workspace = true`:
- `rust.unsafe_code = "forbid"` (enforced permanently across all 8 crates through `v1.0.0` and beyond; `std` is permitted, and zero `unsafe` code is allowed anywhere in the repository).
- `clippy.all = "warn"`
- `clippy.pedantic = "warn"`
