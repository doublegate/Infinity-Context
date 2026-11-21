# Build, Toolchain & CI Configuration

**Applies to:** `icx` v1.1.0  
**References:** [docs/architecture.md](architecture.md), [docs/testing-strategy.md](testing-strategy.md)

---

## 1. Toolchain Requirements

- **Rust toolchain:** pinned to `1.100.0` in `rust-toolchain.toml` (components `rustfmt`,
  `clippy`) and in every CI job. Edition `2024`, `rust-version = "1.100"`.
- **Native dependencies:** none beyond a C compiler. SQLite is compiled from source through
  `rusqlite` with the `bundled` feature.
- **Runtime dependency:** `icx proxy` sends upstream requests with the `curl` executable, which
  must be on `PATH`.
- **Platforms:** CI builds and tests on `ubuntu-latest` (x86-64 Linux). The daemon, Unix-socket
  and permission checks are Unix-specific (`#[cfg(unix)]`). Other platforms are not tested in CI.
  No CPU-specific instruction set (such as AVX-512) is required.

---

## 2. Canonical Workspace Commands

```bash
# 1. Check compilation across all workspace crates
cargo check --workspace --all-targets

# 2. Run the unit, integration and property test suites
cargo test --workspace

# 3. Strict Clippy lint gate (warnings are errors)
cargo clippy --workspace --all-targets -- -D warnings

# 4. Formatting
cargo fmt --all -- --check

# 5. Documentation
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

# 6. Line coverage gate (>= 95% floor)
cargo llvm-cov --workspace --all-targets --lcov --output-path lcov.info --fail-under-lines 95

# 7. Dependency advisories, licenses, bans and sources
cargo deny --workspace check

# 8. Synthetic benchmark and SLO gates
cargo run --release -p icx-cli -- benchmark

# 9. Local health and integration diagnostics
cargo run --release -p icx-cli -- doctor

# 10. Fuzzing (nightly toolchain and cargo-fuzz; fuzz/ is excluded from the workspace)
cargo +nightly fuzz run ahtl_stream    # also: hdc_snapshot, mcp_jsonrpc, redact
```

---

## 3. CI (`.github/workflows/`)

| Workflow / job | Runs |
| :--- | :--- |
| `ci.yml` main job | `cargo fmt --check`, zero-`unsafe` policy check, `cargo clippy -D warnings`, `cargo test --workspace`, `cargo doc` with `-D warnings`, `icx benchmark`, `icx doctor`, reviewer and CLA script self-tests |
| `ci.yml` `deny` | `cargo deny --workspace check` with `cargo-deny@0.21.0` and the policy in `deny.toml`; `cargo-about@0.9.3` generates the third-party notices, so a dependency whose license text cannot be collected fails here rather than at release time |
| `ci.yml` coverage | `cargo llvm-cov` (`cargo-llvm-cov@0.9.1`) with `--fail-under-lines 95`; uploads to Codecov |
| `ci.yml` secret scan | gitleaks (pinned version, SHA-256-verified download) over the git history |
| `reuse.yml` | REUSE 3.3 licensing lint |
| `cla.yml`, `antigravity-review.yml` | Contributor-agreement check and PR review automation |
| `release.yml` | On a `vX.Y.Z` tag push (or a manual run naming an existing tag): requires the tagged commit to be on `main` (so every `ci.yml` gate has run on it), checks the tag matches the `icx-cli` version and has a CHANGELOG section, runs `cargo test --workspace --locked`, generates `THIRD-PARTY-NOTICES.txt` with `cargo-about` (`.github/about.toml`), builds baseline and x86-64-v3 binaries for static musl and for glibc 2.35+ (built on Ubuntu 22.04), checks linkage and smoke-tests them, packages them with the license texts and notices, attests build provenance, and attaches them with `SHA256SUMS`. A new release is created as a draft with the CHANGELOG section as notes. Files already attached are never replaced; `SHA256SUMS` is only ever extended (`.github/scripts/release-assemble.sh`) |

Supply-chain rules (ICX-AUD-040):

- Every GitHub Action is pinned to a full commit SHA, with the release tag as a comment.
- Tools installed in CI are pinned to exact versions.
- `deny.toml` fails the build on RustSec advisories and yanked crates, and allows only licenses
  compatible with distribution under GPL-3.0-or-later / AGPL-3.0-or-later.

---

## 4. Workspace Lints & Licensing (`Cargo.toml`)

- **Licenses:** code is `GPL-3.0-or-later`; `icx-proxy` and `icx-adapters` are
  `GPL-3.0-or-later AND AGPL-3.0-or-later`. See `LICENSING.md` and `REUSE.toml`.
- All 8 workspace crates (`icx-core`, `icx-hdc`, `icx-codec`, `icx-store`, `icx-bus`,
  `icx-adapters`, `icx-proxy`, `icx-cli`) inherit `[workspace.lints]` via
  `[lints] workspace = true`:
  - `rust.unsafe_code = "forbid"` (no `unsafe` in first-party code; dependencies may contain it);
  - `clippy.all = "warn"` and `clippy.pedantic = "warn"`, with a documented allow-list, enforced
    as errors in CI through `-D warnings`.
