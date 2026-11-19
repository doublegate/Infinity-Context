# 09 — Documentation Consistency, Citations and Governance

Workstream W9. The licensing observations are an engineering consistency review, **not legal
advice**; items marked "counsel" need a lawyer.

## ICX-AUD-031 — Cross-document numeric contradictions (Medium)

The same quantity is stated differently across `docs/`, `docs/ahtl/`, `ref-docs/`, README,
CHANGELOG and ROADMAP. Code values were read at `c176d03`.

| Quantity | Statements | Code / measured |
| --- | --- | --- |
| Epoch rollover threshold | 16,384 Anthropic / 8,192 OpenAI (README :257, CHANGELOG :93, ADR-0003 :14, deploy guide :922-923, `performance.md:34`); 4,096 (`docs/ahtl/07:104`) | `EpochControllerConfig::default()` = **4,096**, `protect_last_n_turns` = 4, `tail_slot_max_tokens` = 1,024 (`epoch.rs:21-29`); no provider-specific default exists |
| Protected tail turns | 6 (`virtual-memory-and-dag-engine.md:75`, CHANGELOG); 4 (ch07, deploy) | 4 |
| Tail slot size | ≤ 2,048 (glossary :14, VM :71); ≤ 1,024 (architecture :87, adapters :134); 512 or 1,024 (deploy) | 1,024 |
| Anthropic breakpoint layout | "system + up to 3 sealed epochs" (README :258); BP0 / BP1-3 zones (VM :68-69); BP1 system, BP2 tools, BP3 epoch, BP4 reserved (deploy :847-851) | one breakpoint at `total-3` (`wire.rs:127`) |
| Compression ratio | 8–25×, 12–40×, 12–72×, 15–40×, 18.4–72.1×, 72.1×, 214× | 34–55× on real inputs; 214× on the fixture (ICX-AUD-028) |
| HDC Hamming scan (1k nodes) | `<25 µs`, `<50 µs` (10k), 12.40 µs, 1,306 µs | 72.7 µs median |
| Seqlock publish | `<300 ns`, `<500 ns` (/dev/shm), `<800 ns`, ~45 ns, 44.2 ns, `<1 µs`, `<100 µs` | 62.9 ns in-memory; the file backend is unmeasured |
| Hook budget | `<500 µs`, `<450 µs` p99, `<400 µs`, `<100 µs`, `<250 µs`, 118.4 µs | 36 ms–940 ms real (ICX-AUD-001) |
| Shm transport | positional I/O, no mmap (`performance.md:32`, ADR-0004); "memory-mapped" (`wire-protocol-and-shm-bus.md:13`); `memmap2`/`libc` (research report :122, :144) | positional I/O |
| Shm path | `/dev/shm/icx-fabric-v1`; `.icx/icx-fabric-v1.shm`; `$XDG_RUNTIME_DIR/icx` | `.icx/icx-fabric-v1.shm` |
| Redactor | 4 tiers (ch05, codec doc, threat model); **3 tiers** (`docs/ahtl/README.md:29,51`, PDF cover); entropy threshold 4.35 vs **4.65** (deploy :80, :967, :1080, :1281) | 4 tiers, 4.35 |
| Corner-vertex projection | popcount majority ≥ 320 per block (ch03); "parity of the 160 words" (`hyperdimensional-and-fractal-index.md:60`) | majority |
| PPR iterations | 8 (architecture :86, HDC doc :99); 6 (exactness review :163) | — |
| CAS chunk storage | Zstd-compressed chunks (architecture :45, :69; VM :12; data model `compressed_bytes`; research report :140) | raw bytes; no zstd dependency |
| DAG tier naming | three incompatible schemes (VM :52-57 / VM :12 / README :87) | L1 turn/tool, L2 branch, L3 epoch, L4 fold |
| MCP schema size / ops | `<180` vs `<150` tokens; four different op lists; signature `icx(op, ref, q, k)` vs schema `id, q, range` | ops `deref slice grep recall fold expand xagent status` (`mcp.rs:20-45`) |
| `AhtlWirePacketV1` fields | three different field sets (data model :145-152; 08.md; PDF Ch08) | the `ahtl.rs:357-366` field set |
| Coverage | ≥ 95%; 96.13%; 97.5%+ | 95.75% (95.78% production-only) |
| Sprints | 14 (CHANGELOG :133); 15 (ROADMAP) | — |
| Fractal precision | "128-bit fixed-point" (`ref-docs/papers/INDEX.md:56`) | `[f64; 16]` in memory, i16 Q15/Q1.14 on the wire |

**Remediation.** Keep one source of truth (`docs/performance.md` plus a generated
`benchmarks.json`). Have every other document link to it rather than restate numbers. Add a CI
check that greps the docs for numbers covered by the canonical table.

## ICX-AUD-033 — Citation errors and unsupported attributions (Medium)

All 22 arXiv IDs resolve, and all 22 local PDFs match their arXiv title and authors. The
following problems remain:

| Item | Project statement | Fact | Verdict |
| --- | --- | --- | --- |
| arXiv 2607.23809 | "ACM: Agentic **Cache** Management" with KV-cache block retention (INDEX :46-48, research report :171, PDF filename) | "ACM: Agentic **Context** Management for Long Horizon Tasks" (Li et al.), about agent memory tools; KV cache appears only in related work | CONTRADICTED |
| "up to 10× cost / 4–8× TTFT" (ADR-0003 :9) attributed to arXiv 2610.28577 | — | the paper reports 27.7% cost reduction, 1.2× throughput, 1.5× P95 latency, and 2–5× price ratios | NOT FOUND / misattributed |
| "19.0% passkey recall after summarization" (`docs/ahtl/07:17`) | — | in no cited paper; its only other source is hard-coded in `generate_ahtl_master_diagrams.py:1163,1245` | NOT FOUND |
| "4.5–6.0 bits/token" (ADR-0002 :9) | — | unsourced; derived in `novel-methods` from about 1.1–1.5 bits/char | NOT FOUND |
| RVQ "5.5×–8×" (INDEX :60) | — | 5.5× supported; 8× not found; the paper shows a GSM8k quality gap, not "negligible loss" | PARTIAL |
| "128-bit fixed-point" for Fractal KV archives (INDEX :56) | — | the paper uses IEEE double and 2-D points | CONTRADICTED |
| LCM arXiv 2605.04050 "May 2026" | — | arXiv date stamp 14 Feb 2026; the paper has no "cryptographic" pointers (INDEX :14) | PARTIAL |
| Minor | RVQ authorship and title paraphrased; ICAE title paraphrased; Titans dated Jan 2025 | Titans was submitted 31 Dec 2024 | Low |

The prior-art GitHub repositories listed (14) all exist, are not archived, and match their
descriptions. "ai-memory" is cited by a local filesystem path, not a public URL.

## ICX-AUD-032 — The exactness review contradicts itself

See `01-ahtl-spec-conformance.md` §4: clause escaping is described as both absent and enforced,
the 48-bit birthday figure is wrong, and unresolved items sit under a blanket "ENFORCED".

## ICX-AUD-034 — License declarations are inconsistent (Medium; counsel)

| Source | Says |
| --- | --- |
| `Cargo.toml:18` (all crates) | `GPL-3.0-or-later` |
| `REUSE.toml:44-50`, README :483 | icx-proxy and icx-adapters are `GPL-3.0-or-later AND AGPL-3.0-or-later` |
| `LICENSING.md:11-12` | only icx-proxy (and a hosted pager) is AGPL; adapters omitted |
| README :482 | icx-cli is GPL-only, but it depends on icx-adapters and icx-proxy (`crates/icx-cli/Cargo.toml:17,22`), so the AGPL terms apply to the combined `icx` binary |

- **Effect on tooling:** no `.rs` file carries an SPDX header, and the crates ship no LICENSE
  file. crates.io and `cargo deny` would therefore see GPL only.
- **`LICENSING.md:23`:** it says GPL §13 places network deployments of the GPL crates under the
  AGPL. GPLv3 §13 creates no network obligation; it only permits combination with AGPL works.
  AGPLv3 §13 applies to *modified* versions of the AGPL-covered work, not to "the entire service
  stack".
- **`COMMERCIAL.md:62`:** it implies that internal commercial use requires a commercial license,
  which conflicts with GPLv3 §2 (unlimited permission to run the program).
- **`CLA.md:48`:** it requires contributed third-party material to be "GPL-compatible", while
  the repository itself ships CC-BY-NC-ND, CC-BY-SA, CC-BY-NC-SA and arXiv non-exclusive content.
- **Remediation:** make the Cargo `license` field match REUSE for each crate. Add SPDX headers,
  or a per-crate LICENSE file. Correct LICENSING.md :23 and COMMERCIAL.md :62.

## ICX-AUD-035 — Patent and CLA terms vs GPLv3 (Medium; counsel)

- **Termination clause:** `PATENTS.md:32` and `CLA.md:33` terminate the *copyright* license on a
  patent suit. That is not among the additional terms GPLv3 §7 permits, so under §7 and §10 it is
  a "further restriction" that recipients may remove. It therefore offers little protection while
  signalling non-free terms.
- **Internal-use denial:** `PATENTS.md:28` denies a patent license for internal non-conveyed use,
  which is in tension with the unconditional §11 patent grant from contributors who convey.
- **Unsubstantiated filings:** "Patent pending (5 applications)" has no application numbers in
  the repository; the `patents/` directory is excluded locally (`.git/info/exclude`) and
  described as ready to upload. Confirm the filings exist before keeping the marking: false
  marking (35 U.S.C. § 292) is a risk, and public disclosure in this repository may affect
  novelty in jurisdictions without a grace period.
- **CC-BY-NC-ND specifications:** they contain reference Rust code, so code copied from the spec
  into a GPL fork conflicts with ND. Copyright cannot prevent clean-room reimplementation of
  ideas (17 U.S.C. § 102(b); CC 4.0 §2(b)(2) excludes patent rights).

## ICX-AUD-036 — Redistribution of third-party content (Medium; counsel)

- **arXiv PDFs:**
  - `REUSE.toml:108` labels all 22 papers `CC-BY-4.0 OR arXiv-NonExclusive`. Per their arXiv
    pages, 11 are CC BY 4.0.
  - **10 carry only the arXiv non-exclusive distribution license**, which grants arXiv, not
    third parties, the right to redistribute: 2307.06945, 2403.12968, 2404.07143, 2405.14831,
    2406.02069, 2406.10774, 2410.10819, 2502.12110, 2510.11967 and 2610.28577.
  - 2501.13956 (Zep) is **CC BY-NC-SA 4.0**, which neither label covers.
  - Remediation: remove the non-exclusive PDFs and link to arXiv instead, and relabel Zep.
- **Wikimedia material:**
  - Wikipedia wordmark and tagline images under `ref-docs/blake3-cryptographic-hash/images/retrieved/`
    fall through to the DoubleGate CC-BY-NC-ND label, although they are Wikimedia trademarks.
  - Some Commons images were renamed so that their provenance is unclear (for example
    `Banach_Fixed_Point_Contraction.png`).
  - The 58-page AHTL PDF embeds 61 images under CC-BY-NC-ND with no CC BY-SA attribution for
    Commons-derived figures. Share-alike material cannot be relicensed ND.
- **Unicode-3.0 label:** it is applied to WG2/ISO documents and Unicode chart and chapter PDFs;
  the label is correct only for Unicode data files.
- **Reverse-engineering notes:** `ref-docs/agent-internals/rea-binary-and-sqlite-forensics.md`
  documents reverse engineering of proprietary agent binaries and is labelled as DoubleGate's
  own CC work. Review the vendors' terms before publishing.

## ICX-AUD-037 — Security contact and CLA enforcement (Low)

- **No reporting contact:** `SECURITY.md` has none, although README :309 says it covers
  "vulnerability reporting". Add a private channel, such as GitHub private vulnerability
  reporting.
- **CLA workflow:** the workflow itself is sound. It runs `pull_request_target`, checks out the
  default branch only with `persist-credentials: false`, keys signatures by account ID, and
  requires co-authors to sign. However:
  - The CLA status is **not a required check** in the default-branch ruleset, so it does not block
    merges.
  - The `cla-signatures` branch has no ruleset protection.
  - A second, undocumented signing phrase is accepted (`.github/scripts/cla-check.sh:11`).
  - An exemption is keyed on a git author *name*, which can be spoofed
    (`.github/scripts/cla-check.sh:28`). This only matters for PRs opened by maintainers.

## ICX-AUD-038 — Repository hygiene (Low)

- **Personal paths:** absolute paths to the maintainer's home directory and a conversation UUID
  appear in tracked public documents: the deployment guide (8), the adapters doc (4), the REA
  forensics (11), the prior-art survey (1), and `scripts/generate_assets_and_graphs.py` (1).
- **Large binaries:** 19 MB of generated PDF in git, and a 6.4 MB image stored twice
  (`docs/ahtl/images/retrieved/` and `ref-docs/ahtl-foundations/images/retrieved/`).
- **`patents/` exclusion:** it is excluded only through the local `.git/info/exclude`, not
  `.gitignore`, so a fresh clone on another machine could commit it.
- **Lint results:**
  - `markdownlint-cli2`: 143 issues, mostly blank-line formatting; two structural ones, a
    7-cell row in a 3-column table at `docs/ahtl/05:131` and a dead `#roadmap` fragment at
    README :76.
  - `typos`: two real typos in code comments (`nam` in `redact.rs:756`; `asess` in
    `icx-adapters/src/lib.rs:303-304`). The other hits are false positives (`CC-BY-NC-ND`, hex
    `BA`).
  - `lychee --offline`: no broken internal links.

## Positive results

- `uvx reuse lint` passes: 468 of 468 files are labelled and REUSE 3.3 compliant. Several labels
  are wrong, as described above.
- The cited prior-art repositories exist.
- The CLA workflow avoids the classic `pull_request_target` checkout pitfall.
