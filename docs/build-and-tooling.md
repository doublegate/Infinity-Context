# Build, Toolchain & CI Configuration

**Applies to:** `icx` v1.2.0 development candidate
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
| `release.yml` | On a `vX.Y.Z` tag push (or a manual run naming an existing tag): requires ancestry on `main` and successful required CI/REUSE jobs for the exact tagged SHA in trusted push runs. Missing, failed, cancelled or incomplete checks block release; ancestry alone is insufficient. It checks manifest/CHANGELOG agreement, tests the immutable SHA, generates third-party notices, builds baseline and x86-64-v3 Linux binaries for static musl and glibc 2.35+ (Ubuntu 22.04), verifies linkage and isolated smoke tests, then packages licenses/notices and provenance. The tag and checks are revalidated before publication. New releases are drafts; existing files are never replaced and checksums are only extended. Policy fixtures live in `.github/scripts/test-release-ci-gate.py`. |

**Selective runs on pull requests.** A `changes` job in `ci.yml` diffs the pull request and gates
the other jobs:

| Change touches | Jobs that run |
| :--- | :--- |
| `crates/`, `fuzz/`, `Cargo.toml`/`Cargo.lock`, `rust-toolchain.toml`, `.cargo/` | main job (all Rust steps), coverage |
| any Rust input above, a `Cargo.toml`, `Cargo.lock`, `deny.toml`, `.github/about.*` | `deny` |
| `scripts/`, `.github/scripts/`, `antigravity-review.yml`, `cla.yml`, `release.yml` | main job (self-tests only, no Rust toolchain) |
| anything, documentation included | secret scan, REUSE |
| `.github/workflows/ci.yml` | everything |

- **Jobs are skipped with a job-level `if`,** never a workflow `paths` filter. A skipped job counts
  as passing for required status checks, so the check names listed in the repository ruleset
  still report on every pull request.
- **Pushes to `main` and manual runs always execute every job.** The release gate requires every
  CI job on the tagged commit to conclude `success`, and a skipped job does not.
- **A weekly scheduled run** executes only `deny`, because new RustSec advisories appear without
  any change in this repository.
- **Rust build caches** (`Swatinem/rust-cache`) are saved only from `main`. Pull requests restore
  them but never write their own, which keeps the repository cache small and the `main` entry
  warm.

Supply-chain rules (ICX-AUD-040):

- Every GitHub Action is pinned to a full commit SHA, with the release tag as a comment.
- Tools installed in CI are pinned to exact versions.
- `deny.toml` fails the build on RustSec advisories and yanked crates, and allows only licenses
  compatible with distribution under GPL-3.0-or-later / AGPL-3.0-or-later.

### Documentation site (`docs.yml`)

The site at <https://doublegate.github.io/Infinity-Context/> is built with
[Zensical](https://zensical.org/) (pinned in the workflow) from `docs/` plus the root README,
CHANGELOG, PROJECT-STATUS, CONTRIBUTING, SECURITY and LICENSING pages. Pull requests that touch
the docs build it in strict mode, so a broken internal link fails the check; pushes to `main`
also deploy it to GitHub Pages. To build and preview locally, in a virtual environment (the
hash-locked requirements target Python 3.12 on x86-64 Linux; PEP 668 systems reject a bare
`pip install` outside one):

```bash
uv venv --python 3.12 .venv && . .venv/bin/activate   # or: python3.12 -m venv .venv
uv pip install --require-hashes -r .github/docs-requirements.txt   # or: pip install ...
python3 scripts/build_docs_site.py
zensical build --clean --strict -f .site-src/zensical.toml   # output in .site-src/site
zensical serve -f .site-src/zensical.toml                    # preview on localhost:8000
```

Math is rendered by MathJax 3.2.2, self-hosted rather than loaded from a CDN: staging downloads
the npm package (network access needed), checks it against a pinned SHA-512, and copies only
the script, its fonts and its Apache-2.0 license into the site.

`scripts/build_docs_site.py` copies the pages into `.site-src/` without changing the sources:
links to files outside `docs/` point at GitHub, images outside `docs/` are copied in, and the
navigation lists every page (curated order first, so a new document appears without editing
the configuration).

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
