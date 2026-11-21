# Supply chain, release infrastructure, and governance

Audited source: `a996d59f1eb468e7c386747e15f4c7e58d08410e`. Audit access date: 2026-11-21 UTC (2026-11-20 local). No release or production configuration was modified.

## Controls and boundaries

All five workflow files were inspected: CI, REUSE, CLA, Antigravity review, and release. Actions are pinned to full commit hashes. Rust is pinned to 1.100.0. CI installs versioned cargo-deny, cargo-about, and llvm-cov. Gitleaks download is versioned and checksum verified; history scan includes merge patches. `deny.toml` configures advisory, source, license, and wildcard policy. `cargo-about` notices include build dependencies and exclude development dependencies. SQLite is bundled and musl copyright is separately packaged.

The release build checks static musl linkage or a glibc 2.35 symbol ceiling. Baseline and x86-64-v3 builds are distinct. Packaging fixes archive owner, ordering, timestamp and gzip metadata; the workflow explicitly acknowledges that runner/compiler inputs prevent full binary reproducibility. The asset assembler preserves attached binaries and only extends checksums if every previous line survives. Existing artifacts retained from prior runs are deliberately not re-attested as newly built. This is a reasonable provenance distinction, not evidence that reproducible builds were demonstrated.

CLA uses a default-branch checkout and disables persisted credentials, with account IDs, coauthor enumeration and a compare-and-swap lock for signatures. The name-based exemption for an unlinked maintainer author persists, bounded to exempt opener paths. Its acceptance is a policy decision; it is not identity proof. The reviewer uses default-branch scripts, permission API checks for manual invocation, least-scoped pull-request write access, and a sandbox. It explicitly removes GH_TOKEN and GITHUB_TOKEN from the model process environment; earlier blanket claims that the model directly inherits those tokens do not apply to this revision. A self-hosted job can still be targeted by modified PR workflows, so external-contributor approval settings and runner isolation remain essential external controls. No live repository settings were fetched or changed; required-check and approval claims are **unverifiable from this checkout**.

## ICX-CODEX-070 — Release ancestry does not require successful CI

- Category: release control defect. Severity: Medium. Confidence: high. Revision: audited SHA above.
- Location: `.github/workflows/release.yml:65-95`; `docs/build-and-tooling.md` release table.
- Prerequisite: a maintainer starts a tag release or manual run for a commit in main history whose format, Clippy, coverage, secret, advisory or REUSE checks are failed, absent, pending, or cancelled, while workspace tests pass.
- Trust boundary: branch ancestry is being used as an authorization/evidence substitute for quality-check results. A remote anonymous attacker is not assumed.
- Source proof: `git merge-base --is-ancestor HEAD origin/main` checks only the graph relationship. The gate then runs tests; there is no query of check conclusions or dependency on CI jobs. Other release jobs depend on `gate`, `notices`, `build`, never the CI workflow. The comment asserting gates have run and the documentation inference are stronger than the enforced predicate.
- Impact: a draft release and, for an existing release, missing assets can be produced without the promised gates having succeeded. A draft reduces initial-publication impact but does not repair the predicate; existing releases receive uploads.
- Counterevidence: protected main and required status checks could prevent a subset of cases. Those settings were not verified, and ancestry cannot prove those settings existed for an older commit or that a push CI is complete.
- Recommendation: explicitly run required release gates or resolve the exact tagged commit's required check runs, validate names/workflow provenance and successful conclusions, and wait for pending checks with a bound. Keep a reviewed override separate and logged.
- Acceptance: a fixture or workflow test with ancestor=true, tests=pass, Clippy/coverage=failed must block packaging; missing/pending/cancelled checks must also fail closed. A fully green exact-commit fixture passes. No API/schema change is needed.
- Related prior finding: ICX-AUD-040. Existing pinning remediation is confirmed; this new release gate was outside that earlier finding's narrow remediation.

## ICX-CODEX-071 — Unicode PDF licensing annotation does not establish redistribution rights

- Category: governance evidence defect. Severity: Medium. Confidence: high for annotation mismatch; permission and legal outcome require counsel. Revision: audited SHA.
- Location: `REUSE.toml` block 8; `ref-docs/unicode-braille-patterns/Unicode_U2800_Braille_Patterns_Chart.pdf` and `standards-and-wg2/`.
- Prerequisite/impact: redistributing repository assets under the blanket `Unicode-3.0` annotation can communicate rights not established by that annotation.
- Source proof: block 8 covers both PDF charts and ISO/IEC archival material with Unicode-3.0. Current [Unicode terms](https://www.unicode.org/copyright.html), definitions and permissions, explicitly distinguish data/software from PDF charts and require checking each publication's own broader permissions. ISO material has additional provenance and cannot be assumed to inherit a Unicode data-file grant.
- Counterevidence: individual publications may grant broader rights or the owner may hold separate permissions. Those permissions were not found or independently established here. A passing REUSE lint validates label completeness, not legal accuracy. This report does not assert infringement or demand deletion.
- Recommendation: review each asset's embedded notice, establish written/source permission and owner provenance, annotate correctly, or replace redistribution with primary-source links. Have qualified counsel adjudicate edge cases.
- Acceptance: asset-level ledger links each retained file to a grant allowing its actual distribution and incorporates necessary notices; REUSE annotations then match those grants. No production compatibility change.
- Related: ICX-AUD-036 remains partially fixed: removal/relabel of arXiv files is visible, this component remains open.

## ICX-CODEX-074 — Governance terms remain unreconciled

- Category: governance question. Severity: Medium. Confidence: high that text persists; enforceability is not adjudicated. Revision: audited SHA.
- Locations: `PATENTS.md` sections 2.2–2.3, `CLA.md` section 3, `COMMERCIAL.md` license-required examples.
- Evidence: the documents retain automatic copyright-and-patent termination on litigation and broad commercial/internal/clean-room reservations alongside the distributed GPL/AGPL licenses. GPL's own sections 2, 7, 10 and 11 define running rights, permitted additional terms, downstream restrictions and contributor patent grants; the local unmodified GPL text is the comparison source.
- Impact: downstream users and contributors cannot resolve applicable rights from SPDX labels alone; commercial and patent assertions need authoritative legal reconciliation. No patent search, filing verification, or legal conclusion is claimed.
- Counterevidence: patent rights and copyright rights differ; clean-room implementations may implicate patents independently of copyright. Pending applications need not have public identifiers. Neither fact establishes that all listed restrictions override GPL grants.
- Recommendation: qualified counsel should reconcile the documents, distinguish copyright permissions from actual patent claims, and document any independently verified filing status without exposing confidential filings.
- Acceptance: counsel-approved consistent terms and an internal verification record support public marking; licenses, CLA and commercial examples agree. Preserve history using a dated supplement if documents are immutable.
- Related ICX-AUD-035: open/deferred, as the prior remediation ledger states.

## Prior audit reassessment

| Prior ID | Independent disposition | Evidence and limits |
| --- | --- | --- |
| ICX-AUD-034 | Confirmed fixed for explicit Cargo/REUSE crate declarations; no legal certification | Adapters/proxy manifests now specify GPL AND AGPL; first-party root GPL, notices and license texts inspected. Combined-work interpretation remains counsel scope. |
| ICX-AUD-035 | Open/deferred | ICX-CODEX-074; same unresolved terms persist. |
| ICX-AUD-036 | Partially fixed | REUSE now separately labels Zep NC-SA and remaining arXiv PDFs; non-exclusive papers are link-only in index. Unicode issue persists; no complete license attestation for every embedded image. |
| ICX-AUD-037 | Partially fixed / live settings unverifiable | SECURITY.md now provides private reporting URL and response target. Required-check and signatures-branch protection not inferred from YAML. |
| ICX-AUD-038 | Partially fixed | Asset input directory is configurable; patents ignored in tracked rules; history and binary duplication remain. No claim all historical personal paths are purged. |
| ICX-AUD-040 | Confirmed fixed for enumerated pins/policy; doctor partial | Full SHA pins, exact toolchain/tools and deny policy verified. Doctor on uninitialized CI state remains shallow; release ancestry defect newly identified. |
| ICX-AUD-041 | Confirmed fixed for reported unused dependencies | Old proxy codec/serde removed; CLI serde is now Unix-only and used by daemon. cargo-machete result in gate evidence. |

## Research ledger and external limits

Primary sources accessed 2026-11-21 UTC:

- [GitHub secure use reference](https://docs.github.com/en/actions/reference/security/secure-use): environment intermediates for untrusted workflow expressions, full-SHA pinning, least privilege and self-hosted runner trust. Workflows were checked against these mechanisms; no prompt-injection containment guarantee inferred.
- [GitHub approving fork workflows](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/approve-runs-from-forks): approval is configurable and maintainers must inspect workflow modifications. Actual settings not verified.
- [Unicode terms](https://www.unicode.org/copyright.html): asset-specific restrictions and data/software versus PDF distinction, used in ICX-CODEX-071.
- [CC BY-SA 4.0 legal code](https://creativecommons.org/licenses/by-sa/4.0/legalcode.en): attribution, notices and downstream restrictions; existing per-image annotations are evidence of improvement, not proof every PDF-embedded figure is properly attributed.
- GNU GPL official pages timed out twice; the repository's complete verbatim GPLv3 license was inspected as local primary license text. Remote freshness is unavailable, not silently claimed.

Dependency audit freshness is bounded to the locally available advisory DB unless a separately recorded fetch succeeds. RustSec tooling does not cover all C/SQLite advisories automatically. Native compiler, musl and runner OS security require their own inventory. Unsupported macOS/Windows behavior and deployed GitHub settings remain gaps.

## Reviewed-file disposition

Semantic static review: `.github/workflows/*.yml`, `.github/scripts/release-assemble.sh`, `.github/scripts/cla-check.sh`, `scripts/_agy_print.sh`, `scripts/_agy_comment_body.sh`, `Cargo.toml`, all crate manifests, `deny.toml`, `REUSE.toml`, `.github/about.toml`, `.github/about.hbs`, `.gitleaks.toml`, `.github/dependabot.yml`, `.github/CODEOWNERS`, `.github/actionlint.yaml`, `codecov.yml`, `rust-toolchain.toml`, LICENSE/COPYING and root governance/security documents. Targeted static review plus self-test: `scripts/agy-review.sh`, `scripts/agy-review-selftest.sh`. Targeted generated-output/portability review: `scripts/generate_assets_and_graphs.py`, `scripts/render_diagrams.py`; generators were not executed against tracked assets. Previous audit reports 07–10 and 12 read as hypotheses. Full generated binary reproduction and all image ownership chains were not demonstrated.

Exact commands, logs, tool versions and exits are in `evidence/validation-*`; failures and unavailable network refreshes must not be interpreted as successful gates.
