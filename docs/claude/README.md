# Infinity-Context (`icx`) v1.0.0 — Adversarial Audit

- **Auditor:** Claude (Opus 5.5) in Claude Code, at the maintainer's request.
- **Date:** 2026-11-18.
- **Subject:** branch `claude-audit` at `c176d03` (v1.0.0). The audit changed no production code.

Start with [`00-executive-summary.md`](00-executive-summary.md).

## Documents

| Doc | Workstream |
| --- | --- |
| [00-executive-summary.md](00-executive-summary.md) | Top risks, claim-vs-reality scorecard, prioritized remediation |
| [01-ahtl-spec-conformance.md](01-ahtl-spec-conformance.md) | W1: AHTL PDF vs Markdown vs `icx-codec` |
| [02-integrity-and-lossless-claims.md](02-integrity-and-lossless-claims.md) | W2: CAS, DAG, persistence, "lossless" |
| [03-hdc-fractal-math.md](03-hdc-fractal-math.md) | W3: HDC, Fractal IFS, surprise gate, recall |
| [04-bus-and-concurrency.md](04-bus-and-concurrency.md) | W4: seqlock telepathy bus |
| [05-security-findings.md](05-security-findings.md) | W5: threat model vs code, security defects |
| [06-agent-integration-verification.md](06-agent-integration-verification.md) | W6: Claude Code, Codex, agy, Hermes, MCP, provider caching |
| [07-performance-and-benchmarks.md](07-performance-and-benchmarks.md) | W7: SLO suite, real hook latency, profiling |
| [08-test-quality.md](08-test-quality.md) | W8: coverage, mutation testing, missing test classes |
| [09-docs-citations-governance.md](09-docs-citations-governance.md) | W9: cross-doc consistency, citations, licensing |
| [10-ci-supply-chain.md](10-ci-supply-chain.md) | W10: workflows, pinning, dependencies |
| [11-enhancements-and-optimizations.md](11-enhancements-and-optimizations.md) | Forward-looking improvements |
| [12-v1.1.0-remediation-status.md](12-v1.1.0-remediation-status.md) | Per-finding status after the v1.1.0 release work |
| [findings.csv](findings.csv) | Every finding: ID, severity, location, evidence type (ticket-ready) |

## Method

1. **Baseline.** Before any analysis I ran every gate CI runs, exactly as CI runs it.
2. **Claim extraction.** I listed every quantitative and normative claim across `docs/`,
   `docs/ahtl/` (including the 58-page PDF, extracted with `pdftotext`), `ref-docs/`, README,
   CHANGELOG and ROADMAP.
3. **Verification.** Each claim was checked against the code at a cited line. Wherever possible it
   was also checked against a *reproduction*: a PoC test or binary run against the workspace
   crates, a measurement with the release binary, a `perf` profile, or a mutation run.
4. **Upstream checks.** External claims (agent hook APIs, MCP, provider prompt caching, arXiv
   citations, licenses) were checked against current upstream documentation and the locally
   installed agent binaries. Research subagents did that work, and I re-checked the
   highest-impact claims against primary sources myself.
5. **Severity.**

   | Level | Meaning |
| --- | --- |
   | **Critical** | Breaks the product's core promise in normal use |
   | **High** | A security defect with a realistic path, or a headline claim that is false |
   | **Medium** | A real defect or misstatement with bounded impact |
   | **Low** | Hardening, latent defects, minor inaccuracies |
   | **Info** | Dead code or a note |

6. **Leads rejected.** Pre-audit leads that failed verification are recorded as corrections, not
   findings:
   - the slice-hint packet-level round trip actually works;
   - the cache-hit filter does not exclude seal turns;
   - the short-handle attack costs 2⁴⁸ work, not 2²⁴;
   - `ya29.` and `glpat-` tokens are redacted;
   - the arbitrary `transcriptPath` read cannot disclose content.

**Disclosure.** Security findings are described here at the level of defect, impact and fix.
Exploit-level reproductions are kept outside this public repository (see
`05-security-findings.md`).

## Baseline at `c176d03`

- **Toolchain:** rustc 1.100.0. The host builds with `-C target-cpu=native` (from the global cargo
  config).

| Gate | Result |
| --- | --- |
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `cargo test --workspace` | pass (45 tests) |
| `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps` | pass |
| `cargo llvm-cov … --fail-under-lines 95` | pass: 95.75% lines; production-only 95.78% |
| `icx benchmark` × 10 | all PASS (medians in `07-performance-and-benchmarks.md`) |
| `icx doctor` | READY (informational probes only) |
| `cargo audit`, `osv-scanner` | 0 advisories (30 crates) |
| `cargo deny check` | licenses FAIL: no `deny.toml` exists |
| `uvx reuse lint` | pass (468 of 468 files) |

## Tool versions

| Tool | Version |
| --- | --- |
| cargo-llvm-cov | installed |
| cargo-mutants | 27.1.0 |
| cargo-geiger | 0.14.0 |
| cargo-semver-checks | 0.51.0 (n/a: unpublished) |
| cargo-msrv | 0.19.6 (n/a: MSRV = stable) |
| cargo-machete | 0.10.1 |
| cargo-audit, cargo-deny, cargo-nextest, cargo-outdated | installed |
| osv-scanner | 2.7.0 |
| semgrep | 1.186.0 (`p/rust`, `p/secrets`) |
| gitleaks | (CI pins 8.30.1) |
| typos-cli | 1.52.2 |
| lychee | 0.24.2 |
| markdownlint-cli2 | installed |
| perf | installed |
| pdftotext | installed |

Agent versions used for the integration checks: Claude Code 2.1.328, codex-cli 0.170.0,
agy 1.5.11, Hermes source `dc394c6`.

## Not done, and why

- **No live provider API calls.** Real cache-read tokens were not measured; that needs API keys
  and a recorded session (ICX-AUD-002, -003).
- **No live agent sessions.** Hook-output effects were verified against documentation and code,
  not by running the agents. Items that need a live test are marked *(inferred)* in doc 06.
- **No crash-injection test** of the non-atomic save (ICX-AUD-047), and no dynamic Hermes forgery
  PoC (ICX-AUD-011). Both rest on reading the cited code.
- **Mutation testing covered `icx-codec` only** (756 mutants). Other crates are a recommended
  next step.
- **Anthropic tokenization.** Delimiter BPE alignment for Anthropic models was not measured.

## Finding ID note

IDs run from ICX-AUD-001 to ICX-AUD-055. IDs 029 and 043–045 were reserved during drafting and
merged into other findings (029 into 031; 043–045 into the "Positive results" sections). There
are 51 findings in total.
