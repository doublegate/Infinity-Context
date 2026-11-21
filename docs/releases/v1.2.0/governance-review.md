# v1.2.0 governance decision packet

This packet makes audit071/074 actionable. It is not a legal opinion or evidence that a
permission, filing or qualified approval exists. No license grant or existing terms are changed
by this document. Owner/qualified decisions are still required before closing these findings.

## Asset distribution decision

The prepared [asset-level inventory](evidence/governance-assets.csv) records source references,
file hashes and proposed dispositions. Its entries identify review work; an inventory entry
does not establish a redistribution grant.

Review `REUSE.toml` block8 and `ref-docs/unicode-braille-patterns/`, particularly the Unicode
Braille chart and ISO/WG2 archives. The blanket Unicode-3.0 label does not establish permission
for every PDF/document. For every retained redistributed asset, record source URL, checksum,
embedded notice, grant covering the actual distribution and required attribution. Data-file
permissions and chart/publication permissions must be evaluated separately.

Concrete exclusion option: preserve archival originals outside the release/source distribution,
replace unsupported bundled publications with primary-source links and hashes, and publish an
allowlisted source archive. Merely omitting PDFs from binary tarballs is insufficient while
automatically generated source archives of the tag still contain those tracked publications.
No history rewrite or asset removal has been performed. The proposed exclusion scope is the
publications without an established grant, not Unicode codepoint facts or independent tables.

## Terms reconciliation decision

Have the owner and qualified reviewer resolve these exact points consistently across
`LICENSE`, `COPYING`, `LICENSING.md`, `PATENTS.md`, `COMMERCIAL.md` and `CLA.md`:

1. Which code files are GPL, AGPL, or genuinely offered under alternatives, and how that agrees
   with each manifest and REUSE expression.
2. Whether the patent/CLA retaliation text intends to terminate copyright permissions, and
   how that interacts with the distributed license texts.
3. Which commercial/internal/hosted/reimplementation examples depend on copyright terms,
   contractual promises or independently verified patent claims.
4. What support exists for public patent-pending markings; record verification privately where
   necessary without exposing confidential applications or inventing filing identifiers.
5. Whether already accepted contributor agreements require a dated supplement or a new version.

Proposed clarification for review: explain that distributed open-source grants are governed by
their applicable license texts, distinguish any separate patent/commercial offer, and avoid
claiming that an SPDX annotation or passing metadata linter settles either issue. This is a
review proposal, not an adopted amendment or an assumption that all clauses are enforceable.

## Acceptance record to complete

| Decision | Evidence required | State |
| --- | --- | --- |
| Retained publication grants / source exclusions | Asset-level permission ledger or reviewed exclusion manifest | pending |
| Code-license and network-license consistency | Owner/qualified approved file-to-license mapping | pending |
| Patent/CLA/commercial reconciliation | Approved dated text and transition decision | pending |
| Public patent status | Owner verification supporting public marking | not verified by automation |

Prepare the final reviewable changes before publication approval. Until these rows are resolved,
the release record must retain071/074 as open; metadata completeness alone cannot close them.
