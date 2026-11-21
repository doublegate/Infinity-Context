# Codec and retrieval reproduction

All inputs are synthetic. No agent configuration, credentials, production source, memory store, or reference PDF is modified. Run from repository root with Rust 1.100.0 and cached Cargo dependencies.

1. Make `/tmp/Infinity-Context/codec-audit/src`.
2. Copy `codec-probe.rs` there as `src/main.rs`.
3. Create a standalone Cargo manifest with package name `icx-codec-audit`, edition `2024`, and path dependencies on this checkout's `crates/icx-core`, `crates/icx-codec`, `crates/icx-hdc`, `crates/icx-store`. Do not add it to the production workspace.
4. Set `CARGO_TARGET_DIR=/tmp/Infinity-Context/codec-audit/target` and run `cargo run --release --offline --manifest-path /tmp/Infinity-Context/codec-audit/Cargo.toml` for codec/math probes. The unused `icx-store` dependency warning in the first harness version concerns the isolated harness only.
5. Run the resulting executable with `recall` for deterministic corpus scaling. Report the CSV rows verbatim; these timings share the host with other audit jobs and are not an isolated benchmark or portable/native comparison.
6. Use `multilingual` for the additional nine rows: six non-ASCII vocabulary-prefix families and Chinese-prefixed identifiers. This mode emits process VmRSS/VmHWM to stderr; preserve both streams. `codec-Cargo.lock` records the resolved isolated harness dependency versions.

The retrieval harness uses xorshift seeds 1,17,97, 120 tokens per document, random unique-needle position, and 100 evenly spaced queries per corpus. Repetitive vocabulary has 20 words; varied vocabulary has 100,000 possible words. Subjects have predictable `Bash:cargo test N` form and do not contain the needles. The lexical baseline searches full token equality over saved original strings. The exhaustive baseline requests all `n` hits from the production API, which forces the same scoring function to skip candidate pruning. It is an exhaustive version of the shipped **composite** scorer, not an HDC-only oracle. `snapshot_bytes` measures serialized index bytes, not RSS.

Default mode also emits a three-node PPR vector for1,2,6 iterations. Compare with `python3 docs/codex/evidence/math-ppr-oracle.py`; the independent dense transition matrix is derived from the public forward/half-reverse graph contract, not imported from production CSR arrays.

For the fuzzer idempotence counterexample, invoke the built harness as `icx-codec-audit redact <fixture-path>`. It prints input/once/twice and intentionally exits2 when output changes. `codec-redact-minimize.py <binary> <original-artifact>` checks this exact exit code, delta-debugs the original synthetic bytes and writes hash/hex/replay evidence. `codec-redact-cli.py <icx-binary>` independently demonstrates the related equals-containing YAML password leak in an isolated real workspace. Production functions return normally; the original fuzzer exit77 comes from its property assertion.

PDF procedure: `pdfinfo`, `sha256sum`, `pdftotext -layout`, then split text on form-feed and enumerate 58 nonempty pages. `pdftoppm -f 30 -l 30 -scale-to 1500 -png -singlefile` and the equivalent for page35 generated the visually inspected equations/code pages. Extracted text and rendered images remain under the isolated directory rather than duplicating the 19MB reference PDF in evidence.

Independent canonical digest: Python `blake3.blake3(b'raw_payload_bytes').hexdigest()`; this uses a separate language binding, though the same hash algorithm/library lineage, not a separately implemented cryptographic oracle.
