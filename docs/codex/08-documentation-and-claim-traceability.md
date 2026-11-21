# Documentation, claims, and independent-source traceability

Audited revision: `a996d59f1eb468e7c386747e15f4c7e58d08410e` (v1.1.0).
Research access: 2026-11-21 UTC / 2026-11-20 America/New_York.

## ICX-CODEX-100 — Accepted ADRs and completed roadmap retain superseded guarantees

- **Category / severity / confidence:** documentation contradiction / Medium / High.
- **Locations:** `docs/adr/0001-five-plane-hybrid-interception.md:22`, `docs/adr/0002-alien-holographic-token-lattice.md:14`, `docs/adr/0003-prefix-cache-epoch-pinning.md:9`, `to-dos/ROADMAP.md:9`, `docs/diagrams/crate-and-agent-relationships.mmd`.
- **Prerequisite / impact:** a contributor follows accepted architecture decisions, normative diagram sources, or the current planning entry point. They receive incompatible defaults, integration capabilities, tokenization promises, and performance expectations. This is a reproducible documentation defect; no security boundary is crossed.
- **Proof:** ADR-0003 states 16,384 default rollover tokens and >=94% provider cache hits; `EpochControllerConfig::default()` at `crates/icx-core/src/epoch.rs:21` sets 4,096, and `prefix_reuse_bytes` explicitly measures bytes rather than provider cache hits. ADR-0002 promises single-token operators and 100% identifier preservation in attention space; README's Braille paragraph expressly leaves tokenization unverified. ADR-0001 promises coverage of every CLI/IDE, while README's integration table says Codex cannot shrink output and Hermes lacks a shipped shim. ROADMAP calls v1.0.0 current and reports 97.5%+ measured coverage; Cargo is v1.1.0 and the older audit itself corrected that coverage claim. The normative crate diagram still labels the bus `mmap`, while ADR-0005 and implementation describe positional I/O.
- **Primary-source check:** [Anthropic caching documentation](https://platform.claude.com/docs/en/build-with-claude/prompt-caching) describes cache usage, TTL and exact-prefix prerequisites; it does not establish any application's empirical hit rate. [Cache Engineering for Agents v1](https://arxiv.org/abs/2610.28577v1) reports its own serving-engine experiments; its results cannot establish `icx` performance.
- **Counterevidence:** README and `docs/numbers.md` are substantially clearer, and README explicitly says its diagrams override `.mmd` copies. ADR-0004 is explicitly partly superseded by ADR-0005. Those qualifications reduce impact, but ADRs 0001–0003 remain accepted without equivalent supersession, and the roadmap advertises current status.
- **Recommended remediation:** add a superseding ADR for outdated assertions; annotate historical planning records and diagram sources with their applicable revision; generate current numbers and diagrams from shared inputs. Preserve historical ADR content.
- **Acceptance:** a contributor entering through README, roadmap, each accepted ADR, and normative diagram sources sees the same runtime defaults, supported adapter capabilities, measurement provenance, and qualification of token/provider claims. A documentation check flags retired numerical promises in current normative content.
- **Prior findings:** ICX-AUD-028, 031, 033, 046. The earlier blanket documentation-fixed status is only partially supported.

## Material claim matrix

This table groups repeated claims by mechanism. The extraction ledger retains 826 individual
candidate lines with original paths and line numbers; they are leads, not 826 independently
verified findings. Historical claims are not silently promoted to current product guarantees.

| Claim / documentary entry | Implementation / tests | Independent oracle or evidence | Status and limit |
| --- | --- | --- | --- |
| 8 crates; Rust edition 2024; minimum Rust 1.100 (`Cargo.toml`, README) | Workspace members and crate manifests | Cargo metadata and validation logs | Directly inspectable constants; platform support remains bounded by actual builds |
| No first-party unsafe (README, build guide) | Workspace forbid lint, inherited per crate | Strict compiler/lint gate | First-party policy only; native SQLite and transitive dependencies excluded |
| Exact recovery after redaction (README, architecture) | CAS digest verification, MMU ingestion; core/adapter tests | State probes and codec fixtures | Distinguish exact stored bytes from original secrets and escaped model-facing output |
| 48/64-bit handles; full BLAKE3 integrity (numbers, codec) | CAS and ArtifactId | [BLAKE3 specifications](https://github.com/BLAKE3-team/BLAKE3-specs) | Prefix identity is weaker than full digest; ambiguity/collision handling must be checked separately |
| 50% prefix collision point (numbers) | Mathematical derivation, not runtime test | sqrt(2 ln(2) 2^b): 19.75 million for 48 bits; 5.057 billion for 64 bits | Supported random-uniform birthday approximation; not chosen-target work |
| Byte/Braille bijection (README, AHTL chapters) | `braille.rs`, exhaustive 256-codepoint tests | [Unicode Braille chart](https://www.unicode.org/charts/PDF/U2800.pdf) | Byte-to-codepoint property supported; token count is a different property |
| One BPE token per operator/glyph (ADR-0002, foundations) | No tokenizer integration | No tiktoken/tokenizers/transformers installed in audit Python | Unverified for named provider models; do not infer from Unicode allocation |
| AHTL is directly useful to attention heads (README, foundation corpus) | Text serializer and structural heuristics | [xRAG](https://arxiv.org/abs/2405.13792v2), [DuoAttention](https://arxiv.org/abs/2410.10819v1) | Unsupported transfer inference: xRAG trains an embedding bridge; DuoAttention changes cache policy. Neither evaluates AHTL |
| 214x /159.8x compression (README, numbers) | `icx benchmark`, skeletonizer | Ten-run audit measurements | Synthetic byte ratios of skeleton plus pointer; storage and dereference overhead excluded; not model token savings |
| >=94% provider cache hits (ADR-0003, completed plans) | `prefix_reuse_bytes`, synthetic transcript | Provider usage counters were not called | Unsupported; byte reuse only. Current README qualifies this correctly |
| 4096/4/1024 default vs provider presets (numbers, architecture) | `epoch.rs:21–50`, MMU default | Source constants | Supported defaults; 16384/6/2048 and 8192/6/2048 are optional presets |
| Unlimited effective context (README) | Epoch accumulation and explicit retarget API | Epoch-growth harness | See ICX-CODEX-101; no automatic global context bound demonstrated |
| 10,240-bit vector; XOR/permutation algebra (HDC specification) | `hv.rs`, algebra tests | Binary algebra and independent vectors | Algebra supported; probabilistic semantic recall requires separate experiments |
| Recall@5 >=95% (numbers) | n=300 fixture | Multi-scale audit corpus | Fixture gate only; see math report for distribution and seed limits |
| Fractal matching recovers exact trajectory suffix (foundations) | Distance estimate and inverse-IFS verifier | Math report counterexamples | Distinguish contraction upper bound from converse or unique state identity |
| Titans / Infini-attention behavior (glossary, HDC docs) | Hand-built surprise and bundler | [Titans](https://arxiv.org/abs/2501.00663v1), [Infini-attention](https://arxiv.org/abs/2404.07143v2) | Inspired heuristics; not equivalent neural architectures or reproduced paper quality |
| Bi-temporal validity and PPR (graph specification) | `graph.rs`, functional supersession and CSR tests | Mathematical PPR properties | Algorithm support does not demonstrate multi-hop agent task effectiveness |
| File-backed lock-free zero-copy bus (older ADR/plans) | `ring.rs` positional I/O + lock | Source and concurrency probes | Contradicted for file backend; in-process atomic backend distinct |
| Four-agent <100µs propagation (completed plans) | In-memory adapter simulation; two file handles | Bus report | Two handles in one process do not establish four live harness processes |
| Workspace ownership/seals prevent repository context adoption (threat model) | `workspace.rs`, owner/MAC tests | State audit | Controls are meaningful; same-uid adversary can obtain key and is outside stated isolation |
| Single SQLite transaction makes all acknowledgments durable (possible reading of architecture) | Store transaction vs daemon write-behind | State report | Transaction atomicity and acknowledgment durability are separate guarantees |
| Agent contract compatibility (README integration table) | JSON adapters, local contract tests | Versioned upstream docs | Model consumption and live agent activation unverified; no credentials/config changed |
| MCP conformance (wire spec) | `mcp.rs`, bounds tests | [2025-11-25 MCP specification](https://modelcontextprotocol.io/specification/2025-11-25) | Need lifecycle, envelope, error, and version checks beyond handler unit tests |
| 95% coverage establishes correctness (testing guide) | llvm-cov line gate | Mutation sample and oracle review | Coverage demonstrates execution, not assertion strength or absence of bugs |
| Release tag on main establishes green gates (build guide) | Release workflow | Supply-chain report | Ancestry and actual successful CI status are distinct |
| REUSE metadata establishes redistribution permission (licensing docs) | REUSE.toml / license texts | Official terms; supply-chain report | Metadata lint cannot resolve asset provenance or legal compatibility |

## Historical corpus and research interpretation

`ref-docs/research-report.md` is explicitly historical and points at the dated correction file.
Its Zstd, mmap, AVX-512, 180-token-schema and unconditional latency statements must not be read
as a current feature list. The corrections file appropriately distinguishes raw CAS bytes,
positional I/O, optional presets and heuristic surprise. It does not supersede every normative
ADR or certify newer mathematical assertions in the foundation archives.

All 22 arXiv identifiers in the index resolved during this audit. This checks identity, not
every paper's result or a license grant for every accompanying asset. Latest abstract-page
versions may differ from the frozen local PDFs. The research ledger records those limits.

The local link scan returned one apparent failure: the animated tesseract URL contains balanced
parentheses and the simple regex truncates it. Manual inspection rejected that lead; no broken
local-link finding is raised from it. External URLs and anchors were not exhaustively checked.

## ICX-CODEX-102 — Worked Braille example contains a different Unicode character

- **Category / severity / confidence:** reference specification defect / Low / High.
- **Revision / location:** audited revision above; `ref-docs/unicode-braille-patterns/README.md:90` and the glyph row at 92 in §3.3.
- **Prerequisite / impact:** a reader copies Example 2's glyph as a canonical vector for U+28C7. The copied character is U+0347, COMBINING EQUALS SIGN BELOW, and cannot decode as a Braille byte. This affects the example, not the production arithmetic codec.
- **Proof:** `python3 docs/codex/evidence/check-braille-table.py` checks all 256 full table rows against independent codepoint/UTF-8/name arithmetic. Those rows pass, but the worked example's literal character has `ord=0x0347`, whereas its label says U+28C7. Correct U+28C7 encodes to `e2 a3 87`. [Result](evidence/braille-table-result.json). The [official Unicode chart](https://www.unicode.org/charts/PDF/U2800.pdf) is the primary block reference.
- **Counterevidence:** Example 2's numerical byte, bit pattern, dot list and UTF-8 bytes agree with U+28C7, as does the full lookup table and implementation. This is a copying/typesetting defect, not a broken byte bijection or a security vulnerability.
- **Recommendation:** preserve the immutable reference and add a dated correction identifying the literal scalar and corrected glyph. Generate worked glyphs and tables from a shared numeric source.
- **Acceptance:** every literal worked glyph's scalar and UTF-8 bytes match its label; the full 256-entry arithmetic test stays green.
- **Compatibility / prior IDs:** no wire or API change; no directly corresponding prior finding.
