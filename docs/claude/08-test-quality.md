# 08 — Test Quality

Workstream W8.

## Inventory

- **Count:** 45 `#[test]` functions in total, and `cargo test --workspace` passes all of them.
  - Most are large multi-assert "sprint" scenarios: `icx-proxy` has a single test starting at
    `wire.rs:2386`, and `icx-adapters` has six tests in about 2,000 lines of `lib.rs`.
  - Per file the counts are 1–6 (`braille.rs` 3, `redact.rs` 2, `ahtl.rs` 2, …).
- **No property tests, no fuzzing, no benchmark harness:**
  - No `proptest`, `quickcheck`, `arbitrary`, `criterion` or `divan` dependency.
  - No `fuzz/` directory and no `benches/`.
  - No `[dev-dependencies]` in any manifest.
  - The "10,000 random" Braille test uses an internal PRNG.
- **One integration test:** `crates/icx-cli/tests/cli_bin.rs` contains a single 22-line test.
- **Coverage:** `cargo llvm-cov` reports **95.75%** lines.
  - Excluding `#[cfg(test)]` regions, production-only line coverage is **95.78%** (408 of 9,678
    lines missed), so the ≥95% claim holds and test code does not inflate it.
  - Lowest production coverage: `icx-proxy/src/wire.rs` 87.8% and `icx-cli/src/main.rs` 89.7%.

## ICX-AUD-039 — High coverage, weak oracles (Medium)

Coverage counts executed lines; it does not tell whether a wrong result would be noticed.
Mutation testing measures that.

- **Run:** `cargo mutants -p icx-codec` on `redact.rs`, `ahtl.rs` and `braille.rs`, 756 mutants,
  60 s timeout, `-j 4`.

| File | Caught | Missed | Mutation score |
| --- | --- | --- | --- |
| `icx-codec/src/braille.rs` | 70 | 9 | 88.6% |
| `icx-codec/src/ahtl.rs` | 102 | 34 | 75.0% |
| `icx-codec/src/redact.rs` | 338 | **158** | **68.1%** |
| **Total** (plus 31 timeout, 13 unviable) | 510 | 201 | **71.7%** |

- **Representative surviving mutants** (each is a behaviour no test pins):
  - **PEM/PGP block termination** (`redact.rs:30`, `:36`, `:42`): `&&` → `||` in the END-marker
    checks survives. This is the code behind ICX-AUD-016.
  - **Entropy-gate thresholds** (`redact.rs:704`, `:707`): `&&` → `||` and `>` → `>=` in
    `is_high_entropy_secret_candidate` survive, so the documented 4.35 / 3.50 thresholds and the
    24/32-character minimums are not tested at their boundaries.
  - **Clause escaping** (`ahtl.rs:171-183`): the conditions in `escape_clause_field` can be
    inverted without failing a test, so the v1.0.0 "bijective clause escaping" is not pinned.
  - **`redact_line` boundaries** (`redact.rs:185`, `:197`).
- **Weak or tautological assertions (examples):**
  - `ring.rs:1104` asserts `total_observed > 0` after an 8-writer stress run, which cannot detect
    event loss (ICX-AUD-054).
  - `hv.rs:801` asserts a digest *collision* between the all-zero and all-ones vectors, encoding a
    weakness as expected behaviour (ICX-AUD-053).
  - `main.rs:1296` (`cli_subcommands_…_run_cleanly`) only checks that `doctor` and `benchmark`
    return `Ok`.
  - The ≥94% assertions in `epoch.rs` and `crates/icx-adapters/src/lib.rs` inherit the padded
    fixture (ICX-AUD-002).
  - One test asserts trailing-newline loss (`assert_eq!(deref_out, panic_log.trim_end_matches('\n'))`),
    encoding a lossless-claim violation as expected behaviour (ICX-AUD-021).
  - The fractal benchmark asserts a depth that holds by construction (ICX-AUD-052).
- **Missing test classes:**
  - Agent contract fixtures (ICX-AUD-004 to ICX-AUD-008).
  - Real-binary end-to-end hook latency (ICX-AUD-001).
  - Multi-process bus tests (ICX-AUD-055).
  - Adversarial parser inputs (ICX-AUD-025, ICX-AUD-026).
  - Redactor recall corpus (ICX-AUD-015).

## Recommendations

1. **Split the mega-tests.** One behaviour per test, named for the behaviour. That makes the
   suite readable, and it makes failures point somewhere.
2. **Property tests (`proptest`):**
   - `deref(ingest(x)) == x` on secret-free inputs;
   - packet, stream, epoch-header and Braille round-trips;
   - `escape` / `unescape` inverse;
   - the redactor never lengthens a line by more than its replacement token;
   - bus `observed + gaps == published`.
3. **Fuzz targets (`cargo fuzz`):** `AhtlStream::parse_stream`, `WirePacket::parse_auto`,
   `EpochHeader::parse`, `SecretRedactor::sanitize`, the hook JSON dispatchers, the MCP request
   handler, and the proxy's HTTP/SSE parser. Gate: zero crashes over a fixed corpus in CI.
4. **Mutation gate:** run `cargo mutants --in-diff` on PRs that touch `icx-codec`, `icx-core` or
   `icx-adapters`. Target ≥ 85% on `redact.rs` and `ahtl.rs`.
5. **Benchmarks:** `criterion`/`divan` with percentiles, real-binary hook latency, and recall@k
   (ICX-AUD-049).
6. **Golden contract fixtures** captured from each pinned agent version.

## Toolchain posture

- `rust-version = "1.100"` equals the current stable (1.100.0), and `rust-toolchain.toml` pins
  `channel = "stable"`, not a version. The MSRV is therefore "latest stable": it is never tested
  independently, and a new stable release can change lints under unchanged code (see
  ICX-AUD-040). `cargo msrv verify` adds no information while MSRV equals stable.
- `cargo semver-checks` is not applicable yet, because the crates are not published. Once they
  are, add it to the release ceremony, since v1.0.0 implies API stability.
