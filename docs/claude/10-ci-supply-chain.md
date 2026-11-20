# 10 — CI/CD and Supply Chain

Workstream W10. Files: `.github/workflows/{ci,cla,reuse,antigravity-review}.yml`,
`rust-toolchain.toml`, `codecov.yml`, `.gitleaks.toml`, `Cargo.lock`.

## What the gates actually run (`ci.yml`)

| Job | Steps | Gating |
| --- | --- | --- |
| `check` | fmt check; grep gate for `unsafe` (`:33-52`); `clippy -D warnings`; `cargo test`; docs with `-D warnings`; `icx benchmark` (release, 3× CI factor); `icx doctor`; self-tests of the review and CLA scripts | All blocking; no `continue-on-error` anywhere |
| `coverage` | `cargo llvm-cov --all-targets --fail-under-lines 95`; Codecov upload (`fail_ci_if_error` only when a token is present, `:91`) | Blocking. The audit confirmed the 95% holds for production code alone (`08-test-quality.md`) |
| `secrets` | gitleaks 8.30.1, pinned and SHA-256 verified, including a merge-commit fixture | Blocking; exemplary |

## ICX-AUD-040 — Pinning and policy gaps (Low)

1. **Actions pinned by mutable tag, not SHA:**
   - `actions/checkout@v7` (`ci.yml:26,75,103`; `cla.yml:47`; `reuse.yml:26`)
   - `dtolnay/rust-toolchain@stable` (`ci.yml:27,76`)
   - `Swatinem/rust-cache@v2`
   - `codecov/codecov-action@v7`
   - `fsfe/reuse-action@v6`

   `antigravity-review.yml` already pins SHAs (`:93`, `:168`), so the pattern exists in the repo.
   Pin by SHA with a `# vX` comment and let Dependabot bump them.

2. **`taiki-e/install-action@cargo-llvm-cov`** (`ci.yml:83`) installs the *latest* cargo-llvm-cov
   on every run. A new release can redden unchanged code with no commit to blame. Pin the tool
   version (`tool: cargo-llvm-cov@<x.y.z>`).

3. **Toolchain:** `rust-toolchain.toml` uses `channel = "stable"` and CI uses
   `rust-toolchain@stable`, while `rust-version = "1.100"`. Every new stable release can introduce
   clippy-pedantic warnings, which `-D warnings` turns into failures. Pin an exact toolchain in
   `rust-toolchain.toml`, and add a separate, non-blocking "latest stable" canary job.

4. **No `deny.toml`:** `cargo deny check` fails its license stage because no license policy is
   configured, and no CI job runs `cargo deny` or `cargo audit`. Add a `deny.toml` with an
   allow-list (MIT, Apache-2.0, Unicode-3.0, CC0-1.0, Unlicense, MIT-0, the LLVM exception) and run
   `cargo deny check` in CI. This also catches the GPL/AGPL declaration drift (ICX-AUD-034).

5. **Near-vacuous `doctor` CI step:** CI has no `.icx` workspace and no agents installed, so the
   only failure `doctor` can report there is a non-writable `/dev/shm`. Replace it with a test
   that creates a workspace, ingests, and runs `doctor` against it, Merkle verification included.

6. **Benchmark gate tolerance:** with the 3× CI factor on top of single-run timings, the gate
   cannot catch regressions smaller than about 3×. Use `criterion` baselines on a fixed runner,
   or gate only on counts and ratios in CI and track timings separately.

## ICX-AUD-041 — Unused dependencies (Low)

`cargo machete` flags `icx-codec` and `serde` in `crates/icx-proxy/Cargo.toml`, and `serde` in
`crates/icx-cli/Cargo.toml`. Manual check: neither crate references these. Remove them.

## Self-hosted review runner (`antigravity-review.yml`)

- **Trust model:** it runs on a self-hosted runner labelled `agy` (the maintainer's machine),
  triggered by `pull_request` and `issue_comment`.
- **Fork gating:** for `pull_request`, the workflow file comes from the PR branch, so its own
  fork-gating `if:` condition can be edited out by the PR. The effective control is the
  repository setting that requires approval for workflow runs from outside contributors, as the
  file acknowledges (`:31-38`). Verify that the setting is "require approval for **all**
  external contributors".
- **Main path:** only default-branch scripts run (`persist-credentials: false`). The review agent
  is invoked with `--sandbox --dangerously-skip-permissions`, with `GH_TOKEN`
  (`pull-requests: write`) in its environment, and it reads the untrusted diff. Prompt injection
  through the diff is therefore the main residual risk to the maintainer's machine. Keep the
  token's scope minimal and the sandbox strict, and consider a dedicated, unprivileged runner
  account.
- **Bootstrap step** (`:113-168`): it runs PR-head scripts only for same-repository branches,
  after a live check that the author has write permission. Acceptable.

## Positive results

- gitleaks is version-pinned and checksum-verified.
- No `continue-on-error`; every blocking job really blocks.
- The coverage threshold is real and holds for production code alone.
- `cargo audit` and `osv-scanner` find no advisories in the 30-crate lockfile; the dependency
  surface is small (blake3, serde, serde_json, thiserror).
- The CLA workflow uses `pull_request_target` safely (default-branch checkout).
