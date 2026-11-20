# Changelog

All notable changes to **Infinity-Context (`icx`)** are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html), and is distributed under the **GNU General Public License v3.0 or later (`GPL-3.0-or-later`)** ([`LICENSE`](LICENSE)).

## [Unreleased]

## [1.1.0] - 2026-11-18

Remediation release for the adversarial audit of v1.0.0 in [`docs/claude/`](docs/claude/README.md).
Finding IDs (`ICX-AUD-NNN`) refer to that audit; per-finding status is in
[`docs/claude/12-v1.1.0-remediation-status.md`](docs/claude/12-v1.1.0-remediation-status.md).

### Migration

- Workspaces now live under `$XDG_STATE_HOME/icx/workspaces/<project-hash>/` in a single
  `icx.sqlite3`. A repository-local `.icx/` is ignored with a warning; adopt one you own with
  `icx trust --dir <path>`. v1.0.x JSON stores are migrated on first load without deleting them.
- `icx proxy` requires the install token: use the base URL it prints
  (`http://127.0.0.1:49380/t/<token>`) or send `x-icx-token`. Non-loopback binds need
  `--allow-remote`.
- `icx deref` frames output as untrusted data by default; use `--raw` for exact bytes.

### Security

- Repository-shipped `.icx/` workspaces are no longer loaded into agent context; workspaces are
  owner-checked, 0700/0600, sealed with a per-install key and MAC-verified (ICX-AUD-010, -013).
- The proxy authenticates clients and binds loopback only by default. The curl header files that
  carry upstream API keys are written to the private runtime directory (`$XDG_RUNTIME_DIR/icx`)
  and stale ones are swept on start (ICX-AUD-012, -020).
- icx structural markers anywhere in untrusted input, including the latest message, are escaped
  unless they are byte-identical to a block icx emitted; deref output carries a
  `trust=untrusted_data` provenance header (ICX-AUD-011).
- Bounded hook stdin, MCP lines (4 MiB, oversized lines drained without buffering) and fold
  depth (64) (ICX-AUD-014).
- Only structural tool-call/-result blocks for the icx pager are exempt from marker
  neutralization; tool output that merely contains `"name": "icx"` is not.
- Reopening a store resets widened `icx.sqlite3`, `-wal` and `-shm` modes to `0600`.
- `install.key` is checked (regular file, owner, mode) before it is read, so a FIFO is refused
  instead of blocking; only a missing key is created.
- `icx encode-ahtl` refuses an untrusted workspace instead of printing a handle it never stored.
- `icx mcp-serve` re-verifies the workspace per request, so a later `icx trust` takes effect and
  a workspace whose ownership, mode or seal changed is no longer used.
- Proxy curl header files go to a directory verified to be ours and private
  (`$XDG_RUNTIME_DIR/icx`, else `<tmp>/icx-<uid>`) and stale ones are swept every minute.
- Redactor: URL userinfo passwords for every scheme, SendGrid, Slack app/webhook, GitLab,
  DigitalOcean, Shopify and Twilio formats, more key names, multi-line values, single-line PEM,
  and bounded unterminated key blocks (ICX-AUD-015, -016). A key with no inline value redacts
  only a following scalar line: nested mappings, comments and prose ending in `key:` are kept.
  Token-only URL userinfo (`https://<token>@host`, 16+ mixed or 32+ token characters) is
  redacted. A quoted continuation value
  is treated as a nested key only if its closing quote precedes the separator. Private-key banners are matched
  within one `-----BEGIN ...-----` banner, and text around a key block is kept.
- agy `transcriptPath` opens non-blocking without following symlinks; the bus file refuses
  symlinks, foreign owners and permissive modes (ICX-AUD-018, -019).
- Hooks never echo input fields back as decisions (Claude Code, Codex and agy PreToolUse; agy
  PostInvocation) and never force an agent to continue (agy Stop) (ICX-AUD-007, -009).

### Fixed

- Hook latency no longer grows with workspace size: SQLite store with append-only rows, persisted
  HDC index snapshot, and a bit-sliced bundler. 338 artifacts: 940 ms to 14 ms per hook in
  process; about 3 ms median through the resident daemon (ICX-AUD-001, -047, -050).
- Storage is lossless after redaction: CRLF, trailing newlines and literal delimiters survive;
  escaping applies only to model-facing views (ICX-AUD-021).
- 64-bit handle collisions are refused instead of overwriting, in both the CAS and the DAG,
  including Hermes carried-AHTL lines and subagent summaries (ICX-AUD-022, -023).
- The daemon flushes and releases the workspace lock at most 1 s after the first unsaved change
  even under continuous hook traffic. A failed flush keeps the unsaved state in memory, releases
  the lock and retries every second; after 5 consecutive failures it logs an error and drops the
  changes so the daemon can exit. Requests that never reach a handler (malformed, wrong
  protocol) do not trigger a save.
- Concurrent first use of the install key, and concurrent `prepare` of one workspace, no longer
  collide on temp files or observe a directory before its seal is written.
- A save refuses to seal a snapshot older than the store on disk (another writer committed
  since it was loaded) and re-inserts rows removed behind a long-lived process, instead of
  writing a store that cannot load. The daemon logs an error when another writer's newer
  store replaces its unsaved changes.
- Emitted-block digests are stored in an append-only, seal-covered table instead of being
  rewritten inside the state blob on every save.
- Epoch headers are hex everywhere and epoch ids are u32 (ICX-AUD-024); invalid `slice_hint`
  values are neither rendered nor parsed (ICX-AUD-025); clause chains are parsed iteratively and
  capped at 64 links (ICX-AUD-026).
- Associative recall of a unique token in 1,000 log documents: top-1 5.1% to 99.0%, top-5 5.6%
  to 99.7% (ICX-AUD-049); one hit per artifact (ICX-AUD-042).
- Claude Code: shape-preserving `updatedToolOutput`, plain-text PreCompact instructions,
  `last_assistant_message` on SubagentStop, and no model-facing output on PostCompact or
  SubagentStop (ICX-AUD-004). Codex: no duplicated `additionalContext` (ICX-AUD-005).
- Anthropic proxy placement: client breakpoints are preserved, icx's breakpoint goes on a
  content block, at most 4 in total, and 1h TTLs are inherited (ICX-AUD-003).
- The bus reports dropped publishes and reader loss instead of returning success
  (ICX-AUD-054).
- FastCDC uses spread masks (mean chunk 13.2 KiB to 10.6 KiB at an 8 KiB target) (ICX-AUD-027).

### Added

- `icx daemon` (resident workspace over a private Unix socket), `icx trust`, `ICX_NO_DAEMON`,
  `ICX_DAEMON_AUTOSTART`, `ICX_STATE_HOME` and `ICX_CONFIG_HOME`.
- MCP dual-era support: `initialize` negotiation for 2024-11-05 through 2025-11-25, plus
  `server/discover` and per-request versions for 2026-07-28 (ICX-AUD-008).
- Optional wall-clock event times, enabled for CLI workspaces (ICX-AUD-048).
- `SeqlockTelemetryBus::dropped_publishes` and `poll_since_with_loss` (ICX-AUD-054).
- proptest invariants for the codec, cargo-fuzz targets (`fuzz/`), and a cargo-deny gate
  (ICX-AUD-039, -040).

### Changed

- The benchmark's cache line is a byte-weighted prefix-reuse proxy over all turns, seal turns
  included (95.46% on the synthetic transcript), gated at 90%. Compression and CAS fixtures are
  labelled and the CAS fixture is no longer a single chunk (ICX-AUD-002, -027, -028).
- CI actions are pinned to commit SHAs; the toolchain is pinned to 1.100.0 (ICX-AUD-040).
- `icx-proxy` and `icx-adapters` declare `GPL-3.0-or-later AND AGPL-3.0-or-later`, matching
  REUSE; LICENSING.md's description of GPL section 13 is corrected (ICX-AUD-034).
- Specifications, threat model, deployment guide, README and the AHTL PDF volume describe v1.1.0
  behaviour; canonical figures are in [`docs/numbers.md`](docs/numbers.md) (ICX-AUD-017, -030,
  -031, -046, -051).

### Removed

- 10 arXiv PDFs whose license does not permit redistribution; INDEX.md links to arXiv
  (ICX-AUD-036).
- Unused proxy dependencies `icx-codec` and `serde` (ICX-AUD-041); `icx-cli` depends on
  `serde` and `rustix` only on Unix.
- Personal paths and an agent conversation id from tracked files (ICX-AUD-038).

### Known limitations

- Fractal suffix depth is still inferred from distance and can report false positives
  (ICX-AUD-052).
- Codex does not yet apply `updatedMCPToolOutput`; under Codex icx indexes output but does not
  shrink it. The Codex rollout tailer does not read `.jsonl.zst` files and is not wired into the
  CLI (ICX-AUD-005).
- No Hermes Python shim ships with icx (ICX-AUD-006).
- The rest of the derived state (DAG, epochs, HDC event log, graph) is still rewritten in full
  on each save, so flush cost grows with history. The daemon flushes after any dispatched
  request, including hooks that change nothing.
- A running `icx proxy` decides at start whether to use the workspace; restart it after
  `icx trust`.
- Licensing and patent governance text (PATENTS.md, CLA.md, COMMERCIAL.md) is unchanged by owner
  decision (ICX-AUD-035).

## [1.0.0] - 2026-11-18

### Added

- **Phase 5 Sprint 2 — Bijective `AhtlClause` Escaping, Enhanced `icx doctor` Workspace Integrity Verifier, 9-SLO `icx benchmark` Suite & `v1.0.0` Production Release (`crates/icx-codec`, `crates/icx-core`, `crates/icx-adapters`, `crates/icx-cli`, `docs/`, `ref-docs/`):**
  - Added bijective `escape_clause_key` (`%` -> `%25`, `:` -> `%3A`, `⋮` -> `%E2%8B%AE`, `⇒` -> `%E2%87%92`, `↦` -> `%E2%86%A6`, `⟦` -> `%E2%9F%A6`, `⟧` -> `%E2%9F%A7`, `\n` -> `%0A`, `\r` -> `%0D`), `escape_clause_val`, and `unescape_clause_field` to [`AhtlClause::render`](crates/icx-codec/src/ahtl.rs) and [`AhtlClause::parse`](crates/icx-codec/src/ahtl.rs) (`T-52-002`) so programmatic `AhtlClause` instances round-trip losslessly regardless of embedded delimiter characters.
  - Upgraded [`CasStore::put_bytes_checked`](crates/icx-core/src/cas.rs) to precompute `Vec<(ChunkLineSpan, ArtifactId, Blake3Digest)>` in a single pass and verify both intra-batch and `self.chunks` 64-bit `cid` collisions before inserting chunks, and added `has_indexed_occurrence_for`, `get_occurrence_record_for`, and allocation-free `is_ppr_stop_word` in [`InfinityContextMmu`](crates/icx-adapters/src/mmu.rs).
  - Upgraded `icx doctor [--dir <PATH>]` (`T-52-001`, [`crates/icx-cli/src/main.rs`](crates/icx-cli/src/main.rs)) with `PATH`-aware binary discovery, `/dev/shm` writability inspection, and full workspace `CasStore::verify_all_integrity()` + `MerkleContextDag::verify_all_merkle_integrity_with_cas(Some(&mmu.cas))` cryptographic verification (`[VERIFIED] Workspace Storage & Merkle-DAG`).
  - Expanded `icx benchmark` (`T-52-001`, [`crates/icx-cli/src/main.rs`](crates/icx-cli/src/main.rs)) to execute and report all 9 SLO rows from [`docs/performance.md`](docs/performance.md) §1 (`10,240-bit HDC Bind + Permute`, `10,240-bit HDC Hamming Scan`, `Fractal IFS Suffix Match`, `Two-Stage HDC + CSR PPR`, `AHTL Compression Ratio`, `FastCDC + BLAKE3 CAS Ingest (64K)`, `Atomic Seqlock Broadcast`, `Full Synchronous Hook Round-Trip`, and `50-Turn NIH + Cache-Epoch`).
  - Synchronized [`docs/ahtl/08-v1-reference-implementation-blueprint.md`](docs/ahtl/08-v1-reference-implementation-blueprint.md), [`docs/performance.md`](docs/performance.md), [`docs/virtual-memory-and-dag-engine.md`](docs/virtual-memory-and-dag-engine.md) (§5 CAS availability/retention/backup policy), [`ref-docs/ahtl-exactness-review-and-analysis.md`](ref-docs/ahtl-exactness-review-and-analysis.md) (§§2.5–2.6 all 7 minimum standards `ENFORCED` in `v1.0.0`), [`docs/deployment-and-agent-integration-guide.md`](docs/deployment-and-agent-integration-guide.md), [`README.md`](README.md), and [`to-dos/ROADMAP.md`](to-dos/ROADMAP.md) (`T-52-002`).
- **Phase 5 Sprint 1 — 500-Turn (`> 2.1M` Token) Needle-in-a-Haystack / OOLONG & Prefix-Cache Simulator, 4-Agent Shared-File `/dev/shm` Telepathy Verification & AHTL Exactness Hardening (`crates/icx-core`, `crates/icx-hdc`, `crates/icx-adapters`, `crates/icx-proxy`):**
  - Added full 256-bit `Blake3Digest` collision verification on both top-level 64-bit `ArtifactId` and `L0` chunk hits in [`CasStore::put_bytes_checked`](crates/icx-core/src/cas.rs) plus single-pass [`CasStore::verify_all_integrity`](crates/icx-core/src/cas.rs) (`T-51-001`).
  - Upgraded [`HdcAssociativeIndex::query_associative`](crates/icx-hdc/src/lib.rs) with adaptive Stage-1 candidate scaling (`(entries.len() / 64).clamp(16, 512)`) so multi-thousand-artifact corpora maintain high Stage-1 recall (`T-51-001`).
  - Upgraded [`InfinityContextMmu`](crates/icx-adapters/src/mmu.rs) with punctuation-stripped, stop-word-filtered PPR seed extraction in `build_ephemeral_tail_slot` and per-agent occurrence isolation (`agent_occurrences`, `agent_reordered_occurrences`, `truncate_agent_suffix`) so multi-agent proxy and Hermes sessions sharing one `InfinityContextMmu` never clobber each other's positional occurrence caches (`T-51-001`, `T-51-002`).
  - Eliminated unnecessary textual response buffer cloning when no `⟪!icx:deref(...)⟫` marker is present in [`WireProxyTransformer::relay_upstream_response_with_eof`](crates/icx-proxy/src/wire.rs).
  - Implemented `sprint_5_1_500_turn_two_million_token_nih_oolong_and_prefix_cache_simulation` (`> 8.4 MB` / `> 2.1M` raw tokens across 500 turns, `>= 94.0%` prefix-cache hit ratio, `< 32,768` active window tokens, 100% passkey recall & byte-exact dereference at turns `3`, `47`, `189`, `412`) and `sprint_5_1_four_agent_shared_shm_telepathy_and_sub_100us_propagation` (`ClaudeCode` -> `Antigravity` -> `Codex` -> `Hermes` -> `McpPagerServer` over shared-file `/dev/shm` ring in `< 100 µs`) in [`crates/icx-adapters/src/lib.rs`](crates/icx-adapters/src/lib.rs) (`T-51-001`, `T-51-002`).

## [0.5.0] - 2026-11-18

### Added

- **Phase 4 Sprint 3 — Single-Tool Polymorphic `McpPagerServer` (`icx`), Multi-Provider + SSE `WireProxyTransformer`, Local HTTP Proxy (`icx proxy`) & `v1.0.0` Deployment Guide (`crates/icx-adapters`, `crates/icx-proxy`, `crates/icx-cli`, `docs/deployment-and-agent-integration-guide.md`):**
  - Expanded [`McpPagerServer`](crates/icx-adapters/src/mcp.rs) (`T-43-001`) with `schema_token_estimate() < 180`, `"expand"` operation alias, `notifications/initialized` and `ping` JSON-RPC methods, `isError` status flagging, structured 16-hex disambiguation guidance on `IcxError::AmbiguousHandlePrefix`, in-band `⟪ICX:PROVENANCE│id=...│redacted=...│exactness=post_sanitization_cas⟫` headers on `deref`/`expand`/`slice` when `FLAG_IS_REDACTED` is set or `provenance: true` is requested, sanitized `fold` branch labels/summaries with `sync_global_dag_root()`, `id`-scoped or global `redacted_artifacts` reporting in `op: "status"`, and bidirectional `op: "xagent"` (publishing onto `mmu.bus` when `q` is non-empty, polling when `q` is empty/omitted).
  - Expanded [`WireProxyTransformer`](crates/icx-proxy/src/wire.rs) (`T-43-002`) with `ProviderFormat` (`Anthropic`, `OpenAI`, `Gemini`), Anthropic content-block array virtualization (`text` and `tool_result` blocks with `icx` pager exemption and `cache_control` stripping), `transform_gemini_request` (`contents[].parts[].text`), `expand_inline_deref_directives` (`⟪!icx:deref(#icx:<hex>)⟫` and `⟪!icx:deref(#icx:<hex>,start:end)⟫`), `transform_sse_chunk`, `transform_for_provider`, `handle_http_request`, and safe `std::net::TcpListener` `serve_http_listener`.
  - Expanded [`icx-cli`](crates/icx-cli/src/main.rs) with `--agent hermes` and `--dir <PATH>` on `icx hook`, `--dir <PATH>` on `icx mcp-serve` and `icx status`, `--provider gemini` and workspace persistence on `icx proxy-transform`, and the `icx proxy` HTTP/1.1 & SSE reverse-proxy subcommand.
  - Authored [`docs/deployment-and-agent-integration-guide.md`](docs/deployment-and-agent-integration-guide.md) (`1,279` lines) covering standalone (`Mode A`) and dual-stack (`Mode B` with `ai-memory`) architecture, setup/configuration for Claude Code, OpenAI Codex CLI, Google Antigravity CLI (`agy`), Hermes Agent, and MCP/ACP clients, prompt-cache coexistence, operational best practices, quantified pros/cons, and 4 end-to-end turn-by-turn use cases.
- **Phase 4 Sprint 2 — Google Antigravity CLI (`agy`) Adapter & Hermes Agent `ContextEngine` / `MemoryProvider` Bridge (`crates/icx-adapters`, `crates/icx-core`):**
  - Expanded [`AntigravityAdapter`](crates/icx-adapters/src/antigravity.rs) (`T-42-001`) with `PostInvocation` (`force_continue`), `PreToolUse` (`overwrite`), `PostToolUse` non-error filtering + bounded `512 KiB` tail-window `transcriptPath` JSONL fallback (`read_transcript_step`), `conversationId` correlation tagging (`attach_conversation_id`), and `Stop` `#icx:<16-hex>` sealed-epoch handle reporting while strictly enforcing process exit code `0`.
  - Expanded [`HermesAdapter`](crates/icx-adapters/src/hermes.rs) (`T-42-002`) to implement the full Hermes `ContextEngine` and `MemoryProvider` (`PRE_COMPRESS_CHECKPOINT_API_VERSION = 2`) contract: `select_context`, `prune_tool_results_only`, `compress`, `on_turn_complete`, `on_pre_compress`, and `handle_hook`, with `tool_calls` group preservation across `protect_first_n` / `mid_slice` / `tail_slice`, multimodal non-text archiving, and occurrence-aligned `ingest_or_reuse_at_index` (`MerkleContextDag::find_turn` / `find_turn_any_role`).
- **Phase 4 Sprint 1 — `AgentHookAdapter` Trait, `ClaudeCodeAdapter`, `CodexAdapter` & `CodexRolloutTailer` (`crates/icx-adapters`, `crates/icx-core`):**
  - Implemented [`AgentHookAdapter`](crates/icx-adapters/src/lib.rs) and `AdapterError`, clear-state functional supersession in [`InfinityContextMmu`](crates/icx-adapters/src/mmu.rs) (`("fail", "0")` and `("status", "PASS")` in `BiTemporalGraph`), atomic workspace snapshot persistence (`mmu_state.json`, `cas_store.json`, `bitemporal_graph.json`, `icx-fabric-v1.shm`), `ClaudeCodeAdapter` (`SessionStart` `ICX:LEGEND`, `PreToolUse`, `PostToolUse` `lines=1..<L>` + `updatedMCPToolOutput`, `SubagentStart`/`SubagentStop`), `CodexAdapter` (strict `additionalProperties: false` wire schemas), and bounded `CodexRolloutTailer` (`T-41-001`, `T-41-002`).

## [0.4.0] - 2026-11-17

### Added

- **Phase 3 Sprint 3 — Bi-Temporal CSR Knowledge Graph (`CsrGraphSnapshot`), Functional Contradiction Supersession & HippoRAG Personalized PageRank (`crates/icx-store`, `crates/icx-adapters`, `crates/icx-cli`):**
  - Enhanced [`BiTemporalGraph::upsert_edge`](crates/icx-store/src/graph.rs) (`T-33-001`) so any existing active edge (`t_invalid.is_none()`) sharing `(source_entity, relation)` on a functional relation (`is_functional == true`) is automatically superseded (`t_invalid = Some(now_ms)`, `t_tx_end = Some(now_ms)`), guaranteeing at most one active edge per functional `(source_entity, relation)` pair across all timestamps.
  - Added [`CsrGraphSnapshot`](crates/icx-store/src/graph.rs) (`nodes`, `row_ptr`, `col_idx`, `weights`, `inv_out_weight`, `prov_row_ptr`, `prov_ids`), `BiTemporalGraph::build_csr`, and `CsrGraphSnapshot::run_ppr` (`T-33-002`) executing HippoRAG Personalized PageRank (`arXiv:2405.14831`) power iteration over flat cache-contiguous Compressed Sparse Row (`CSR`) arrays with $O(V + E)$ scalar dangling-node mass redistribution.
  - Added `BiTemporalGraph::{entity_count, edges, query_edges_to, query_relation, invalidate_edge, save_to_dir, load_from_dir}` for incoming/relation time-travel queries, explicit invalidation, and atomic JSON persistence (`bitemporal_graph.json`).
  - Expanded [`InfinityContextMmu::ingest_payload`](crates/icx-adapters/src/mmu.rs) functional-relation classification (`status`, `test_status`, `return_type`, `fail`, `pass`, `exit_code`) and added two-stage `HdcAssociativeIndex` + CSR `PersonalizedPageRank` latency verification to `icx benchmark` (`crates/icx-cli/src/main.rs`).
- **Phase 3 Sprint 2 — Contractive IFS `FractalTrajectoryArchive`, Boundary-Aware Inverse-IFS Suffix Verification & `TitansSurpriseGate` Momentum Decay (`crates/icx-hdc`):**
  - Expanded [`FractalCoord`](crates/icx-hdc/src/fractal.rs) (`T-32-001`) with `euclidean_distance` ($L_2$), boundary-aware inverse-IFS suffix verification (`verified_suffix_depth`), signed `Q1.14` fixed-point quantization (`to_q1_14` / `from_q1_14`, scale $2^{14} = 16,384$), uniform half-open dyadic 64-bit Morton Z-order prefix codes (`morton_z64`), `@o1:o2` octal quadrant addressing (`encode_octal_addr` / `decode_octal_addr`), and [`FractalTrajectoryArchive`](crates/icx-hdc/src/fractal.rs) (`record_step`, `insert_coord`, `query_shared_suffix`), verified against Theorem 3.2 contractive bounds ($d_\infty \le 2^{1-K}$ and $d_2 \le 4 d_\infty \le 2^{3-K}$) across all $K \in 1..=32$.
  - Expanded [`TitansSurpriseGate`](crates/icx-hdc/src/surprise.rs) (`T-32-002`) with `RetentionTier::PinVerbatim`, `with_eta_and_alpha` (with `NaN` sanitization), `momentum()`, `eta()`, `alpha()`, `events_seen()`, `memory_trace()`, uncapped exponential idle decay `apply_idle_decay(steps)` ($S_t \leftarrow (1 - \alpha)^{\text{steps}} S_t$), `observe_with_exit_code`, and Delta-Rule novelty gating (`is_anomaly` with $\nu_t \ge 0.05$, or routine with $\nu_t \ge 0.15$) so repetitive boilerplate never saturates $M_t$.
  - Integrated `FractalTrajectoryArchive` and `observe_event_with_exit_code` into [`HdcAssociativeIndex`](crates/icx-hdc/src/lib.rs).
- **Phase 3 Sprint 1 — 10,240-Bit HDC/HRR Algebraic Engine, Infini-Attention Delta Bundler & Two-Stage Associative Index (`crates/icx-hdc`):**
  - Expanded [`Hypervector`](crates/icx-hdc/src/hv.rs) (`T-31-001`) with `BitXor`/`BitXorAssign` (`^`, `^=`), `unbind`, `inverse_permute` ($\Pi_{-s} \Pi_s = I$), `from_seed_framed` (4-byte little-endian length-framed domain/token hashing), `bind_role_filler`, `encode_sequence`, `popcount`, `cosine_bipolar`, `sub_hypervector_64`, and non-linear 128-bit `dedup_fingerprint_u128`.
  - Upgraded [`HdcBundler`](crates/icx-hdc/src/hv.rs) (`T-31-002`) with `BTreeSet<u128>` non-linear fingerprint deduplication (recorded only after the `sim < 0.85` similarity gate accepts a vector), `add_weighted_unique`, deterministic coordinate-parity tie-breaking (`((word_idx ^ bit) & 1) == 0`), `count()`, `reset()`, and `unbind_query()`.
  - Defined [`AssociativeMemoryIndex`](crates/icx-hdc/src/lib.rs) and upgraded [`HdcAssociativeIndex::query_associative`](crates/icx-hdc/src/lib.rs) with a two-stage L2-cache pre-screening pipeline over 16-byte `EntrySummary` records (`hv_sub64`, `subj_bloom32`, `quadrant_u16`, `surprise_q8`, `flags`).

## [0.3.0] - 2026-11-17

### Added

- **Phase 2 Sprint 3 — 4-Tier Pre-Ingestion `SecretRedactor`, Shannon Character Entropy Gate, `FLAG_IS_REDACTED` Tagging & AHTL v1.0 Volume Cross-Verification (`crates/icx-codec`, `crates/icx-adapters`, `crates/icx-hdc`, `docs/ahtl/`):**
  - Expanded [`SecretRedactor`](crates/icx-codec/src/redact.rs) into a 4-tier defense-in-depth scrubbing pipeline (`T-23-001`):
    - **Tier 0 — AHTL Structural Delimiter Neutralization:** `SecretRedactor::escape_ahtl_delimiters` escapes `⟪` -> `<<`, `⟫` -> `>>`, `⟦` -> `[[`, and `⟧` -> `]]` so untrusted payloads can never forge `AhtlPacket`, `AhtlWirePacketV1`, or `EpochHeader` boundaries.
    - **Tier 1 — Multi-Line Cryptographic Block Removal:** Stateful stripping of `-----BEGIN ... PRIVATE KEY-----` PEM blocks (`RSA`, `EC`, `OPENSSH`, `DSA`, `ENCRYPTED`) and `-----BEGIN PGP PRIVATE KEY BLOCK-----` blocks.
    - **Tier 2 — 28 Vendor Credential Prefixes, JWTs, Connection URLs, CLI Flags & Key-Value Assignments:** Cursor-advancing token scanner for Anthropic (`sk-ant-*`), OpenAI (`sk-proj-*`, `sk-svcacct-*`, `sk-*`), Stripe (`sk_live_*`, `rk_live_*`, `sk_test_*`, `rk_test_*`, `whsec_*`), GitHub (`ghp_*`, `gho_*`, `ghu_*`, `ghs_*`, `ghr_*`, `github_pat_*`), AWS (`AKIA*`, `ASIA*`), GCP (`AIza*`), Hugging Face (`hf_*`), NPM (`npm_*`), PyPI (`pypi-*`), Slack (`xox[bparso]-*`), `ai-memory` (`aim_*`), 3-segment Base64URL JWT tokens (`eyJ...`), database/broker connection URLs (`postgres://`, `postgresql://`, `mysql://`, `mongodb://`, `mongodb+srv://`, `redis://`, `rediss://`, `amqp://`, `amqps://`), CLI flags (`--api-key=`, `--token=`, `--password=`, `--secret=`, etc.), `Authorization: Bearer/Basic`, and single-line/multi-key `.env`, JSON, and YAML secret assignments (including backslash-escaped quotes `\"`).
    - **Tier 3 — Shannon Character Entropy Gate ($H(S)$):** `SecretRedactor::shannon_entropy` computes character Shannon entropy $H(S) = -\sum P(c)\log_2 P(c)$ in bits/char and redacts unprefixed high-entropy Base64/mixed tokens ($|S| \ge 24$, $H(S) > 4.35$) and hex assignment tokens ($|S| \ge 32$, $H(S) > 3.50$) while exempting file paths, URLs, source code identifiers, and `#icx:`/`fc:`/`hv:` handles.
  - Updated [`InfinityContextMmu::ingest_payload`](crates/icx-adapters/src/mmu.rs) to set `FLAG_IS_REDACTED` (`1 << 7`) on `receipt.hologram_bytes[7]` whenever `SecretRedactor` scrubs content, verified by end-to-end zero-leakage tests across CAS, Merkle-DAG, HDC, Bi-Temporal Graph, and `/dev/shm` Seqlock Bus.
  - Cross-referenced all 57 pages of [`docs/ahtl/AHTL_v1_Complete_Technical_Specification_Volume.pdf`](docs/ahtl/AHTL_v1_Complete_Technical_Specification_Volume.pdf) against the workspace implementation: added 4-way unrolled [`Hypervector::hamming_distance_simd`](crates/icx-hdc/src/hv.rs) (Chapter 08 §3), aligned `ChannelBHologram` byte `B[7]` (`TIER_MASK = 0b11` bits `0..=1` + `FLAG_HAS_ANOMALY`/`FLAG_IS_MUTATED`/`FLAG_IS_FOLDED`/`FLAG_IS_SEALED`/`FLAG_IS_TELEPATHY`/`FLAG_IS_REDACTED` bits `2..=7`, Chapter 02 §4.1), and pinned Canonical Test Vectors 1 & 2 in [`crates/icx-codec/src/ahtl.rs`](crates/icx-codec/src/ahtl.rs) (Chapter 01 §§6.1–6.2).
- **Phase 2 Sprint 2 — Multi-Language AST/Lexical, Unified Diff & Multi-Ecosystem Diagnostic Log `StructuralSkeletonizer` (`crates/icx-codec/src/skeleton.rs`):**
  - Expanded [`StructuralSkeletonizer::skeletonize_source`](crates/icx-codec/src/skeleton.rs) (`T-22-001`) to extract top-level declarations (`fn`, `struct`, `enum`, `trait`, `impl`, `class`, `interface`, `type`) across `Rust`, `Python`, `TypeScript/JavaScript`, `Go`, and `C/C++` with quote-, Rust-lifetime- (`'a`), and comment-aware brace/indentation line-span tracking (`L<start>-<end>`), contiguous header import folding (`imports:L<a>-<b>(N)` including Go `import (...)` blocks), module headers (`mod@L<n>`), and paired `--- `/`+++ ` unified `git diff` file & `@@ ... @@` hunk extraction.
  - Expanded [`StructuralSkeletonizer::skeletonize_log`](crates/icx-codec/src/skeleton.rs) (`T-22-002`) with line-anchored multi-ecosystem compiler and test-runner log detection and compression (`cargo test`/`clippy`, `pytest`, `tsc`/`vitest`/`jest`, `go test`), achieving `>= 25x` (`39.6x` measured) compression on 500-test logs while unconditionally preserving 100% of failing test names (`!FAIL:<test_name>`) and deduplicating repeated pytest summary entries.
- **Phase 2 Sprint 1 — AHTL v1.0 Dual-Profile Codec, Braille-256 Bijection & 1-Lipschitz 64-Band Spectral Projection (`crates/icx-codec`, `crates/icx-hdc`, `crates/icx-cli`):**
  - Implemented [`byte_to_braille_utf8`](crates/icx-codec/src/braille.rs), `braille_utf8_to_byte`, `encode_braille_64`, `decode_braille_64`, `sketch_hamming_distance`, `spectral_hamming_distance_64`, and `ChannelBHologram` in `crates/icx-codec/src/braille.rs`, verified by exhaustive 256-byte bijection and 10,000-sample randomized Hamming isometry property tests (`T-21-001`).
  - Implemented [`Hypervector::from_u64_seed`](crates/icx-hdc/src/hv.rs) (`SplitMix64`), `Hypervector::spectral_bands_64` (1-Lipschitz 20-bit sub-block majority voting + 8-sub-block byte packing across 160 `u64` words), `Hypervector::hv_digest_u64`, and [`FractalCoord::encode_fc_hex64`](crates/icx-hdc/src/fractal.rs) / `decode_fc_hex64` (`T-21-001`).
  - Implemented Profile A [`AhtlWirePacketV1`](crates/icx-codec/src/ahtl.rs) (`⟦AHTL v1│#icx:<12-hex>│fc:<64-hex>│hv:<16-hex>│<64-Braille>│<skeleton>⟧`), enhanced Profile B `AhtlPacket` (`slice_hint`, `hologram_tag`, `compact_48bit_handle`, bijective `%25`/`%23L` subject escaping, `⇒` causal chains), `EpochHeader`, `WirePacket`, `AhtlStream`, `ContextCodec` trait, and `icx encode-ahtl --profile <a|b>` CLI support (`T-21-002`).

## [0.2.0] - 2026-11-17

### Added

- **Phase 1 Sprint 4 — 100% Zero-`unsafe` Conversion, Permanent Workspace-Wide `unsafe_code = "forbid"` Gate & `>= 95%` Codecov Enforcement (`crates/icx-bus`, `crates/icx-proxy`, `crates/icx-adapters`, `crates/icx-cli`, `.github/workflows/ci.yml`):**
  - Refactored `SeqlockTelemetryBus`, `FabricHeader` (64 B), and `SeqlockSlot` (4,096 B) in `crates/icx-bus/src/ring.rs` to 100% safe `AtomicU32` and `AtomicU64` word-packed structures (`[AtomicU64; 496]` for `ahtl_words`, `[AtomicU64; 4]` for `hdc_sketch`, `[AtomicU64; 4]` for `fractal_q15_words`, `AtomicU64` for `packed_meta`) and safe `/dev/shm` positional I/O (`std::os::unix::fs::FileExt::{read_exact_at, write_all_at}` + `File::try_lock`), removing `memmap2` and eliminating all 11 `unsafe` blocks (`T-14-002`).
  - Enforced `[lints] workspace = true` (`unsafe_code = "forbid"`) across all 8 workspace crates and added an automated `Enforce workspace-wide zero-unsafe policy` CI step in `.github/workflows/ci.yml` (`T-14-003`).
  - Added `WireProxyTransformer::transform_openai_request` (`crates/icx-proxy/src/wire.rs`) and expanded unit and integration test suites across all 8 crates to achieve **`96.13%` workspace line coverage** (`>= 95.20%` in every crate), raising the CI `cargo-llvm-cov` floor to `--fail-under-lines 95` and `codecov.yml` targets to `95%` (`T-14-004`).
- **Phase 1 Sprint 3 — Lock-Free `/dev/shm` Seqlock Telepathy Bus, Workspace Isolation & CLI `init`/`deref` (`crates/icx-bus`, `crates/icx-adapters`, `crates/icx-cli`):**
  - Enriched 64-byte `FabricHeader` (`root_seqlock: AtomicU32`, `active_agents_mask: AtomicU64`, `global_dag_root: [AtomicU64; 4]`) and 4,096-byte `SeqlockSlot` (`packet_kind: u8`) with compile-time size and alignment assertions (`T-13-001`).
  - Hardened `SeqlockTelemetryBus::open_shared_file` with `0o600` owner-only Unix permissions, magic (`ICX_SHM_MAGIC`) and schema version (`ICX_SHM_VERSION`) validation (`BusError::UnsupportedVersion`, `BusError::InsecurePermissions`), per-slot atomic `compare_exchange_weak` writer serialization, UTF-8 char-boundary-safe inline packet copying, volatile slot accesses, `poll_workspace_since`, and the 8-writer × 8-reader × 10,000-message concurrent Seqlock stress test (`T-13-002`).
  - Wired `InfinityContextMmu` to update `FabricHeader::global_dag_root` on epoch rollover and poll workspace-scoped telepathy via `poll_workspace_since`, and added `icx init` and `icx deref <HANDLE> [--lines START:END]` CLI subcommands.
- **Phase 1 Sprint 2 — Lossless Merkle-DAG (`LCM`), Nested Context-Folding & Cache-Epoch Budget Retargeting (`crates/icx-core`):**
  - Added structured `ToolArtifactNode` (`DagNode::ToolArtifact`) with domain-separated `b"ICX_DAG_TOOL_V1"` hashing (`ToolArtifactNode::compute_merkle_digest`) and `MerkleContextDag::append_tool_artifact` (`T-12-001`).
  - Upgraded `MerkleContextDag` branch state to a nested stack (`branch_stack: Vec<OpenBranch>`, `active_branch_depth`), added `expand_branch` for lossless recursive `L2 -> L1` branch expansion, added `fold_epochs` for `L3 -> L4` hierarchical epoch compaction, and added `verify_merkle_integrity` for cryptographic verification of any `L1`, `L2`, `L3`, or `L4` node (`T-12-001`, `T-12-002`).
  - Added provider-tuned `EpochControllerConfig::anthropic_default()` (`16,384` rollover tokens, `6` protected turns, `2,048` tail slot) and `openai_default()` (`8,192` rollover tokens), `CacheEpochController::record_node`, `zone1_compressed_tokens`, level-order pairwise `retarget_budget` (`L3 -> L4` folding), and the 200-turn Prefix-Cache Simulation Test verifying `>= 94%` average prefix-cache hit rate (`T-12-003`).
- **Phase 1 Sprint 1 — Normalized FastCDC, Dual-Profile `#icx:<hex>` Handle Resolution & Selective Chunk Line Slicing (`crates/icx-core`, `crates/icx-adapters`):**
  - Implemented `ArtifactId::parse_prefix_range`, `ArtifactId::to_short_hex12`, `ArtifactId::to_short_handle`, and `CasStore::resolve_handle` supporting both 64-bit (`#icx:<16-hex>`) and 48-bit (`#icx:<12-hex>`) handles with $O(\log N)$ `BTreeMap::range` lookup and fail-closed prefix ambiguity detection (`IcxError::AmbiguousHandlePrefix`), wired into `McpPagerServer` (`deref`, `slice`, `grep`).
  - Upgraded `chunk_bytes` (`crates/icx-core/src/chunker.rs`) to two-threshold Normalized FastCDC (`mask_s` before `avg_size`, `mask_l` from `avg_size` to `max_size`) and added streaming `chunk_bytes_with_lines` (`ChunkLineSpan`).
  - Added `LineIndexEntry` to `StoredArtifact` (`crates/icx-core/src/cas.rs`), enabling `CasStore::deref_lines` to verify and read only the intersecting `L0` chunks for `[start_line..=end_line]`, added `put_bytes_checked` (`ICX_MAX_INGEST_BYTES = 64 MiB` STRIDE DoS bound), `grep_cold_with_context`, `ContentAddressedStore` trait implementation, and atomic directory persistence (`save_to_dir` / `load_from_dir`).
- **AHTL v1.0 Master Specification Volume & Foundational Reference Archives (`docs/ahtl/`, `ref-docs/`):**
  - Complete 9-document normative specification suite ([`docs/ahtl/README.md`](docs/ahtl/README.md) and Chapters `01`–`08`) and 55-page printable technical volume ([`docs/ahtl/AHTL_v1_Complete_Technical_Specification_Volume.pdf`](docs/ahtl/AHTL_v1_Complete_Technical_Specification_Volume.pdf)) covering ISO/IEC 14977 EBNF grammar, ISO/TR 11548-1 Braille-256 spectral bijection (`0x00..0xFF`), contractive IFS Chaos-Game coordinates (`fc:<64-hex>`), $Q_{10240}$ Boolean hypercube HDC algebra (`hv:<16-hex>`), homomorphic AST skeletonization & 3-tier secret redaction, FastCDC + BLAKE3 Merkle-DAG virtual handles (`#icx:<hex>`), Transformer induction-head attention mechanics, and the v1.0 Rust reference implementation blueprint.
  - Four primary reference archives in `ref-docs/` separating generated diagrams (`images/generated/`) from retrieved archival figures (`images/retrieved/`): [`ref-docs/unicode-braille-patterns/`](ref-docs/unicode-braille-patterns/README.md), [`ref-docs/blake3-cryptographic-hash/`](ref-docs/blake3-cryptographic-hash/README.md), [`ref-docs/hypercube-and-hamming-geometry/`](ref-docs/hypercube-and-hamming-geometry/README.md), and [`ref-docs/ahtl-foundations/`](ref-docs/ahtl-foundations/README.md).
- **Automated Antigravity CLI (`agy`) Pull Request Review & CI/CD Hardening (`.github/`, `scripts/`):**
  - Self-hosted Antigravity PR review workflow ([`.github/workflows/antigravity-review.yml`](.github/workflows/antigravity-review.yml)), review rubric ([`.github/agy-review.md`](.github/agy-review.md)), deterministic parser and deduplicating GitHub comment publisher ([`scripts/agy-review.sh`](scripts/agy-review.sh)), and 16-case offline regression harness ([`scripts/agy-review-selftest.sh`](scripts/agy-review-selftest.sh)).
  - Automated Contributor License Agreement (`CLA.md`) verification workflow ([`.github/workflows/cla.yml`](.github/workflows/cla.yml)) and commit-author / co-author, PR-description, and PR-comment signature verifier with offline selftest suite ([`.github/scripts/cla-check.sh`](.github/scripts/cla-check.sh)).
  - Expanded CI pipeline ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) adding `cargo-llvm-cov` LCOV workspace coverage reporting with `codecov/codecov-action@v5` and checksum-pinned `gitleaks` `v8.30.1` secret scanning alongside `check` and `agy` self-tests.
  - Repository code ownership ([`.github/CODEOWNERS`](.github/CODEOWNERS)), weekly Cargo + GitHub Actions dependency updates ([`.github/dependabot.yml`](.github/dependabot.yml)), Codecov project/patch/component coverage configuration ([`codecov.yml`](codecov.yml)), Gitleaks ruleset extension ([`.gitleaks.toml`](.gitleaks.toml)), CodeRabbit v2 review configuration ([`.coderabbit.yaml`](.coderabbit.yaml)), and GitHub Copilot instructions ([`.github/copilot-instructions.md`](.github/copilot-instructions.md)).

### Changed

- **Toolchain, Cargo Workspace Dependencies & GitHub Actions Bumps (`Cargo.toml`, `.github/workflows/`):**
  - Upgraded workspace Rust edition to `2024` and MSRV (`rust-version`) to `1.100` (`rustc 1.100.0`).
  - Bumped workspace crate version requirements to current latest releases (`blake3 = "1.8.7"`, `serde = "1.0.229"`, `serde_json = "1.0.154"`, `thiserror = "2.0.21"`, `memmap2 = "0.9.12"`).
  - Bumped `actions/checkout` from `v4` to `v7` across `.github/workflows/ci.yml`, `.github/workflows/cla.yml`, and `.github/workflows/reuse.yml`, and bumped `codecov/codecov-action` from `v5` to `v7` in `.github/workflows/ci.yml`.
- **Five-Plane Patent-Pending Notice & README Badge Matrix ([`PATENTS.md`](PATENTS.md), [`README.md`](README.md)):**
  - Reconciled [`PATENTS.md`](PATENTS.md) to explicitly enumerate the five pending invention families across all five architectural planes of **Infinity-Context (`icx`)** (`AHTL` & Braille-256 symbolic codec, Virtual Context MMU & 4-tier `BLAKE3` Merkle-DAG with subtask context-folding, 10,240-bit `HDC` + Delta-Rule bundler + Contractive IFS `FCGA` archive, Titans surprise-gated bi-temporal `PPR` + 4-zone prefix-cache epoch controller, and lock-free `/dev/shm` seqlock telepathy ring bus + multi-harness hook fabric + polymorphic `icx` MCP pager).
  - Optimized the [`README.md`](README.md) hero badge block into four balanced, width-verified rows covering CI, REUSE 3.3, CLA, Codecov, Antigravity (`agy`) PR review, Gitleaks, licensing, 5 pending patent families, verified performance benchmarks, and supported agent harnesses.

## [0.1.0] - 2026-11-16

### Added

- **Licensing, Patent-Pending IP Protection & REUSE 3.3 Governance:**
  - Multi-tier IP protection framework ([`LICENSING.md`](LICENSING.md), [`PATENTS.md`](PATENTS.md), [`COMMERCIAL.md`](COMMERCIAL.md), [`CLA.md`](CLA.md), [`REUSE.toml`](REUSE.toml), [`LICENSES/`](LICENSES/)) verified at 100% REUSE 3.3 compliance (`uvx reuse lint`): `GPL-3.0-or-later` (`LICENSE` / `COPYING`) for core workspace code, `GPL-3.0-or-later AND AGPL-3.0-or-later` for networked proxy/MCP crates (`icx-proxy`, `icx-adapters`), `CC-BY-NC-ND-4.0 AND LicenseRef-InfinityContext-Commercial` for specifications, mathematical derivations, and artwork, plus patent-pending and commercial reservation in `PATENTS.md` and `COMMERCIAL.md`.
- **Research & Forensics Corpus (`ref-docs/`):**
  - Initial project concept (`ref-docs/initial-concept.md`) and 38-source Deep-Research Technical Report (`ref-docs/research-report.md`).
  - Downloaded 22 primary arXiv research PDFs into `ref-docs/papers/` with full mathematical synthesis (`ref-docs/papers/INDEX.md`).
  - Reverse-Engineer-Anything (`rea`) binary and SQLite forensic ledger covering Claude Code `2.1.326`, OpenAI Codex `0.169.0`, Google Antigravity CLI `agy`, Hermes Agent, and `ai-memory` (`ref-docs/agent-internals/rea-binary-and-sqlite-forensics.md`).
  - Cross-agent hook and wire interception matrix (`ref-docs/agent-internals/cross-agent-hook-and-wire-matrix.md`), industry prior-art comparison (`ref-docs/prior-art/github-and-industry-landscape.md`), and mathematical derivations of `AHTL`, 10,240-bit `HDC/HRR`, Contractive IFS `FCGA`, and Titans Surprise Gating (`ref-docs/novel-methods/non-human-neural-codecs-and-holographic-context.md`).
- **Implementation Specifications (`docs/`):**
  - 13 implementation-grade specification documents (`architecture.md`, `alien-holographic-codec.md`, `virtual-memory-and-dag-engine.md`, `hyperdimensional-and-fractal-index.md`, `agent-adapters-and-interception.md`, `wire-protocol-and-shm-bus.md`, `data-model.md`, `interfaces.md`, `performance.md`, `security-and-threat-model.md`, `testing-strategy.md`, `build-and-tooling.md`, `glossary.md`).
  - 4 Michael Nygard Architecture Decision Records in `docs/adr/` (`ADR-0001` through `ADR-0004`).
  - Normative Mermaid diagram sources in `docs/diagrams/` and dark/light vector SVGs in `docs/images/`.
- **Hierarchical Roadmap & Sprint Plans (`to-dos/`):**
  - 5-Phase, 14-Sprint hierarchical roadmap (`to-dos/ROADMAP.md`), Phase 0 precursory setup checklist (`to-dos/phase-0-precursory-setup/overview.md`), and ticket specifications `T-11-001` through `T-52-002`.
- **8-Crate Rust Workspace (`crates/`):**
  - `icx-core`: `IcxHandle` (`#icx:<16-hex>`), Gear-hash `FastCdcChunker`, `zstd`-compressed `BLAKE3` `CasStore`, 4-level `MerkleContextDag` (`LCM`), and `CacheEpochController`.
  - `icx-hdc`: 10,240-bit `Hypervector` (`[u64; 160]`), Infini-attention Delta-Rule `HdcBundler`, Contractive IFS `FractalCoord`, `TitansSurpriseGate`, and `HdcAssociativeIndex`.
  - `icx-codec`: `AhtlPacket` non-human BPE-aligned symbolic codec, Unicode Braille-256 bijection (`U+2800..U+28FF`), `StructuralSkeletonizer`, and pre-hash `SecretRedactor`.
  - `icx-store`: `BiTemporalGraph` (`[t_valid, t_invalid)` x `[t_created, t_expired)`) and HippoRAG `PersonalizedPageRank` (`PPR`).
  - `icx-bus`: Lock-free POSIX `/dev/shm/icx-fabric-v1.shm` `SeqlockTelemetryBus` (`512 x 2,048-byte` `#[repr(C, align(64))]` slots).
  - `icx-adapters`: `InfinityContextMmu`, `ClaudeCodeAdapter`, `CodexAdapter`, `AntigravityAdapter` (strict exit code `0`), `HermesAdapter`, and Single-Tool Polymorphic `McpPagerServer`.
  - `icx-proxy`: `WireProxyTransformer` for Anthropic 4-slot `cache_control: {"type": "ephemeral"}` and OpenAI 1,024-token prefix cache alignment.
  - `icx-cli`: `icx` binary supporting `hook`, `mcp-serve`, `proxy-transform`, `encode-ahtl`, `decode-ahtl`, `status`, `doctor`, and `benchmark`.
- **Branding, Icons & Relationship Graphs (`images/`, `assets/`):**
  - Graphviz relationship diagrams (`images/infinity-context-architecture.{dot,png,svg}` and `images/infinity-context-crate-graph.{dot,png,svg}`).
  - Multi-resolution icon packs (`assets/icons/`), 3:1 wide banners (`assets/branding/icx-banner-{1500x500,1200x400,900x300}.png`), full 16:9 hero banner (`images/InfinityContext_Full-Logo.png`), web manifest (`assets/web/`), Linux `.desktop` launcher (`assets/linux/`), and `1280x640` GitHub Social Preview card (`assets/branding/icx-social-preview-1280x640.{png,svg}`).
