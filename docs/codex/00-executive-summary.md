# Executive assessment

Revision `a996d59f1eb468e7c386747e15f4c7e58d08410e` has materially stronger integrity and workspace
isolation than the v1.0.0 baseline. It is **not ready for an unqualified claim of unlimited,
reliable context retrieval or complete credential scrubbing**. Preserve its useful exact
post-redaction CAS capability, while addressing the specific failures below before extending
those product guarantees.

The [findings register](10-findings-register.md) contains **19 findings: 1 High, 10 Medium and 8 Low**.
Governance questions, specification contradictions and library robustness gaps are separated
from demonstrated security failures. No Critical remote exploit is claimed.

## Principal risks and evidence

| Priority | Finding | Independently demonstrated result |
| --- | --- | --- |
| P1 | [004: corpus-growth recall](02-mathematics-and-retrieval.md) | At 10k varied artifacts, recall@5 is 28–36% across 3 seeds; exact lexical baseline 100%, exhaustive composite 97–99%. Multilingual@5 is 25–38%. Root separately repeated the 33% row |
| P1 | [001: YAML block scalar credentials](01-ahtl-conformance.md) | Literal/folded password-value lines survive redaction. Root real-CLI `init`→`encode-ahtl`→`deref --raw` confirms both synthetic credential lines persisted |
| P1 | [030: daemon acknowledgment and failed flush](03-integrity-persistence-security.md) | A successful handle response can be lost on pre-flush SIGKILL. Injected final-save failure also returns successful response and successful daemon exit; root independently reproduced both |
| P1 | [070: release gate](07-supply-chain-governance.md) | Tag ancestry on main and passing tests do not establish successful lint/coverage/advisory/secret/REUSE checks on that exact commit |
| P2 | [005: coordinate scale](01-ahtl-conformance.md) | Normative fc Q1.14 and shipped Q15 codec differ by roughly 2×; self-round-trips cannot detect interoperability disagreement |
| P2 | [101: context budget](09-context-budget-and-enhancements.md) | Default controller retains 996 sealed frames and an 86,880-byte prefix after 1,000 synthetic turns. Budget retargeting exists, but production callers do not invoke it automatically |

Other register entries cover public Profile A framing, nonfinite snapshots, MCP validation,
sequence invariants, extreme public configuration, unsupported Codex output fields, weak test
oracles, outdated accepted specifications and qualified governance/provenance questions.

## Strengths that survived adversarial review

- Default CLI workspace is outside the repository, private, owner-sealed and store-MAC checked.
  Repository-local `.icx` is not silently adopted. Same-user processes remain trusted.
- Checked CAS insertion compares full digests and refuses collisions rather than overwriting.
  Independent CRLF/trailing-newline/delimiter/multilingual fixtures recover exact stored bytes.
- SQLite replaces the former multi-file transaction, and HDC snapshots/daemon avoid re-encoding
  the whole index per hook. Transaction atomicity is separate from durable acknowledgment.
- Existing clause-depth/input bounds, loopback/token proxy controls, no-follow file checks,
  SHA-pinned actions and dependency policy are meaningful improvements.
- Current README/numbers distinguish skeleton-plus-handle byte ratios and synthetic prefix
  reuse from lossless inline encoding and actual provider cache-hit measurements.

## Validation and interpretation

Format, strict Clippy, warning-free rustdoc and host workspace tests pass. The host coverage
run records 18,925/19,890 lines (**95.1483%**). Initial sandbox test/coverage failures were socket
permission failures and are retained separately. Exact CI-version gitleaks passes; an older
installed version's historical synthetic-fixture flags are not reported as real credentials.
Dependency, notice, REUSE and workflow checks are detailed with exact exits in report 06.

Strict benchmark SLO checks fail in most portable/native repetitions on the active shared
host. Concurrent audit jobs limit timing attribution; these failures do not establish a quiet
host regression or a claimed optimization. They also cannot be relabeled as passing gates.
Fuzzing independently exposed non-idempotent redaction (006): the property assertion aborts,
while production redaction returns normally. Fuzz and sampled all-crate mutation outcomes,
including crashes/survivors/timeouts and incomplete
partitions, are recorded in report 06 and its evidence rather than hidden behind line coverage.

## Readiness and next work

Prioritize redaction correctness, representative recall, explicit durability/failure reporting
and release-check enforcement. Then settle fc wire migration and automatic model-window
budgeting before changing schemas or APIs. The [roadmap](11-remediation-roadmap.md) specifies
dependencies, effort, compatibility effects and verification criteria.

This is an inventoried, evidence-backed audit with the [limits in report 14](14-coverage-ledger.md),
not a proof that all code, claims, platforms or legal rights are correct. All 51 prior finding rows
were independently reassessed at their stated verification depth. No production fix, commit,
push, merge, publication, live-agent configuration or user-memory change was made.
