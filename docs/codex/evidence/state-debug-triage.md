# Debug triage: redactor fuzz assertion and scratch exhaustion

## Redactor fuzz exit 77

**Failure type:** semantic test assertion, not an AddressSanitizer memory-safety report.

**Evidence:** `validation-fuzz-redact.log` identifies `fuzz/fuzz_targets/redact.rs:10`, whose oracle compares `redact(redact(input))` with `redact(input)`. The panic is in that assertion; the production redactor returns normally. LibFuzzer converts the panic into an abort and exit 77. It saved the synthetic input as `crash-c7f6a05aa7d4b3a1a3c1a1651d89bdb9c753e319`.

**Independent reproduction:** `state-debug-redact.rs` includes the exact current production redactor source (only std dependencies), reproduces the original mismatch, and greedily removes byte ranges while retaining the mismatch. Compile/run commands, both exiting zero:

```bash
rustc --edition=2024 docs/codex/evidence/state-debug-redact.rs \
  -o /tmp/Infinity-Context/state-debug-redact
/tmp/Infinity-Context/state-debug-redact \
  /tmp/Infinity-Context/validation/fuzz/redact/artifacts/crash-c7f6a05aa7d4b3a1a3c1a1651d89bdb9c753e319
```

The `#[path]` absolute checkout prefix needs adjustment on another machine. The compiler warns that the standalone probe does not call the production `sanitize`/delimiter helper functions; no warnings were suppressed or production code changed. Exact output is retained in `state-debug-redact.log`.

Minimal independent vector:

```text
input  = "TOKEN:\nPsk-OyyyTOKEN:"
once   = "TOKEN:\nP[REDACTED:API_KEY]:"
twice  = "TOKEN:\n[REDACTED:SECRET]"
```

**Root cause:** the pending-sensitive-value state first sees an identifier-shaped second line ending in `:` and classifies it as a nested structure (`ContinuationLine::classify`, `redact.rs:928`). The vendor-prefix pass then inserts a bracketed marker into that apparent key. On the second whole-text pass, the transformed line is no longer identifier-shaped and is classified as a scalar value of the preceding `TOKEN:` key, so it is replaced more broadly. Classification is based on the current input while later passes can change its lexical structure.

**Classification:** a real idempotence/canonicalization defect with potential extra redaction and content-identity changes on repeated processing. This vector does not prove that an actual secret survives the first pass, that the library panics on the input, or that an attacker crosses a privilege boundary. Report it as semantic correctness, with a regression oracle preserving the stated idempotence contract. The codec workstream owns the finding identifier and recommendation.

## ENOSPC during expanded mutation runs

**Failure type:** experiment infrastructure/resource failure, independent of the fuzz assertion.

Read-only measurements found `/tmp` mounted as a 32 GiB tmpfs, 97% full with approximately 1.1 GiB free. The audit's validation scratch occupied about 14 GiB. Five active `cargo-mutants-Infinity-Context-*.tmp` source copies each contained approximately 719 MiB of copied `fuzz/` content and 763 MiB of copied `patents/` content, in addition to their compiled targets. `validation-mutation-expanded-icx-core.log`, `...-icx-bus.log` and `...-icx-adapters.log` contain OS error 28. The audit does not treat those aborted builds as viable mutants or survived mutations.

**Minimal recovery advised to the validation owner:** preserve every crash artifact, corpus and mutation log/outcome, then remove only redundant **copied** `fuzz/target` build directories inside its own scratch mutation trees; the root workspace excludes `fuzz`, so those binaries are unnecessary for workspace mutation builds. Never remove the original checkout's `fuzz/target`, source assets, user files or unrelated `/tmp` trees. Further omission of copied ignored assets should use a minimal tracked-source scratch snapshot for future runs. The validation owner controls its jobs and cleanup; this debug worker performed no cleanup and killed no process.

**Prevention:** preflight scratch size and available tmpfs space, estimate copy amplification at chosen concurrency, omit unrelated ignored build trees/assets from isolated snapshots, and reserve space for logs/crash artifacts. Retain infrastructure-failed mutation runs separately from subsequent successful retries.
