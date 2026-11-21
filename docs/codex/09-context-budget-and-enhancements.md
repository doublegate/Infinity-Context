# Context budgeting and enhancement opportunities

## ICX-CODEX-101 — Production epoch accumulation has no automatic overall context cap

- **Category / severity / confidence:** architecture limitation and unsupported capability / Medium / High.
- **Affected revision:** `a996d59f1eb468e7c386747e15f4c7e58d08410e`.
- **Locations:** `crates/icx-core/src/epoch.rs:236`, `:308`, `:333`; `crates/icx-adapters/src/mmu.rs:896`; README overview.
- **Prerequisite:** a sufficiently long-running caller records new distinct turns and assembles the complete epoch window without explicitly calling `retarget_budget`.
- **Observable result:** sealed epochs and their rendered prefix grow with history. Automatic rollover reduces the unsealed turn list but does not place a maximum on the combined prefix. A fixed physical model window still needs an explicit budget/compaction policy.
- **Source proof:** `maybe_rollover` pushes to `sealed_epochs`. `assemble_window` clones every stored epoch frame and every active frame. `force_seal_active_epoch` deliberately keeps the epoch list intact. The only non-definition `retarget_budget` calls in adapter/CLI/proxy sources are inside the adapter's long-horizon test (`crates/icx-adapters/src/lib.rs:2298`), where the test itself intervenes above 20,480 estimated tokens. The public MMU assembly method does not do that intervention.
- **Reproduction:** run `python3 docs/codex/evidence/run-epoch-growth.py` from repository root. The isolated public-API harness uses `EpochControllerConfig::default()`, distinct turn nodes, and recorded estimates of 4,096 tokens per turn. At 10/100/1,000 turns it reports 6/96/996 sealed frames, 742/8,576/86,880 prefix bytes, and 126/2,016/20,916 estimated Zone-1 tokens. [Machine-readable result](evidence/epoch-growth-result.json). No provider calls, persistence, or real tokenizer is involved.
- **Impact limits / counterevidence:** this is not proof that a particular deployed model fails at 1,000 turns; the harness uses synthetic estimates and a controller-level API. The explicit retarget API exists, Hermes/proxy may select portions of the history, and host compaction can independently manage windows. Exact cold storage remains useful. Those mechanisms do not establish the README's unqualified unlimited effective-context claim, nor automatic bounded total context for all callers.
- **Attacker / boundary:** ordinary accumulated user activity is sufficient; no adversary or privilege crossing is required.
- **Recommendation:** give the orchestration layer a model-context budget, reserve room for response/tool schemas/tail, and automatically fold or page older epochs when the assembled prefix exceeds that budget. Define the cache invalidation tradeoff: a changed historical prefix cannot simultaneously retain its old provider cache key. Use named tokenizers or conservatively documented estimates.
- **Compatibility / migration:** preserve CAS/DAG identities and old frames; change which frames are selected for a prompt. Persist the selected-window/budget state with a version so restarts retain the same policy. Tightening the default assembled output affects cache prefixes and agent-visible history; offer explicit configuration and migration notes.
- **Acceptance:** run more than 10,000 distinct turns through the actual production caller without test-only retarget calls; show a configured total-window upper bound including UTF-8, head, tail and schemas; dereference old omitted content exactly; verify restart and smaller-model switching.
- **Related prior findings:** ICX-AUD-001 (history scaling), 002 (cache metric), 031 (capability claims). This is distinct from per-hook reload performance.

## Measured opportunities and proposals

| Opportunity | Evidence / expected benefit | Dependencies and acceptance |
| --- | --- | --- |
| Replace approximate token-Bloom rejection with a verified lexical candidate path | Multi-scale math experiment separates candidate loss from full-ranker loss | Keep an inverted token posting list or hybrid search; compare same seeds against lexical/exhaustive baseline, reporting recall and p50/p95/RSS |
| Avoid whole-history cloning during window assembly | Source proof above identifies one owned String clone per sealed frame | Profile real production calls first; cache immutable serialized frame storage or borrow/slice selected frames; preserve exact bytes for retained prefix |
| Introduce explicit bounded window selection | The epoch-growth probe measures linear prefix growth | Account head + history + tail + tools + completion reserve; test >10k turns and model switch without test intervention |
| Separate acknowledgment from durability | Daemon intentionally writes behind | Document volatile ACK, add flush/durable ACK or journal; crash/restart oracle compares returned handles with recovered CAS |
| Tokenizer-aware report mode | Byte ratios exist, installed audit Python lacks tokenizers | Optional offline tokenizer with name/hash; count originals, packets, schema and dereference cost; provider counts only in separately authorized work |
| Reproducible benchmark dataset and results schema | Existing synthetic labels are clearer than old claims; values remain copied across many docs | Export seed, compiler target, hardware, iterations, distribution, corpus hash and measurement scope; generate tables from that schema |
| Contract fixtures from pinned agent versions | Adapters tested locally; live model consumption remains separate | Vendor bounded permissively redistributable schemas or links/checksums, verify producer/consumer behavior with a fake host, and retain optional manual live acceptance steps |

These are recommendations, not implemented optimizations. No performance improvement is claimed
without a before/after experiment. The running audit's parallel builds and fuzzing create host
contention; timings from probes characterize that run rather than an isolated hardware SLO.
