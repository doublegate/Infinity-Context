# 00 — Executive Summary

**Scope:** all 8 crates (~23k lines of Rust), every document in `docs/` and `ref-docs/` including
the 58-page AHTL PDF, CI and supply chain, licensing, and the agent integrations verified against
current upstream APIs. Subject: v1.0.0 at `c176d03`.

**Result:** 51 findings.

| Critical | High | Medium-High | Medium | Low | Info |
| --- | --- | --- | --- | --- | --- |
| 1 | 8 | 1 | 22 | 18 | 1 |

## Bottom line

The engineering hygiene is good:

- zero `unsafe` in first-party code;
- clean clippy-pedantic;
- real 95% coverage;
- no dependency advisories;
- a pinned and verified secret scanner;
- careful bounded parsing in most places.

The product claims are not supported by the shipped system:

- **Latency.** The hook path is about 1,900× slower than documented at 332 stored tool outputs
  (~940 ms vs `<500 µs`), and the slowdown grows linearly with every further output
  (ICX-AUD-001).
- **Cache hits.** The headline ≥94% cache-hit rate is produced by benchmark padding
  (ICX-AUD-002).
- **Provider caching.** The proxy's caching strategy probably works against the provider caches
  (ICX-AUD-003).
- **Integration.** Several adapters send context to fields the agents ignore or show only to the
  user (ICX-AUD-004 to ICX-AUD-006).
- **"Lossless".** The store is not lossless (ICX-AUD-021).
- **Recall.** Associative recall degrades to near chance at a thousand documents (ICX-AUD-049).
- **Security.** One security finding is High: a cloned repository can plant persistent prompt
  injections in the agent's first turn (ICX-AUD-010).

## Top 10 findings

| # | ID | Sev. | Finding | Evidence |
| --- | --- | --- | --- | --- |
| 1 | ICX-AUD-001 | Critical | Each hook reloads and re-encodes the entire HDC history. Real `icx hook` latency: 36 ms at ~14 artifacts, 296 ms at ~104, ~940 ms at 332; `perf` puts 87.5% in `HdcBundler::add_delta`. The `<500 µs` SLO measures an in-memory path. | measured, perf |
| 2 | ICX-AUD-010 | High | A repo-shipped `.icx/` directory is loaded without trust checks, and its text is injected into SessionStart/UserPromptSubmit context. Full bodies are served by deref with no untrusted marker. | PoC (private) |
| 3 | ICX-AUD-002 | High | The ≥94% hit rate is a byte-prefix ratio inflated by a 22.8 KB static head. Without the padding it is 69.9%. It is not a provider measurement. | PoC |
| 4 | ICX-AUD-004/005/006 | High | Claude Code `systemMessage` outputs go to the user, not the model. Codex `updatedMCPToolOutput` is unsupported and `additionalContext` *adds* tokens. The documented Hermes/Codex configurations cannot load. | upstream docs, local sources |
| 5 | ICX-AUD-003 | High | The Anthropic proxy places `cache_control` on message objects, strips client breakpoints, and rewrites already-cached history every turn. | docs + code |
| 6 | ICX-AUD-049 | High | HDC recall of a unique token: top-1 28% at 100 docs, **5.1% at 1,000 docs**. The Stage-1 pre-screen uses 64 of 10,240 bits. | PoC |
| 7 | ICX-AUD-021 | High | "Lossless" storage collapses CRLF/LF, trailing newlines, and `⟪` vs `<<` into the same artifact. | PoC |
| 8 | ICX-AUD-011 | Med-High | Content that starts with icx's own markers (`⟦`, `⟪ICX:STUB│`, …) bypasses Tier-0 escaping and redaction in the proxy and the Hermes adapter. | code path |
| 9 | ICX-AUD-012/013/014 | Medium | Unauthenticated proxy routes serve CAS data; `.icx/` is not git-ignored; unbounded stdin (300 MiB → 1.12 GB RSS, 4.3 s). | PoC, measured |
| 10 | ICX-AUD-015/016 | Medium | The redactor misses URL credentials, dotted tokens and multi-line secrets. A one-line PEM deletes the rest of the input. The redactor's mutation score is 68%. | PoC, mutants |

## Claim-vs-reality scorecard

| Claim | Verdict |
| --- | --- |
| Zero `unsafe` across 8 crates | **True** (first-party; dependencies use `unsafe` as expected) |
| ≥ 95% coverage | **True** (95.75%; 95.78% production-only) |
| 100% lossless CAS / DAG | **False** (ICX-AUD-021, -022) |
| Sub-50 µs associative index | Latency **true**; recall quality **false** (ICX-AUD-049) |
| < 500 µs synchronous hook | **False** for the shipped binary (ICX-AUD-001) |
| ≥ 94% prefix-cache hit rate | **Unsupported**: a fixture artifact (ICX-AUD-002, -003) |
| Lock-free `/dev/shm` seqlock telepathy bus | **Inaccurate**: file I/O with a writer lock; silent loss (ICX-AUD-046, -054) |
| Bi-temporal graph | **Synthetic timestamps** (ICX-AUD-048) |
| Tier-0 escaping prevents packet forgery | **Bypassable** (ICX-AUD-011, -010) |
| Loopback/UID isolation, UDS + `SO_PEERCRED` | **Not implemented** (ICX-AUD-012, -017) |
| Claude Code / Codex / agy / Hermes integration | **Partially working**; several outputs never reach the model (ICX-AUD-004 to -008) |
| AHTL v1.0 specification volume | **Stale** Chapter 08 (ICX-AUD-030) |
| Benchmarked SLO numbers | **Inconsistent** across documents, up to 100× (ICX-AUD-028, -031) |
| Research citations | Mostly accurate; several misattributed or unsourced figures (ICX-AUD-033) |

## Prioritized remediation roadmap

Ticket IDs follow the project's `T-PS-NNN` convention. They are proposals, and nothing has been
added to `to-dos/` yet.

1. **Now (security and correctness):**
   - Move workspaces out of the repository and refuse untrusted `.icx/` directories (010, 013).
   - Remove the prefix-based trust checks (011).
   - Add a token to the proxy and refuse non-loopback binds (012).
   - Bound stdin, MCP line length and fold depth (014).
   - Fix the PEM swallow and the URL-credential leak (015, 016).
   - Remove the checked-to-unchecked CAS fallback (022).
2. **Next (make the core promise true):**
   - Persist the HDC index and run a resident daemon, so the hook p99 stays under 50 ms at 10k
     artifacts (001, 047, 050).
   - Replace the Stage-1 pre-screen and add a token index (049).
   - Move virtualization to the hook path and stop rewriting history in the proxy (003).
   - Fix the adapter outputs against contract fixtures (004 to 008).
3. **Then (honesty and governance):**
   - Regenerate all published numbers from one benchmark JSON, drop the padded cache metric, and
     measure provider cache tokens (002, 028, 031).
   - Rebuild or unpublish the stale PDF (030).
   - Correct the citations (033).
   - Align the licenses and have counsel review PATENTS/CLA (034 to 036).
   - Add a security contact and make the CLA check required (037).
4. **Continuously (test strength):**
   - `proptest` round-trips, `cargo fuzz` on every parser, and a mutation gate of at least 85% on
     the codec (039).
   - Agent contract fixtures pinned to each agent version (006 to 008).

## What is solid

- **Integrity on read:** BLAKE3 verification on every CAS read, and fail-closed ambiguous handles.
- **Bounded parsing:** panic-free parsing on almost every path; recursion depth (026) is the one
  exception.
- **Seqlock memory ordering:** the in-process seqlock ordering is correct.
- **Atomic file writes:** temp file, 0600, rename.
- **Dependencies:** a small dependency set with no advisories, and a well-built gitleaks gate.
- **CLA workflow:** it avoids the classic `pull_request_target` checkout pitfall.
- **Real-input compression:** inline footprint reduction on real inputs (34–55×) is within the
  documented range.
