# 11 — Enhancements, Optimizations and Architectural Recommendations

These are forward-looking improvements, separate from the defects in documents 01–10. Each item
notes the findings it resolves or reduces.

## A. Architecture

1. **A resident daemon per workspace** (resolves ICX-AUD-001, -047, -054; reduces -012).
   - Hooks become thin clients that send one request over a `0600` Unix socket under
     `$XDG_RUNTIME_DIR/icx/`, and authenticate the peer with `SO_PEERCRED`. This is the design the
     threat model already describes.
   - The daemon keeps the MMU and HDC index in memory, appends to a write-ahead log, and
     snapshots periodically.
   - The proxy and the MCP pager run inside the same daemon, so they share one index instead of
     each reloading JSON per request.
2. **Workspace location and trust** (resolves ICX-AUD-010, -013).
   - Default to `$XDG_STATE_HOME/icx/<blake3(canonical repo root)>`.
   - A repo-local `.icx/` becomes opt-in and is refused when it is git-tracked or not owned by the
     user.
3. **Out-of-band provenance** (resolves ICX-AUD-011).
   - Attach a keyed BLAKE3 MAC, under a per-install secret, to every icx-emitted block or stub.
   - Content without a valid MAC is always untrusted, regardless of its prefix.
4. **Rewrite on the hook path, not in the proxy** (resolves ICX-AUD-003).
   - Virtualize tool output when it enters the transcript: PostToolUse `updatedToolOutput` with
     the correct output shape for each tool.
   - Never rewrite history that has already been sent.
   - The proxy then only appends dynamic material after the client's last breakpoint, and never
     strips client breakpoints.
5. **Honest storage tiers** (resolves ICX-AUD-021).
   - Store raw bytes, encrypted at rest with a per-workspace key, alongside the sanitized
     model-facing view.
   - "Lossless" then refers to the raw tier, and the model sees only the sanitized view.

## B. Retrieval quality (resolves ICX-AUD-049)

- Hybrid retrieval: a rare-token inverted index (BM25 or trigram) for exact identifiers, HDC
  similarity for fuzzy and structural matches, and PPR over the graph for multi-hop expansion.
- Pre-screen on at least 1,024 bits instead of 64. Make `candidate_cap` grow with corpus size.
- Ship a retrieval evaluation harness (recall@k, MRR) on a seeded corpus. Report retrieval
  quality separately from generation, as master-core module 91 recommends.

## C. Performance (resolves ICX-AUD-001, -050; see `07-performance-and-benchmarks.md` §2)

- Persist per-artifact hypervectors and the bundler state instead of re-encoding on load.
- Use bit-sliced majority counters, a SIMD-friendly layout, and runtime CPU dispatch for POPCNT.
- Replace whole-file JSON persistence with SQLite (WAL) or an append-only segment log.
- Build an incremental SSE parser, and give `grep_cold` a line or trigram index.

## D. Codec and specification

- Make the EBNF executable: generate parser tests from it, and add every emitted header kind as
  a production (ICX-AUD-030, -031).
- Use one reversible escaping layer for every free-text field: subject, clause, slice hint and
  tail-slot labels (ICX-AUD-021, -025).
- Bound recursion and length in every parser (ICX-AUD-026).
- Use full 256-bit digests as internal keys; keep 64- and 48-bit forms as display handles
  (ICX-AUD-023).

## E. Integration robustness (resolves ICX-AUD-004 to ICX-AUD-008)

- Keep a per-agent capability matrix that records, for each pinned agent version, which output
  fields reach the model. Generate it from contract tests, not by hand.
- Add `icx install --agent <name> --dry-run`, which writes the *current, verified* hook
  configuration for each agent, so users don't copy outdated snippets from the guide.
- MCP: negotiate the protocol version and implement `server/discover`.

## F. Observability (module 91 alignment)

- Emit one trace per hook or request, with spans for load, retrieve, transform and save, and
  token counts before and after.
- Record cache read and creation tokens from provider responses through the proxy. This is the
  only honest basis for any cache-hit claim (ICX-AUD-002).

## G. Documentation and governance

- One canonical numbers table, generated from benchmark JSON. Everything else links to it
  (ICX-AUD-028, -031).
- Rebuild the PDF in CI, or publish it as a release asset instead of committing it (ICX-AUD-030).
- Align licensing declarations, run `cargo deny` with a license allow-list, and add a security
  contact (ICX-AUD-034, -037, -040).
- Replace the 10 non-redistributable arXiv PDFs with links (ICX-AUD-036).
