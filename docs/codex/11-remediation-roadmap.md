# Prioritized remediation and enhancement roadmap

Recommendations only; production source, schema and wire formats were not changed by this audit.
Effort bands are estimates for implementation plus meaningful regression tests: S = under two
engineering days, M = several days, L = one or more weeks. Reproduction harnesses are starting
oracles, not a substitute for integration tests.

| Priority | Scope / finding | Dependency | Effort | Compatibility and acceptance |
| --- | --- | --- | --- | --- |
| P1 | Secret YAML block values (001) | Agree scalar indentation/chomping behavior | M | Narrow previously stored secrets; purge/rotate already-exposed real credentials only through a separately authorized data plan. Verify both redact and MMU CAS/deref; preserve following public keys |
| P1 | Retrieval candidate recall (004) | Freeze varied/repetitive/multilingual evaluation corpora | L | Rebuild derived token/candidate index with versioned snapshots; CAS handles unchanged. Compare three seeds at 100/1k/10k against exact lexical and exhaustive scorer, including distractors and non-Latin queries |
| P1 | Daemon acknowledgment/persistence failure semantics | State report findings and durability policy decision | M–L | Add durable mode/journal or explicit volatile receipt. Preserve pending writes across reload failures; crash after ACK and during flush, compare recovered handles, test competing writers and retries |
| P1 | Release status verification (070) | Decide required CI check names and repository policy | M | Refuse publication until required checks on exact commit succeeded; fake API status fixtures cover pending/failed/cancelled/missing and fork/tag cases |
| P2 | Profile A framing (002) | Canonical escaping/validation contract | M | Accepted raw skeletons may need a versioned decoder or migration. Hostile field property must preserve one-packet cardinality or return an error |
| P2 | Fractal fc scale (005) | Decide normative Q1.14 vs shipped Q15 | M | Never reinterpret existing archived values silently. Tag versions; independent literal vectors ±1, ±0.5, 0 and cross-version tests |
| P2 | Snapshot semantic validation (003) | Define finite/range and cached-summary invariants | M | Reject/rebuild invalid derived state while retaining source artifacts; NaN/Inf/out-of-range and inconsistent summary fixtures |
| P2 | Redaction idempotence (006) | Define continuation classification before/after replacement | M | Preserve the minimized fuzz fixture and ordinary valid YAML cases. One pass must reach the documented normal form; repeat redaction must not remove additional public text or produce new artifacts |
| P2 | Automatic window budget (101) | Model limits, tokenizer/estimate policy, output reserve | L | Window selection changes provider prefixes. Preserve cold history; >10k-turn actual-caller test with cap including schemas/head/tail/completion reserve |
| P2 | MCP and bus protocol edge cases | See state report recommendations | M | Explicit errors can affect permissive callers. Version/document strict mode and migration; fuzz typed protocol envelopes and sequence invariants |
| P2 | Redistribution provenance (071), patent/CLA text (074) | Qualified owner/counsel decisions | M–L | Metadata-only relabeling does not obtain permissions. Obtain grants or remove/link artifacts; synchronize package notices and published terms |
| P2 | Current documentation contradictions (100) | Decisions above and benchmark source schema | M | Add superseding ADRs; retain historical text with clear status. Generate diagram labels and tables; reviewers must see same runtime defaults from all entry points |
| P3 | Test assurance (073) | Mutant triage and meaningful behavioral contracts | L | Add tests for independently meaningful missing behaviors; do not assert incidental implementation details. Run complete mutation partitions and classify equivalent/unviable/timeout separately |
| P3 | Measurement labeling (072) | Direct 10k measurement and benchmark schema | S | Label arithmetic extrapolation or measure 10k directly; persist target CPU, seeds, distributions and repetition count |

## Stop conditions and evidence for a future release

1. Resolve credential leakage and demonstrated context-retrieval regressions first; a green
   baseline does not waive them.
2. Decide durable acknowledgment and window-budget contracts before changing APIs or persistence.
3. Run regression probes against the new revision, then full CI-equivalent gates and refresh
   advisory/protocol sources. Record exactly which behavior each new test would detect.
4. Complete the outstanding full mutation partitions, omitted failure-injection scenarios and
   unsupported-platform builds. Use offline fake provider servers for HTTP/SSE/cancellation.
5. Record separate optional live-agent/model/provider cache acceptance with named versions and
   explicit authorization for any paid calls. Never substitute byte-prefix reuse for provider
   counters or exact dereference for model recall.

No audit finding authorizes publishing a release, altering repository protection, replacing
user agent configurations, or deleting stored data. Those actions were outside this approved
audit's scope.
