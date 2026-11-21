# v1.2.0 implementation plan

Branch: `codex-audit`. Starting source revision: `a996d59f1eb468e7c386747e15f4c7e58d08410e`.
The audit in `docs/codex/` describes that revision and remains historical evidence.
This tracker records implementation separately. No commit, push, tag or publication is authorized.

## Acceptance and compatibility decisions

- Preserve existing CAS handles and exact post-redaction recovery. New redaction applies to new
  ingestion; existing user stores are never silently rewritten or purged.
- Preserve existing Q15 wire interpretation. Any corrected alternative needs explicit versioning;
  bus Q15 and Profile A coordinate semantics must be documented independently.
- A durable daemon receipt must follow a successful store save. Buffered receipts must identify
  their weaker guarantee explicitly. Save errors must propagate to clients and process exits.
- Configure a total model window including system/head, history, tools/tail and completion reserve.
  Historical content remains in cold storage when omitted from the prompt.
- Retrieval acceptance uses frozen corpus hashes/seeds at 100/1k/10k, exact lexical and exhaustive
  baselines, recall@1/@5, p50/p95 and memory. Exact lexical candidates must not disappear because
  of approximate filtering. Publish measured quality/latency rather than an unqualified guarantee.
- Changes to supported wire/API contracts must remain backward readable or include an explicit,
  tested migration. No silent reinterpretation of old snapshots or archived packets.

## Tickets

Status: pending / active / verified / external decision. A ticket is verified only with recorded
regression evidence; a successful build alone does not close it.

| Ticket | Phase | Audit / scope | Status | Acceptance |
| --- | --- | --- | --- | --- |
| T-120-001 | 0 | Planning and durable status | verified locally | All 19 findings and enhancement proposals mapped; evidence preserved |
| T-120-002 | 1 | 001 YAML secret scalars | verified locally | Literal/folded/equals scalar fixtures removed before CAS; public sibling fields preserved |
| T-120-003 | 1 | 006 redaction idempotence | verified locally | Minimized/original fixtures pass; final 859,078 executions/901 seconds with normal sanitizer shutdown |
| T-120-004 | 1 | 002 Profile A framing | verified locally | Hostile skeletons roundtrip as exactly one packet or explicit error |
| T-120-005 | 1 | 003 snapshot validation | verified locally | Reject nonfinite/inconsistent derived state; valid old snapshots load |
| T-120-006 | 1 | 032 bus sequence validation | verified locally | Coherent wrong-sequence slot rejected with correct loss/recovery behavior |
| T-120-007 | 1 | 033 extreme chunker configuration | verified locally | Invalid public configurations cannot panic or overflow |
| T-120-008 | 2 | 004 retrieval candidate recall | verified locally | Frozen varied/multilingual corpus vs lexical/exhaustive baselines; no approximate false-negative exclusion |
| T-120-009 | 2 | 101 automatic total context budget | verified locally | Actual production caller >10k turns stays bounded; restart/model-switch/old dereference pass |
| T-120-010 | 3 | 030 daemon durability | verified locally | ACK→kill→restart preserves durable receipts; save failure returns error and nonzero exit |
| T-120-011 | 3 | 031 MCP input validation | verified locally | Invalid version/id/op/arguments return typed errors without side effects |
| T-120-012 | 3 | 035 Codex capability | verified locally | Supported contract or explicit unavailable capability; no unsupported replacement output |
| T-120-013 | 3 | 005 coordinate scale | verified locally | Independent literal vectors and explicit preserved/versioned interpretation |
| T-120-014 | 4 | 100 conflicting specifications | verified locally | Superseding ADRs and current docs agree with runtime; historical records preserved |
| T-120-015 | 4 | 102 Unicode example | verified locally | Correct codepoint/UTF8 literal; archived publication corrected by erratum |
| T-120-016 | 4 | 071 redistribution provenance | verified locally | Owner decision 2026-11-21: Unicode/WG2/UTC publications removed from the tree from v1.2.0 on (v1.1.0 tag, source archives and history still contain them; history rewrite not done); `ref-docs/unicode-braille-patterns/SOURCES.md` lists official sources (3 byte-identical) and prior SHA-256; UCD data files retained |
| T-120-017 | 4 | 074 patent/CLA/commercial terms | external decision | Owner decision 2026-11-21: terms unchanged in v1.2.0 pending counsel; carried as an open known limitation |
| T-120-018 | 5 | 070 release exact-commit gates | verified locally | Missing/failed/pending/cancelled checks block; correct exact-SHA success passes |
| T-120-019 | 5 | 072 measured benchmarks | verified locally | Actual 1k/10k scans and machine-readable provenance/dispersion |
| T-120-020 | 5 | 073 test assurance | verified bounded scope | Independent critical-family oracles/rechecks; all survivors/timeouts triaged; unexecuted mutation space explicit |
| T-120-021 | 2 | Window assembly allocation optimization | verified locally | Profile before/after; retained prefix bytes unchanged; measurable benefit |
| T-120-022 | 5 | Offline tokenizer reporting | verified locally | Identified tokenizer/hash or explicit estimator; include schema and dereference costs |
| T-120-023 | 5 | Pinned agent contract fixtures | verified locally | Offline producer/consumer fixtures with provenance; manual live acceptance remains separate |
| T-120-024 | 5 | Release candidate qualification | active qualification | Local packages/migrations pass; strict performance, governance and exact-commit CI remain open |
| T-120-025 | 5 | Measured HDC kernel/codegen optimization | active qualification | Exact-bit oracle, matched A/B proof and unchanged strict performance ceilings |
| T-120-026 | 5 | Review: redaction header/multi-line/flow/plural leaks | verified locally | `{api_key: x, password: \|`, under-indented plain and quoted multi-line scalars, `password: [x]`, plural keys hidden; idempotent |
| T-120-027 | 5 | Review: public digest over-redaction | verified locally | Owner decision: v1.1.0 hex rule kept except 40/64/128-hex digests in sha256sum, `sha256:`, `commit`, CSV and table contexts |
| T-120-028 | 5 | Review: saturated window prefix stability | verified locally | Step eviction to 50% with persisted point and one `ICX:HISTORY_OMITTED` frame; 0.94 steady-state reuse (was 0.01) |
| T-120-029 | 5 | Review: lexical index bounds, IDF ranking, candidate cap | verified locally | >256-byte tokens not posted, snapshot always decodable; rare identifiers outrank stop words; bounded candidates |
| T-120-030 | 5 | Review: daemon lock handling, Codex nulls, bus loss | verified locally | Lock/reload failure fails only the request; request read before lock under one deadline; Codex replies null-free; loss from one head |

## Execution evidence

Local evidence: `docs/releases/v1.2.0/evidence/`; current report: `docs/releases/v1.2.0/README.md`.

- Regression tests reproduced YAML leakage/idempotence, packet framing, snapshot corruption,
  bus sequence mismatch, arithmetic overflow, invalid MCP and volatile daemon receipts before fixes.
- Full workspace tests including the ignored qualification test pass: 224 tests, 0 ignored.
  Final post-change coverage passes 19,571/20,559 lines = 95.1943%; the initial 95.029% result
  remains separately recorded.
- Frozen 27-row corpus: recall@1/@5=100% across all rows. Actual >10k turn production test passes.
- Legacy 03 fixture generated by old writer; MMU upgrade rebuilds postings and preserves CAS bytes.
- Window profile compares a996d59/current on identical 10k turn history: owned rendered prefix
  869,884→24,505 bytes; retained frames byte-identical; stored 9,996 epochs preserved. Timings are
  synthetic shared-host measurements, not a provider/cache guarantee.
- Initial all-crate mutation samples executed 135 cases across two frozen snapshots; 80 caught,
  43 missed, 7 unviable, 5 timeouts. Targeted rechecks and post-snapshot tests are separate evidence.
- [Mutation triage](../docs/releases/v1.2.0/mutation-triage.md) covers every recorded survivor
  and timeout, with remaining oracle gaps explicit; fresh directed rechecks are separate.
- Three initial parser fuzz campaigns completed 900+ seconds with normal shutdown. Additional
  first-pass/UTF8 fixes passed renewed redactor 859,078 executions/901 seconds. Seeded 03/04
  complete snapshot fuzzing passed 2,231,269 executions/901 seconds with a 64KiB input bound.
- All four local Linux packages pass linkage/provenance/checksum and pre/post-strip smoke
  checks, including execution on pinned Ubuntu 22.04/glibc 2.35. Optimized strict benchmarks
  still fail; host compiler contention was observed and is retained as a measurement limit.
- Current docs/diagrams are reconciled; branded raster artwork is explicitly historical.
- Redistribution and patent/CLA review packets are prepared; owner/qualified decisions remain open.

Implementation owner: Codex on this working tree. Governance decisions: project owner/qualified
reviewer. No commits or external publication have been performed.
Build/test scratch: `/tmp/Infinity-Context/v120/`. Run meaningful targeted tests per change,
then full CI-equivalent gates before release qualification. Fuzz runs require functioning
sanitizer shutdown. Quiet-host benchmarks must not run beside mutation/fuzz compilation.

## Release gates

- All findings have verified remediation or an explicit documented release decision; legal
  questions cannot be closed by changing an SPDX annotation alone.
- Formatting, strict Clippy, workspace tests, warning-free rustdoc, >=95% line coverage,
  dependency/advisory/license/secret checks and workflow self-tests pass on the candidate.
- Regression corpora, crash/restart, snapshot/wire migration and configured-window tests pass.
- Fuzz/mutation outcomes have no concealed infrastructure failures or untriaged material survivors.
- Versions/lockfile/changelog/guides/notices/release notes agree. Packages install and run doctor.
- Exact candidate commit has required successful CI. Merge/tag/publish require separate instruction.
