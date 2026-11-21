# Coverage ledger and verification limits

## Baseline and file accounting

The baseline contains **519 tracked paths**, including **103 Markdown files**. Each path has
its byte size, SHA-256, assigned workstream and disposition/depth in
[inventory.csv](evidence/inventory.csv). The baseline revision, branch transition and preserved
AGENTS content hash are in [baseline.json](evidence/baseline.json). The initial only tracked
change was `AGENTS.md`:24 insertions and 1 deletion. Audit-created files are separately confined
to `docs/codex/`; no pre-existing untracked project content was found before their creation.

The path inventory is exhaustive for tracked files. **Semantic assurance is not uniform or
exhaustive.** A hashed asset is accounted for, not proven correct. An executed test is not the
same as reviewing its oracle. Historical HTML/JSON/Wikimedia/ISO archives were inventoried and
sampled for relevant claims/provenance; all their embedded external pages were not rereviewed.
Ignored build trees and private/local state are outside the source inventory. No user memory,
credential database, or active agent configuration was changed.

## Workstream coverage

| Area | Reviewed/executed | Exact depth / remaining limitation |
| --- | --- | --- |
| `icx-core` | CAS/chunker/DAG/epoch source, baseline tests, mutation partitions; byte and epoch probes | State source review targeted by mechanism; not every in-module test independently inspected. Epoch assembly/defaults independently rechecked by root |
| `icx-codec` | Production parser/renderer, redactor, skeletonizer and Braille paths; property/regression tests and fuzz | [Codec per-file ledger](evidence/codec-coverage.md); some long fixture tails sampled. Profile A framing and YAML secrets independently reproduced |
| `icx-hdc` | Index/query/snapshot/bundler/fractal/surprise source; algebra derivations; 27 scaling datasets; fuzz/mutation | Three seeds, 100/1k/10k repetitive/varied/multilingual corpora. Live-model retrieval effectiveness remains outside offline evidence |
| `icx-store` | Functional edges, temporal query, CSR and PPR source; baseline tests and independent dense numerical oracle | Oracle is a small three-node vector; no realistic multi-hop graph contribution ablation. Transaction-time query axis not demonstrated |
| `icx-bus` | Layout/attach/publish/poll/loss source; stress/regression tests; corrupted metadata, truncation and separate-process probes | No exhaustive weak-memory proof, all interleavings, sequence wrap, four live harnesses or every process-death point |
| `icx-adapters` | Workspace trust, seals, SQLite, MMU selected paths; all adapter wire contracts; MCP fuzz/probes | [State per-file CSV](evidence/state-coverage.csv). Complete text reads for selected modules, targeted source for others. Live agents, every tool output shape and installed Hermes shim not exercised |
| `icx-proxy` | Authentication/routing/framing/SSE/curl/caching source and baseline fixtures | No exhaustive malformed HTTP/SSE campaign. A real-curl cancellation countertest rejects one source-only lead; no paid provider use |
| `icx-cli` | Dispatch/workspace/deref/daemon/benchmark/doctor paths; binary tests; real CLI YAML roundtrip | CLI tests rerun with socket permission. Doctor detects installations, not live tool interception. Daemon ACK/final-failure probes use actual daemon module with synthetic dispatcher |
| AHTL PDF | Hash; all 58 pages extracted and assigned chapter; conformance matrix; root contact-sheet layout pass of all 58 | Two equation/code pages checked at full size. Contact-sheet thumbnails do not certify every equation/table cell; model-effectiveness statements were not validated |
| `docs/` and root prose | Quantitative/normative lead extraction, architecture/interfaces/data-model/security/build/testing/deployment, accepted ADRs and prior reports | Current core documents reviewed across workers; some historical reports and large tables targeted/sampled. Claims grouped by material mechanism in report 08, not 826 independent verified assertions |
| `ref-docs/` prose | Initial/research/corrections/prior-art/novel-methods/exactness/foundations, forensic ledger and Braille/BLAKE3 archives | 22 arXiv identity checks plus selected primary-paper mechanisms. Every bibliography link/result and every archival proof not independently validated. Immutable references preserved |
| Roadmap and planning | Roadmap, all phase/sprint files and implementation plans read as historical planning claims | Completed checkboxes are not test evidence; stale current-version and success guarantees recorded as 100 |
| Unicode tables | All 256 detailed Braille table rows independently checked for codepoint/UTF-8/name/bit arithmetic | Worked literal example separately fails 102; historical ISO adoption statements not fully rederived |
| Workflows/scripts/governance | Semantic static review, self-tests, actionlint, dependency/notice/REUSE/secret checks | GitHub live required-check policy, signing keys, legal grants for every embedded image and patent filing status not verified |
| Generated images/assets | Full path/size/hash accounting and source-label/metadata sampling | Not regenerated over source; no exhaustive visual or creator-rights certification. Source labels can be stale even when bitmap matches generator |

## Planned validation and excluded assurance

| Requested validation | Evidence disposition |
| --- | --- |
| Baseline format/Clippy/tests/rustdoc/95% coverage | Executed; see report 06 and individual logs. Sandbox socket failures were separated from successful host reruns |
| Benchmark / doctor / dependency / notices / REUSE / secrets / workflow tests | Executed with exact exits. Strict timing gates failed during contention; do not call all gates green. Installed vs CI secret scanner version differences adjudicated separately |
| Four existing fuzz targets, 15 minutes each | Each has a900-second execution budget after build; final outcome records are in validation evidence. Requested duration alone is not proof a fuzzer completed |
| Mutation analysis across 8 crates | Full mutant inventories; deterministic sampled partitions per crate. Full generated mutation space is **not exhausted**; survivors, unviable cases and timeouts remain distinct |
| 100/1k/10k corpora, 3 seeds, varied/repetitive/multilingual | Executed with lexical and exhaustive composite baselines, latency and serialized index sizes; multilingual RSS includes entire harness process |
| Ten independent portable/native benchmark repetitions and warm-up | Executed; shared active-host contention limits timing inference. No small speedup or portable-SLO guarantee inferred |
| Byte recovery / corruption / crash / failed save | Specific probes executed. Full power-cut, disk-full, competing-writer, fsync fault and every SQLite/slot transition matrix remains unexecuted |
| Agent/provider/tokenization behavior | Public contract/source checks only. No live agent sessions, provider-cache counters, tokenizer APIs or reasoning-efficacy evaluation |
| Unsupported platforms | No macOS/Windows/BSD or cross-architecture executable validation in this audit |

These are practical coverage boundaries, not evidence that the excluded cases pass. The audit
does not certify exhaustive source review, all platform support, formal algorithm correctness,
legal compliance or absence of vulnerabilities. Its reproducible findings and explicit
dispositions are intended to make the remaining work reviewable.
