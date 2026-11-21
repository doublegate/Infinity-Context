# Primary-source research ledger

Access date: **2026-11-21 UTC** (2026-11-20 local). All lookups were read-only/free.
No model provider, token-counting API, active agent session, or credential store was invoked.

Closeout refresh on the same UTC date re-opened the official MCP 2025-11-25 overview,
Anthropic caching page, BLAKE3 specification repository and Unicode Braille chart. The chart
again redirected to Unicode 18.0. This refresh confirms the cited source locations and scope;
it does not establish provider behavior, mathematical proofs or redistribution permission.

## Standards and mechanism checks

| Source and version | Checked section / supporting content (paraphrase) | What it does and does not establish |
| --- | --- | --- |
| [MCP 2025-11-25](https://modelcontextprotocol.io/specification/2025-11-25) | Overview: stateful JSON-RPC connections and capability negotiation | Normative legacy contract; handler support alone is not lifecycle conformance |
| [Anthropic prompt caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching) | Automatic/explicit cache control, prefix identity, 5-minute and 1-hour TTLs | Supports cache-policy review; no application hit rate measured |
| [BLAKE3 specifications](https://github.com/BLAKE3-team/BLAKE3-specs) | Maintainer specification/analysis repository | Full hash/XOF/MAC design; not proof that a 48/64-bit prefix is collision free |
| [Unicode Braille chart](https://www.unicode.org/charts/PDF/U2800.pdf) | Redirects to Unicode 18.0, block U+2800–U+28FF; first page terms distinguish chart reproduction | Establishes codepoints, not BPE tokens or general chart redistribution rights |
| [xRAG v2](https://arxiv.org/abs/2405.13792v2) | Abstract: trainable modality bridge from document embeddings into a frozen model | Its results do not validate textual AHTL sketches without such a bridge |
| [DuoAttention v1](https://arxiv.org/abs/2410.10819v1) | Abstract: full KV for retrieval heads, smaller cache for streaming heads | Model/cache-internal work, not evidence of preserved semantics for arbitrary symbolic syntax |
| [Infini-attention v2](https://arxiv.org/abs/2404.07143v2) | Abstract: compressive memory integrated into transformer attention | Inspiration for local bundling must be separated from implementing the neural architecture |
| [LLMLingua-2 v2](https://arxiv.org/abs/2403.12968v2) | Abstract: distilled token classification with encoder, measured compression on named tasks | Does not establish AHTL token ratios or reasoning quality |
| [Cache Engineering v1](https://arxiv.org/abs/2610.28577v1) | Abstract: serving-cache operators and engine evaluation | Reports paper-specific 27.7% average cost reduction and 1.5x P95 speedup; not ADR's 4–8x TTFT claim |
| [ACM v1](https://arxiv.org/abs/2607.23809v1) | Title/abstract: Agentic Context Management, offload/query tools and post-training | Confirms correction from “Cache” to “Context”; no demonstrated equivalence to icx wire caching |
| [LCM v1](https://arxiv.org/abs/2605.04050v1) | Title/date/abstract: summary DAG retaining original-message pointers | Primary page says submitted 14 Feb 2026 despite identifier prefix. No fabricated date correction inferred |
| [Fractal KV Archives v2](https://arxiv.org/abs/2607.07144v2) | Abstract: symbols after quantization, lossless archive vs lossy quantizer | Lossless symbolic archive does not imply lossless recovery from icx's 16-dimensional trajectory projection |

Source summaries are deliberately short. Workstream reports carry more specific protocol,
security, redistribution and mathematical checks near each finding.

## Bibliographic identity pass: all 22 indexed arXiv records

Every abstract URL below resolved. Version is the latest version exposed by the page during
this pass; this is not a claim that each frozen local PDF has that version. Local reference-PDF
hashes, page counts and title metadata are in [reference-pdf-metadata.json](evidence/reference-pdf-metadata.json).

| Record | Latest observed version | Identity/result scope |
| --- | --- | --- |
| [2610.28577](https://arxiv.org/abs/2610.28577) | v1 | Cache Engineering for Agents; abstract checked |
| [2607.23809](https://arxiv.org/abs/2607.23809) | v1 | Agentic Context Management; abstract checked |
| [2607.07144](https://arxiv.org/abs/2607.07144) | v2 | Fractal KV-Cache Archives; abstract checked |
| [2605.04050](https://arxiv.org/abs/2605.04050) | v1 | Lossless Context Management; abstract checked |
| [2512.24601](https://arxiv.org/abs/2512.24601) | v3 | Recursive Language Models; identity/version checked |
| [2510.11967](https://arxiv.org/abs/2510.11967) | v1 | Context-Folding; identity/version checked |
| [2510.04618](https://arxiv.org/abs/2510.04618) | v3 | Agentic Context Engineering; identity/version checked |
| [2502.12110](https://arxiv.org/abs/2502.12110) | v11 | A-MEM; identity/version checked |
| [2501.13956](https://arxiv.org/abs/2501.13956) | v1 | Zep; identity/version checked |
| [2501.00663](https://arxiv.org/abs/2501.00663) | v1 | Titans; submission date 31 Dec 2024 |
| [2410.15704](https://arxiv.org/abs/2410.15704) | v1 | Residual vector quantization; single author/title checked |
| [2410.10819](https://arxiv.org/abs/2410.10819) | v1 | DuoAttention; abstract checked |
| [2406.10774](https://arxiv.org/abs/2406.10774) | v2 | Quest; identity/version checked |
| [2406.02069](https://arxiv.org/abs/2406.02069) | v4 | PyramidKV; identity/version checked |
| [2405.14831](https://arxiv.org/abs/2405.14831) | v3 | HippoRAG; identity/version checked |
| [2405.13792](https://arxiv.org/abs/2405.13792) | v2 | xRAG; abstract checked |
| [2404.14469](https://arxiv.org/abs/2404.14469) | v2 | SnapKV; identity/version checked |
| [2404.07143](https://arxiv.org/abs/2404.07143) | v2 | Infini-attention; abstract checked |
| [2403.12968](https://arxiv.org/abs/2403.12968) | v2 | LLMLingua-2; abstract checked |
| [2401.03462](https://arxiv.org/abs/2401.03462) | v3 | Activation Beacon; identity/version checked |
| [2310.08560](https://arxiv.org/abs/2310.08560) | v2 | MemGPT; identity/version checked |
| [2307.06945](https://arxiv.org/abs/2307.06945) | not captured | ICAE; title/resolution checked; version not independently transcribed |

## Explicit research limits

- The 22-record pass checks identity and selected abstract claims; it does not independently reproduce all paper experiments or re-review every line of all papers.
- Maintained website pages can change; versioned MCP/arXiv links are used where possible. The Unicode unversioned URL redirected to 18.0 during this audit. Source identity is not license permission.
- Historical `ev_*` binary/SQLite evidence identifiers were read as provenance claims, not treated as current schema or authorization. Private databases were not opened to validate them.
- Non-arXiv historical bibliography and every external prior-art repository were not exhaustively revalidated. A local filesystem path to ai-memory is not independently accessible public prior art.
- No Anthropic tokenizer, real cache counters, live model reasoning effectiveness, repository branch-protection configuration or patent filing records were verified. These remain follow-up procedures, not positive assurance.
- Where current primary sources conflict with a frozen paper or historical record, preserve both with version/date and explain the change; do not silently rewrite immutable reference material.
