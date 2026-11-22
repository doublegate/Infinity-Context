# Below-Hook Findings: Agent Internals Investigation, 2026-11-22

**Scope:** v1.4.0 Phase 6 Sprint 1 (issue #40), tickets T-61-001 to T-61-007.

**Sources:**

- each agent's documentation, changelog and public issues;
- the public source of the open-source agents (Codex, Hermes), at the installed tag or commit;
- the behaviour of the owner's own installed versions, examined for interoperability.

**Confidentiality:** this page follows
[ADR-0008](../../docs/adr/0008-reverse-engineering-output-stays-private.md). It states
interoperability conclusions only: which surfaces, fields and limits icx can rely on.
Open-source agents are cited by file and line. Everything else, including per-agent scope and
evidence, is kept privately, outside the repository.

**Baseline:** this page supplements the two existing files in this folder, and neither is
edited:

- [rea-binary-and-sqlite-forensics.md](rea-binary-and-sqlite-forensics.md), the 2026-11-16
  survey;
- [cross-agent-hook-and-wire-matrix.md](cross-agent-hook-and-wire-matrix.md).

**Labels used below:**

- **observed:** confirmed on the installed version or in its public source;
- **source:** read in the public source at the installed tag or commit;
- **docs:** taken from vendor documentation;
- **inferred:** concluded from the above but not seen directly;
- **unverified:** still needs a live run before icx relies on it.

## 1. Scope (T-61-001)

**What was covered:** the five agents icx integrates with, at the versions in section 4.

**Licences of the open-source agents:**

- Codex (`openai/codex`) is Apache-2.0;
- Hermes (`NousResearch/hermes-agent`) is MIT.

**Where the rest is recorded:** the per-agent scope decisions and the terms review behind them
are recorded in the owner's private notes, not here.

**Build provenance of the open-source agents (observed):**

- The installed Codex binary was built from `rust-v0.171.0`. Toolchain, embedded commit and every
  recovered dependency version match the tag's lockfile.
- The installed Hermes runs unmodified from its `v0.23.3` checkout, and its compiled bytecode
  matches the source.

## 2. Findings per agent

### Claude Code 2.1.330 (T-61-002)

**Tool-output replacement (PostToolUse):**

- **Validation:** a `hookSpecificOutput.updatedToolOutput` value is checked against the tool's own
  output schema, then against its renderer. If either check fails, the original output is used
  (observed).
- **Visibility of a failure:** the failure is logged and the user sees a hook-error notice
  (observed). On 2.1.233 it was visible only in debug output (issue #86753).
- **Precedence:** `updatedToolOutput` wins over `updatedMCPToolOutput`. The MCP-only field is used
  only when `updatedToolOutput` is absent, and an empty value is ignored (observed).
- **MCP tools:** their replacements are not schema-validated (observed).
- **Fields each built-in tool needs for a replacement to be accepted (observed):**

  | Tool | Required fields |
  | --- | --- |
  | Bash | `stdout`, `stderr`, `interrupted` (`isImage` optional) |
  | Read (text) | `type`, and `file` with `filePath`, `content`, `numLines`, `startLine`, `totalLines` |
  | Grep | `numFiles`, `filenames` |
  | Glob | `durationMs`, `numFiles`, `filenames`, `truncated` |
  | WebFetch | `bytes`, `code`, `codeText`, `result`, `durationMs`, `url` |
  | Write | `type`, `filePath`, `content`, `structuredPatch`, `originalFile` |
  | Edit | `filePath`, `oldString`, `newString`, `originalFile`, `structuredPatch`, `userModified`, `replaceAll` |

  icx's shape-preserving replacement satisfies Bash, Read, Grep, Glob and WebFetch (inferred from
  the schemas). WebSearch and Agent are not resolved.

**Plan mode:**

- An MCP tool without `annotations.readOnlyHint: true` is held with "Cannot call ... while in plan
  mode" before allow rules are consulted (observed).
- Neither a `permissions.allow` entry nor a hook or plugin allow lifts the hold (observed).
- `readOnlyHint` is the only signal that marks an MCP tool read-only, and it also lets calls run
  concurrently (observed). A request for another route, issue #90058, was closed as not planned.
- The icx tool declares no annotations, which explains the plan-mode prompts.

**Compaction:**

- **PreCompact** receives `trigger` and `custom_instructions`. Its stdout is appended to the
  summarizer's instructions, up to 10,000 characters, and a blocking reply stops compaction
  (observed, docs). Inside subagents only blocking is honoured.
- **PostCompact** receives `trigger` and `compact_summary`. Its output is shown to the user only and
  never reaches the model (observed).
- **Re-injecting context after compaction** works only through the SessionStart that follows it.

**`additionalContext`:**

- It is dropped unless `hookEventName` matches the event exactly (observed).
- Large context is written to a file, and the model sees a preview and the path.
- On the hook path icx uses (hooks from settings files), context is capped at 8,000 characters
  and 200 lines (observed).

**Events:**

- 33 events in total.
- Payloads confirmed for `WorktreeCreate`, `WorktreeRemove`, `CwdChanged`, `FileChanged`,
  `PostToolBatch` (`tool_calls[]`, may add context) and `PreModelSwitch`/`PostModelSwitch` (model
  ids, source, context tokens, prompt-cache warmth and TTL). Pre can allow, deny or ask.

**Local IPC:**

- The session transcript is `<config>/projects/<sanitized-cwd>/<id>.jsonl`.
- IDE lock files and an internal socket transport exist but are undocumented, and icx does not
  use them.

### Codex CLI 0.171.0 (T-61-003)

All paths are relative to `codex-rs/` at tag `rust-v0.171.0` (source). In the installed binary, code
references the message of each of the four hook behaviours below (observed: two found by
Ghidra, two by a direct scan of the machine code).

**Hook replies are parsed strictly:**

- Every output schema uses `deny_unknown_fields` (`hooks/src/schema.rs:89-100`, `143-156`,
  `229-240`).
- **`updatedToolOutput`** is an unknown field. A reply that contains it fails to parse as a whole
  ("invalid post-tool-use JSON output", `post_tool_use.rs:256-261`).
- **`updatedMCPToolOutput`** marks the hook Failed ("unsupported updatedMCPToolOutput",
  `output_parser.rs:431-439`), and that hook's `additionalContext` is dropped too
  (`post_tool_use.rs:204-215`).
- **`suppressOutput: true`** is rejected as unsupported (`output_parser.rs:393-399`).

**How each reply reaches the model:**

- **`decision: "block"`** needs a reason (`output_parser.rs:215-226`). The model receives the reason
  as a failed tool call (`registry.rs:1080-1088`).
- **`continue: false` with `reason` or `stopReason`** on PostToolUse: the model sees only that
  text in place of the result. The call is not marked failed, logs keep the original output, and
  the turn continues (`registry.rs:371-393`, `1089-1096`; `post_tool_use.rs:44-49`). This is the
  only whole-result substitution available on this version.
- **`additionalContext`** is subject to a per-handler `additionalContextLimit`, default about
  2,500 tokens, where 0 means no limit (`output_spill.rs:13`). Text over the limit goes to a file,
  and the model gets a preview and the path (`output_spill.rs:127`). The limit applies to
  PreToolUse, PostToolUse, SessionStart, UserPromptSubmit and SubagentStart
  (`discovery.rs:695-712`).
- **Which calls get PostToolUse:**
  - shell commands appear as tool `Bash`, and their result always counts as success
    (`handlers/unified_exec.rs:131-149`);
  - MCP tools appear as `mcp__<server>__<tool>`;
  - PostToolUse runs only for calls that succeed, so a failed MCP call gets none.

**Rollouts:**

- **Path:** `~/.codex/sessions/YYYY/MM/DD/rollout-<ts>-<thread>[_<id>].jsonl`
  (`rollout/src/recorder.rs:1701-1723`).
- **Line format:** `{timestamp, ordinal?, type, payload[, metadata]}`.
- **zstd compression** (level 3) applies to files older than 7 days and runs at most every 6 hours
  (`rollout/src/compression.rs:26`, `370-376`). It is behind the off-by-default
  `local_thread_store_compression` feature, but compressed rollouts already exist on this machine.
  A compressed file is decompressed again when it is appended to (`compression.rs:115`).
- **Size caps:** each MCP and command output stored in the rollout is capped at 64 KiB
  (`rollout/src/policy.rs:43-46`).

**MCP tool approval:**

- Modes are `auto | prompt | writes | approve`, default `auto` (`config/src/mcp_types.rs:22-35`).
- `auto` asks unless the tool is annotated read-only.
- `default_tools_approval_mode = "approve"` under `[mcp_servers.icx]` works on 0.171.0 (verified
  live).

**Compaction:**

- PreCompact and PostCompact accept only the shared output fields, and no hook can change the
  summary.
- Only `compact_prompt` and `experimental_compact_prompt_file` change the summary.

**IPC:**

- `codex app-server` speaks JSON-RPC over stdio, unix or ws.
- Methods such as `thread/inject_items` exist but are experimental (`app-server-protocol`).

### agy 1.5.14 (T-61-004)

agy is written in Go. It is not derived from gemini-cli and has no public source.

**Hooks:**

- **Events:** SessionStart, PreToolUse, PostToolUse, PreInvocation, PostInvocation and Stop. There
  is no compaction event and no user-prompt event (observed).
- **Where hooks are loaded from:** `~/.gemini/config/hooks.json`, `<workspace>/.agents/hooks.json`,
  and plugin `hooks.json`. Hooks placed in `settings.json` are not loaded. The only hook type is
  `command`.
- **PreToolUse** accepts `decision`, `reason`, `overwrite` (a shallow merge into the arguments) and
  `permissionOverrides` (observed).
- **PostToolUse:** the documented reply is `{}`. A non-empty `overwriteResult` may replace the
  tool result text. Whether a `hooks.json` command hook's reply is honoured this way is not
  established. **Unverified.**

**Injected messages (`injectSteps`):**

- `metadata.user_markdown: ""` hides nothing (observed). The terminal UI never reads it, the web UI
  treats an empty value as unset, and the model always receives the text.
- `renderInfo.hidden` is unlikely to hide hook-injected steps in the terminal UI. **Unverified.**
- Treat injected steps as visible to the user.

**Permissions:**

- Rules take the form `action(target)`, evaluated deny, then ask, then allow.
- `mcp(server/tool)` matches on `/`-separated parts, and `*` matches a server or a tool
  (observed). `mcp(icx/*)` pre-approves icx (verified live).
- Hook matchers see MCP tools as `call_mcp_tool` when lazy-loaded and as `mcp_<server>_<tool>`
  when eager.

**Compaction and storage:**

- No hook can influence compaction.
- Hooks receive `transcriptPath`, which points at a JSONL transcript.
- Conversation state is kept in local SQLite stores, which can be read read-only but have an
  undocumented format (observed).

**IPC:** `agy acp` (ACP JSON-RPC over stdio, since 1.5.0) and headless `-p --output-format
json|stream-json` are the supported ways to drive agy.

### Muse Code 1.4.25 (T-61-004, documented interfaces only)

**MCP pre-approval:**

- No settings key pre-approves MCP tools (CLI help, `muse mcp list --json`).
- The documented route is the server declaring a tool read-only, that is,
  `annotations.readOnlyHint` (docs).
- The policy-plane validator accepts only an empty `settings` object, so its vocabulary is
  undocumented.

**Workspace trust:**

- Trust is remembered per workspace root in a user-level file whose format is undocumented. icx
  must never write it.
- A repository that is its own git root needs its own trust grant before Muse loads MCP servers
  there (inferred).

**Hooks:**

- 17 documented events, including `PostToolBatch`, `PreLLMCall`/`PostLLMCall`, `PermissionRequest`
  and PreCompact/PostCompact.
- Output fields are undocumented. In normal use `additionalContext` works, and replacing
  PostToolUse output is refused.

**Output control:** `muse exec --max-tool-output-bytes` limits tool output on the Muse side
(CLI help).

### Hermes v0.23.3, commit `caafe5389ae` (T-61-005)

All paths are relative to the checkout (source). The commit recorded in the adapters doc,
`dc394c6`, is older. The interface changed only additively since then.

**`ContextEngine` interface (`agent/context_engine.py`):**

- **Required:**
  - `name` (`:63-66`);
  - `update_from_response(usage)` (`:87`);
  - `should_compress(prompt_tokens=None)` (`:96`);
  - `compress(messages, current_tokens=None, focus_topic=None, force=False, memory_context="")`
    (`:109-113`).
- **Optional hooks used by the host:**
  - `select_context(request_messages, *, conversation_messages=None, incoming_message=None,
    budget_tokens=0)` (`:137`). It is request-only, runs on every request including retries, and
    has no size cap or timeout.
  - `on_turn_complete` (`:159`).
  - `on_session_start` (`:197`). It is also called after every committed compaction with
    `boundary_reason="compression"` (`conversation_compression.py:2410`).

**Loading and failure:**

- `context.engine` selects the engine.
- Discovery is a text heuristic: the file must contain "ContextEngine".
- An import error falls back to the built-in compressor. Runtime hook failures fail open.

**Hooks icx does not use yet:**

- `transform_terminal_output` (`tools/terminal_tool_result.py:147-158`). The first string it
  returns replaces the shell output, and the output limit is applied afterwards.
- It is available only to a general plugin listed in `plugins.enabled`, not to a context engine.

**Defects in icx's current Hermes plugin** (installed by hand; no shim ships yet, ICX-AUD-006):

- `kind: context_engine` is not a valid manifest kind (`hermes_cli/plugins_manifest.py:27`).
  Hermes logs "unknown kind" and treats the plugin as standalone.
- The plugin receives no `compression:` config.
- SessionStart is called synchronously, at init and after every compaction.
- `on_turn_complete` re-sends the whole transcript every turn and starts an unbounded thread each
  time.

## 3. Go/no-go decisions (T-61-007)

**Rule:**

- Every go item that changes what icx sends to an agent is off by default and version-gated in
  its follow-up ticket.
- `readOnlyHint` is the exception: it ships only if the live test in T-64-007 shows it works.
- Go items that are tests, budgets or documentation corrections need no flag.

**Before deciding,** each candidate was re-checked against the latest installed release of its
agent on 2026-11-22.

### Claude Code

| Candidate | Decision | Reason | Follow-up |
| --- | --- | --- | --- |
| `readOnlyHint` on a read-only icx tool | Go | The only route out of the plan-mode hold. Allow rules are not consulted there. Also documented for Muse and honoured by Codex `auto` | T-64-007, T-64-008 |
| Per-tool shape-exact `updatedToolOutput` using the fields above | Go | Required fields are known for seven tools. WebSearch and Agent stay ingest-only until resolved | T-64-001 |
| Keep both fields for MCP tools, and never emit an empty `updatedMCPToolOutput` | Go (test only) | Current icx behaviour is correct; pin it with a regression test | T-64-001 |
| Budget `additionalContext` at 8,000 characters and 200 lines, with exact `hookEventName` | Go | The cap on icx's hook path is 8,000/200, not the 10,000 first assumed; a mismatched name drops the context | T-64-006 |
| `PostToolBatch` batch-level ingestion and context | Go | Payload confirmed | T-64-004 |
| Model-switch events seal the cache epoch, using the cache-warmth fields | Go | Payload confirmed | T-64-005 |
| PreCompact instruction to keep `#icx:` handles, plus re-injection at the following SessionStart | Go | PreCompact stdout reaches the summarizer; PostCompact output does not reach the model | T-73-001, T-73-002 |
| IDE lock files, internal socket transport | No-go | Undocumented | None |

### Codex CLI

| Candidate | Decision | Reason | Follow-up |
| --- | --- | --- | --- |
| `continue: false` with `reason` as stub substitution | Go | Source-confirmed whole-result substitution that is not marked failed. It relies on a side effect, so pin it with a version test | T-62-001, T-62-002 |
| `decision: "block"` as substitution | No-go | The model sees a failed tool call, which changes its behaviour | T-62-001 records the finding |
| Never emit `updatedToolOutput` or `updatedMCPToolOutput` | Go (test only) | Either field breaks the reply; icx already omits both, and tests pin it | T-62-005 |
| Respect the 2,500-token `additionalContextLimit` | Go | Over the limit the context is spilled to a file and only a preview reaches the model | T-62-006 |
| Rollout ingestion handling `.zst` and the 64 KiB per-output cap | Go | Format and compression confirmed in source | T-62-003, T-62-004 |
| `mcp_tool` hook handler instead of a hook process | Deferred | Saves a process per hook but changes the trust boundary; revisit with Phase 8 timing data | Phase 6 close re-scan |
| app-server `thread/inject_items` | No-go | Experimental API | None |
| PreCompact used to seal an epoch and snapshot, without injection | Go | No hook can change the summary | T-73-002 |

### agy

| Candidate | Decision | Reason | Follow-up |
| --- | --- | --- | --- |
| PostToolUse `overwriteResult` stub replacement | Go, gated on live verification | May replace the result; not yet shown for command hooks. Undocumented, so off by default and version-gated | T-65-007 |
| Treat `injectSteps` as visible; correct the "invisible" claim | Go | `user_markdown: ""` hides nothing; `renderInfo.hidden` is unlikely to help | T-65-003 |
| Matcher covering `call_mcp_tool` and `mcp_icx_*`, so the pager exemption holds | Go | Both tool-name forms reach hooks | T-65-008 |
| `mcp(icx/*)` pre-approval | Go | Verified live | T-65-005 |
| PreToolUse `overwrite` to rewrite commands before execution | Deferred | Belongs to the pre-execution rewriter | T-10-1-001 |
| Read `transcriptPath` JSONL for recall | Go (existing) | Hooks supply the path; icx already tails it | None |
| Read the SQLite conversation stores | No-go | Undocumented proprietary format | None |
| Inject checkpoint steps into compaction | No-go | High risk; how compaction filters injected checkpoints is unproven | None |
| `agy acp` adapter | Deferred | Supported and stable, but a new adapter class; revisit at Phase 6 close | Phase 6 close re-scan |

### Muse Code

| Candidate | Decision | Reason | Follow-up |
| --- | --- | --- | --- |
| `readOnlyHint` for on-request auto-approval | Go | The documented route, and no settings key exists | T-64-007, T-65-005 |
| Installer prints trust guidance and never writes the trust file | Go | Undocumented format | T-65-004 |
| Document `--max-tool-output-bytes` as Muse's output reducer | Go | Muse refuses hook output replacement | T-65-004 |
| `PostToolBatch` batch-level context | Deferred | Payload undocumented | Re-check when documented |
| PostToolUse output replacement | No-go | Refused | None |

### Hermes

| Candidate | Decision | Reason | Follow-up |
| --- | --- | --- | --- |
| Fix the plugin's manifest kind, delta ingestion, asynchronous SessionStart and forwarded `compression:` config, and ship it | Go | Real defects in the current plugin | T-65-002 |
| Resume packet at the compression boundary, via `on_session_start` then `select_context` | Go | Request-only and fails open | T-73-003 |
| General plugin using `transform_terminal_output` to stub shell output | Go, off by default | The only output-replacement seam; needs `plugins.enabled` | T-65-009 |
| `post_compaction` hook feeding telemetry | Deferred | Belongs with observability | T-83-001 |
| `memory_context` via a MemoryProvider | No-go | The provider slot is exclusive and already in use | None |
| icx pager exposed through `get_tool_schemas` | Deferred | MCP already provides it; the name would collide | None |

## 4. Versions and evidence

| Agent | Version inspected | Evidence |
| --- | --- | --- |
| Claude Code | 2.1.330 (native install) | Installed version; public CHANGELOG, docs and issues #86753, #67442, #90058 |
| Codex CLI | 0.171.0 (`rust-v0.171.0`, commit `ccf9be8cc458`) | Source at the tag; installed binary checked against it |
| agy | 1.5.14 | Installed version; public docs at antigravity.google |
| Muse Code | 1.4.25 (`1.4.25-R9361.1`) | CLI help, config validation, files on disk, docs at dev.meta.ai |
| Hermes | v0.23.3 (`caafe5389ae`, 2026-11-19) | Source at the commit; installed bytecode checked against it |

The evidence and working notes behind these conclusions are kept privately, per ADR-0008.
