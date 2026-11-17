# Contributing to Infinity-Context (`icx`)

## Development Workflow

1. Read `AGENTS.md` and `docs/architecture.md` before modifying any subsystem.
2. Reference the corresponding ticket ID from `to-dos/ROADMAP.md` (`T-<Phase><Sprint>-<NNN>`) in branch names and Conventional Commit messages.
3. Update the matching specification in `docs/` in the same change as any behavior change (docs-as-spec).
4. Run all quality gates before submitting a pull request:

   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   cargo doc --workspace --no-deps
   cargo run --release -p icx-cli -- benchmark
   uvx reuse lint
   ```

## Licensing & Contributor License Agreement (CLA)

All contributions to **Infinity-Context (`icx`)** are governed by the [Contributor License Agreement (`CLA.md`)](CLA.md), [`LICENSING.md`](LICENSING.md), [`PATENTS.md`](PATENTS.md), and [`COMMERCIAL.md`](COMMERCIAL.md), with open-source code licensed under `GPL-3.0-or-later` / `AGPL-3.0-or-later` ([`LICENSE`](LICENSE)) and checked for REUSE 3.3 compliance via [`REUSE.toml`](REUSE.toml).
