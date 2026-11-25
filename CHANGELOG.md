# Changelog

All notable changes to **Infinity-Context (`icx`)** are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html), and is distributed under the **GNU General Public License v3.0 or later (`GPL-3.0-or-later`)** ([`LICENSE`](LICENSE)).

## [Unreleased]

### Fixed

- `icx install`: the Claude Code matcher note judges all icx `PostToolUse` groups together, so a
  setup split into a Bash group and a Read/Grep/Glob/Agent group gets no note, and it gives the
  "after `^(`" advice only for a plain `^(` group (never `^(?:` or `^(?!`, where it would yield
  an invalid regex) and only when that group spans the whole matcher. An empty `{}` or `[]`
  file (spaces inside included) gets its new content laid out over several lines (#95, #96). A
  group with no matcher counts as matching every tool; a matcher that is not a string is
  ignored.
- Lexical index (T-71-001, off by default): with `ICX_RECALL_RANKING=rrf`, the workspace store
  keeps two contentless SQLite FTS5 tables, BM25 over words (compound identifiers also indexed by
  their parts) and trigrams for substrings, confirmed against the stored text. It is unsealed
  derived state, so it can only name artifacts the sealed CAS holds; it is backfilled 2,000
  artifacts per save; the schema version is unchanged, so older builds still open the store.
  Turning it off also stops its queries, and tables of another version (or only one of the pair)
  are rebuilt rather than read.
  Measured on a real 54 MB store with the production code: 10.2 MB of index (rows are numbered
  compactly; random 64-bit rowids took 38.5 MB), queries under 1.6 ms (`docs/numbers.md`).
- Recall ranking fusion (T-71-002, opt-in): with `ICX_RECALL_RANKING=rrf`, the injected recall
  line comes from the HDC hits fused with BM25 matches by reciprocal rank (k = 60, ties by
  handle). Off by default; with it off, recall output is unchanged.
- `icx eval-recall` (T-72-002): scores recall rankers (`legacy`, `bm25`, `rrf`, `recent`) by
  recall@1/5/10 and MRR@10 on a seeded synthetic set or a JSON Lines query set over a workspace,
  read only, ranking only artifacts that existed when each query was asked.
- `scripts/build-recall-eval-set.py` (T-72-001): builds a private evaluation set from Claude
  Code transcripts (agent notes or user prompts paired with the outputs then dereferenced),
  written only under the gitignored `eval/private/`.
- ADR-0010: the default recall ranking stays `legacy`. Real-session answers were almost always
  just produced (newest-first finds 83–88% at rank 1), so the data cannot show a long-range gain;
  local embeddings (T-72-003) are therefore not built.
- Recall reports a shared fractal suffix only for a history that really repeats. Each trajectory
  step keeps a 16-bit corner of its state, so distinct events with equal corners were reported as
  a shared history (0.33% of similar events in a 20,000-event measurement, ICX-AUD-052); each
  step is now confirmed against the stored states (T-71-004).

## [1.4.1] - 2026-11-24

Fixes found while installing v1.4.0 on a real machine, and the remaining suggestions from the
v1.4.0 release review. Release notes: [`docs/releases/v1.4.1/README.md`](docs/releases/v1.4.1/README.md).

### Fixed

- `icx install` keeps each JSON file's own formatting. It used to re-serialise a whole file with
  two-space indentation, so a tab-indented or hand-aligned config such as OpenCode's
  `opencode.json` changed on every line for a one-entry addition. Now every existing byte is kept
  and only the new members and elements are inserted, indented like their siblings, with the
  file's colon spacing and line endings (CRLF stays CRLF); a compact file stays compact, trailing
  newline or not. A change that is not a pure addition falls back to re-serialising with the
  file's own indent unit.
- `icx install` warns when an existing icx `PostToolUse` hook for Claude Code has a matcher from
  before v1.4.0 that names none of Read, Grep, Glob or Agent, so file reads and subagent reports
  were never shrunk; it prints the matcher to use (match-all matchers such as `.*` or `^.*$` get
  no note). It still never edits an existing hook.
- Command reading (own-command and existing-hook detection, Rust and Python): an unquoted newline
  ends a command, so `cd /repo` followed by `icx deref ...` on the next line is recognised as
  icx's own command; a backslash before a newline (LF or CRLF) joins the lines. `shell_words` and the command
  splitter now share one lexer.
- `icx install` never writes a hook path containing `..` (from `../bin/icx install` or a `PATH`
  entry with `..`); it uses the binary's canonical path, so removing the directory the install
  ran from cannot break the hooks.
- A temporary or backup file is removed after a failure only when icx created it; a failed create
  never removes anything.

### Documentation

- The v1.4.0 Hermes plugin test count was stale: 27 tests, not 24.

### Tests

- A daemon lock test waits 5 s instead of 600 ms for the released lock; under coverage
  instrumentation it flaked on a slow but correct release.

## [1.4.0] - 2026-11-23

Phase 6, agent reach and output shrinking: Gemini CLI, Cursor and OpenCode adapters, the Hermes
plugins, `icx install`, the read-only `icx` MCP tool, live contract fixtures and a generated
capability matrix. Release notes: [`docs/releases/v1.4.0/README.md`](docs/releases/v1.4.0/README.md).

### Fixed (release review)

- `icx install` refuses to overwrite a file that changed after it was read (a running agent's
  edit), removes a partly written backup, writes hooks that run the path `icx` was started by (a
  versioned symlink such as a Homebrew or Nix link survives upgrades; `argv[0]`, checked against
  the running binary), and no longer lists a named agent as skipped.
- Command reading (own-command and existing-hook detection), one scanner in Rust and Python:
  backslash escapes inside double quotes, separators and `#` inside quotes or words are
  arguments (`grep "|" x`, `FOO="a; b"`, `#icx:...`), separators are found by the scanner rather
  than marked in the text, and wrapper scripts (`-c`) are read at most 8 levels deep.
- Hermes engine: hook side effects never raise into Hermes; tail-slot wait events are held
  weakly and the worker drops finished jobs, so finished subagent engines leave nothing behind;
  a failed fetch start never makes a request wait. `icx-terminal`: output that cannot be stored
  byte for byte (a lone surrogate) is not stubbed and stays raw.
- `sanitize-hook-fixtures.py` fails when no captures are found, and handles a `HOME` of `/`.

### Added

- **Generated agent capability matrix** (`docs/capability-matrix.md`, T-65-006): per agent,
  whether icx ingests, shrinks (by default or opt-in) and injects context, whether compaction and
  session end seal the epoch, how the agent's side was verified, and the pre-approval rule. A
  test builds it by running each adapter and fails when the committed file is out of date.
- **Hermes plugins ship** in `integrations/hermes/` (T-65-002, T-65-009; ICX-AUD-006 fixed),
  written against Hermes v0.23.3 and pinned to `>=0.23.3,<0.24`. `icx install --agent hermes`
  copies them; Hermes's `config.yaml` is not edited.
  - `icx`, the context engine: Hermes's built-in compressor built with your `compression:`
    settings, which Hermes does not pass to a plugin engine. It ingests only the messages added
    since the last turn on one bounded background worker, checkpoints history before each
    compaction, and fetches the tail slot in the background, so Hermes never waits for icx.
    Fixed from the hand-installed plugin: the invalid `kind: context_engine`, whole-transcript
    re-sends with one thread per turn, a blocking session start, and ignored `compression:`
    settings.
  - `icx-terminal`, off until listed in `plugins.enabled`: stubs large successful shell results
    through Hermes's `transform_terminal_output` hook (new `icx hook --agent hermes` event of the
    same name). A failed command, or one with an unknown exit status, keeps its output, and icx's
    own commands (`icx deref` through the shell) and output already in an icx frame are never
    stubbed again. The engine escapes `</icx-context` inside the tail slot, so untrusted text
    cannot open or close its frame. A session's first request waits up to
    `ICX_HERMES_TAIL_WAIT` seconds (default 2) for the tail slot, which is then fixed for the
    session so the prompt-cache prefix stays stable; session start itself never waits.
  - Verified by stand-in tests with the real icx binary, run in CI with the OpenCode plugin
    tests. They have not run in a live Hermes session; that check is deferred to v1.5.0 (#89).
- **`icx install --agent <name|all>`** (T-65-004, T-65-005) registers icx's hooks and MCP server
  with Claude Code, Codex, agy, Muse, Gemini CLI, Cursor, OpenCode and Hermes.
  - Every change is printed as a unified diff first. `--dry-run` stops there; otherwise it asks
    on a terminal, `--yes` skips the question, and with no terminal it refuses to write.
  - Each edited file is backed up to `<file>.icx-backup-<unix time>` and replaced atomically.
    Backups and temporary files are created new with the original's mode (`0600` for new files),
    and a symlinked config is written at its target.
    A file icx cannot read as text (not UTF-8, unreadable) is left alone with a note, never
    taken for a missing one. The hook command quotes the icx path when it needs quoting, and an
    existing hook with an `env` or `KEY=value` prefix is recognised. A dangling dotfiles link is
    written at its target; backups never collide; a path already at the temporary name is
    refused and left alone.
  - Edits are additive and idempotent: an existing icx hook (`icx` or the `icx-hook` wrapper) is
    never duplicated, a second run changes nothing, and a file with nothing to add is left byte
    for byte. JSON key order and unknown keys are kept; Codex's `config.toml` keeps its comments.
    A file that does not parse is never rewritten.
  - `--agent all` covers only the agents installed here.
  - `--approve-tools` pre-approves only the read-only `icx` tool, where the agent supports it
    (deployment guide section 2.11 lists each rule and how to remove it).
- **`icx hook --agent agy` finds the project itself** from the payload's `workspacePaths[0]`, so
  agy needs no wrapper script that changes directory first.
- **`icx codex-ingest`** (T-62-003, T-62-004) reads Codex rollouts into the workspace.
  - It reads both plain `rollout-*.jsonl` files and the zstd-compressed `.jsonl.zst` files Codex
    writes for rollouts older than 7 days.
  - By default it reads only sessions that ran in the current project; `--all-projects`,
    `--project` and `--sessions` change that.
  - Each rollout's position is saved in the workspace (`codex-rollouts.json`), so a later run
    ingests only new records, including after Codex recompresses or decompresses a file.
  - Compressed files are decoded by the pure-Rust `ruzstd` decoder.
    - A frame's content checksum is compared when the frame carries one. Codex's frames don't, so
      corruption that still decodes cannot be detected in them.
    - Bytes after the frame are refused.
    - A file over 1 GiB decompressed, a file that fails to decode, and a symlink are each skipped
      and named, and nothing from them is ingested.
  - **Tool outputs are left to the Codex hook by default.** The hook captures each tool output
    when the tool runs, and its copy never matches the rollout's, by bytes or by call id under
    Code Mode (measured: about 7 of 1,583 matched on this project). Ingesting both would index
    every output twice.
    - The command ingests messages, reasoning and turn context, and reports the tool outputs it
      left to the hook.
    - `--include-tool-outputs` backfills them, for sessions recorded before the hook was
      installed.
  - A record whose redacted output the store already holds byte for byte is skipped and counted
    as "already stored", which catches outputs repeated inside the rollouts.
- **Gemini CLI adapter** (`icx hook --agent gemini-cli`, T-63-001), built from the documented
  hooks contract and **not verified live**.
  - `SessionStart` and `BeforeAgent` add the tail slot to the user turn; `AfterTool` stores
    results; `PreCompress` and `SessionEnd` seal the epoch.
  - Stub replacement through `decision: "deny"` is opt-in (`ICX_GEMINI_STUB=1`).
  - icx's own MCP tool is recognised under Gemini CLI's name for it, `mcp_icx_icx`, and never
    ingested.
- **Cursor adapter** (`icx hook --agent cursor`, T-63-002).
  - `postToolUse` and `postToolUseFailure` store results; `preCompact` and `sessionEnd` seal.
  - Nothing is injected at `sessionStart`, because Cursor puts that text in the system context,
    and icx context is only ever delivered as user-turn data.
  - MCP result replacement through `updated_mcp_tool_output` is opt-in (`ICX_CURSOR_STUB=1`) and
    not yet verified live.
- **OpenCode plugin** (`integrations/opencode/icx.ts` and `icx hook --agent opencode`, T-63-003)
  for the V2 plugin API, verified on OpenCode v2.2.7.
  - It appends the legend and tail slot to the user's message, stores tool results, and seals the
    epoch at compaction. It adds no system text.
  - Replacing large results with a stub works on v2.2.7 and is opt-in (`ICX_OPENCODE_STUB=1`).
  - It fails open when icx is missing, fails or times out.
- **Stub modes follow the hook caller.** `ICX_GEMINI_STUB`, `ICX_CURSOR_STUB` and
  `ICX_OPENCODE_STUB` are read from the hook's own environment. A hook that asks for stubs runs
  in-process, and through the daemon these adapters capture only, so a daemon started with
  other settings cannot override the caller's.
- **Mixed results are kept whole.** A Gemini CLI, Cursor or OpenCode result whose parts are not
  all text (an image, a resource) is stored as its full JSON, so nothing is dropped.
- **`icx doctor` checks the new agents** (T-63-004): for Gemini CLI, Cursor and OpenCode it
  reports presence, version and whether an icx hook or plugin is registered, in the user config
  or the current project's.
  - Only a command whose program is `icx` counts, so another tool's hooks for the same agent are
    not mistaken for icx's.
  - The OpenCode plugin is recognised in any quote style.

### Changed

- **agy: icx no longer ingests its own MCP replies when agy loads the tool lazily** (T-65-008).
  agy then names the call `call_mcp_tool` with `ServerName`/`ToolName` (recorded live on 1.5.14),
  which the own-tool exemption did not recognise.
- **agy: the docs no longer claim injected steps are invisible** (T-65-003). On 1.5.14
  `metadata.user_markdown: ""` hides nothing; the steps are visible to the user.
- **agy stays ingest-only** (T-65-007): a PostToolUse `overwriteResult` did not replace the result
  the model saw on 1.5.14.
- **Live contract fixtures for agy 1.5.14** (T-65-001) under
  `crates/icx-adapters/tests/fixtures/agy/1.5.14/`, written by the new
  `scripts/sanitize-hook-fixtures.py`, which replaces home and project paths, UUIDs and e-mail
  addresses and refuses output in which a real path survives.
- **The MCP server offers two tools** (T-64-008, ADR-0009).
  - **`icx`** is now read-only: `deref`, `slice`, `grep`, `recall`, `expand`, `status`, and
    `xagent` as a poll. It carries `readOnlyHint`, so Claude Code's plan mode runs it without a
    prompt (measured on 2.1.330: an allow rule alone does not lift the plan-mode hold).
  - **`icx_write`** takes `fold`, and `xagent` with a message.
  - **Breaking:** `fold` and broadcasts sent to `icx` are refused with an error naming
    `icx_write`, so the read-only hint is never false. Update instructions that tell agents to
    fold with the `icx` tool.
  - Every hook adapter treats both tools as icx's own and never ingests their output.
- **The OpenCode plugin sends the legend again after a compaction**, since the summary may drop
  the prompt that carried it.
- **Claude Code `CwdChanged` moves the agent to the new directory's workspace** (T-64-003,
  verified live on 2.1.330). The workspace is resolved from `new_cwd`, so the agent's icx MCP
  server follows a `cd` into another repository or worktree on its next call (#34). Register the
  hook to use it (deployment guide §2.2).
- **Claude Code Read, Grep, Glob and Code Mode results are stored as their text** (T-64-001,
  verified live on 2.1.330).
  - Before, Read, Grep and Glob results were stored as one line of JSON, so a stub said
    `lines=1..1` and `slice` could not address the tool's lines.
  - A Code Mode script result (`REPL`) was taken for a Bash result: only its usually empty
    `stdout` was stored and a large `result` was never shrunk. Its `result` now gets the stub,
    and the script itself is left as is.
  - Live, the model saw the stub for Bash, Read, Grep, Glob, WebSearch, an Agent report, a Bash
    call inside a Code Mode script, and the script result.
  - An Agent report's text blocks are replaced by one stub block; before, only the largest
    block was, and the rest of the report still reached the model. Image and resource blocks
    are kept and not stored. WebFetch and Agent results are stored as their text.
  - Binary data is never replaced: `image`, `audio` and Anthropic `base64` blocks and `blob`
    resources are not entered (MCP's `mimeType` is optional, so the block type decides), and a
    `base64` value, or `data`/`blob` beside a MIME type or `uri`, is skipped, and a result with no text value larger than the stub (an image `Read`, an
    image-only report, a screenshot with a caption) reaches the model unchanged, though it is
    still stored. `updatedMCPToolOutput`, a string that replaces the whole MCP result, is sent
    only when every content block is text and nothing is binary, so images, audio, resources and
    resource links are never dropped; structured JSON with its own `type` fields still gets it.
    The two replacements are decided independently.
  - These shapes apply only when the call names that built-in tool (`Read`, `Grep`, `Glob`,
    `WebFetch`, `Agent` or `Task`, `REPL`), and the stub replaces exactly the field that was
    stored. An MCP result with the same keys is stored whole, as any other JSON result.
  - `scripts/live-claude-replacement-check.sh` repeats the check after a Claude Code update
    (billed, run by hand). It exits 1 if the model saw raw output, and 2 if `claude` itself
    failed (its stderr is shown).
- **Compaction and session end always seal the cache epoch** in Claude Code, Codex, Muse,
  Gemini CLI, Cursor and OpenCode, and Hermes's `on_pre_compress` seals too. agy fires neither
  event: its `Stop` seals only on `max_steps_exceeded` or an error (the adapter's `SessionEnd`
  handler is there for callers that send one).
  - Before, these events sealed only an epoch that had reached its token threshold, although the
    docs said they always sealed it. Compaction rewrites the conversation, so the old cached
    prefix is gone whatever the epoch's size.
  - A repeated event with nothing new adds no empty epoch. agy's `Stop` keeps the conditional
    check, except for the error reasons that already forced it.
- **Claude Code `PostModelSwitch` seals the cache epoch** (T-64-005), since the new model starts
  a cold prompt cache. `PreModelSwitch` is not used: a failing or slow hook there blocks the
  switch.
- **Stub settings reach the daemon** (review of #87). A hook whose caller sets
  `ICX_GEMINI_STUB`, `ICX_CURSOR_STUB` or `ICX_OPENCODE_STUB` sends the setting with its daemon
  request (protocol version 2), so it keeps the daemon's warm store instead of loading the store
  itself.
  - A daemon from an earlier release refuses version 2, and the hook then runs in-process.
  - A version-1 request never asks for stubs.
  - The OpenCode plugin's default timeout is now 10 s (`ICX_PLUGIN_TIMEOUT_MS`). Measured for
    one stub-mode hook: about 2 s through a warm daemon and 4.5 s in-process on an 825 MB store,
    against 0.07 s and 0.15 s on a 28 MB store.
- **Review fixes from #87, round 2:**
  - Cursor no longer treats `"mcp_server_name": null` as an MCP tool, and matches `mcp_`
    case-insensitively.
  - The OpenCode plugin sends the legend once per session, and sends `session.end` when it sees
    `session.deleted`.
  - `icx doctor` reads an icx hook command in any flag order and matches the agent name exactly.
  - `icx doctor` no longer stalls on a binary that prints more than a pipe buffer.
  - The Codex oversized-record check tolerates spacing and reads direct records.
  - Gemini CLI ignores an empty prompt for recall.
- **Claude Code `additionalContext` is bounded to 8,000 characters and 200 lines** (T-64-006),
  the inline limit on the settings-file hook path, above which Claude Code spills the text to a
  file.
  - Characters are counted as JavaScript string length.
  - Whole lowest-ranked lines are dropped, with a `⟪ICX:TRUNCATED│dropped=<n>⟫` line, so no
    `#icx:` handle is cut.
- **Codex `additionalContext` is bounded to 10,000 bytes** (T-62-006). That is Codex's default
  `additionalContextLimit` of 2,500 tokens, above which Codex spills the text to a file. The tail
  slot already fits; a larger value would be replaced whole by a diagnostic, never cut.

### Security

- **Reverse-engineering output stays private** (ADR-0008).
  - Raw REA and Ghidra output, evidence bundles, analysis projects and string dumps of agent
    builds are gitignored, and so is `ref-docs/agent-internals/private/`.
  - The 2026-11-16 binary survey is now a public summary that keeps only its interoperability
    conclusions. The full ledger is kept on disk only.

### Build

- **CI runs the agent plugin tests:** the OpenCode plugin (`bun test`, Bun from the SHA-pinned
  `oven-sh/setup-bun` v2.2.0) and the Hermes stand-in tests (`python3 -m unittest`), both against
  the debug `icx` binary. Changes under `integrations/` now trigger the Rust jobs, because
  `icx-cli` embeds those files.
- **New dependencies** for `icx install`: `toml_edit` 0.25.20 and `similar` 3.2.1 (`cargo deny`
  clean).
- **`.gitignore`** covers `icx install` backups and temporary files (`*.icx-backup-*`,
  `.*.icx-tmp-*`) and local Bun artifacts (`node_modules/`, `bun.lock`, `bun.lockb`).
- **CI runs only what a pull request needs.** Documentation-only pull requests skip the Rust
  build, tests, benchmark, coverage and `cargo deny`, while the secret scan and REUSE still run.
  - Pushes to `main` and manual runs keep the full suite, which the release gate requires.
  - A weekly scheduled run re-checks advisories.
  - Rust caches are written only from `main`.

### Documentation

- **README:** Getting Started and CLI Usage show `icx install`; the interception matrix has rows
  for Gemini CLI, Cursor and OpenCode, the shipped Hermes plugins, and the two MCP tools; the
  repository map lists `integrations/`.
- **Specifications:** `docs/interfaces.md` (the `install` command and the new environment
  variables), `docs/testing-strategy.md` (live fixtures, plugin and installer tests),
  `docs/security-and-threat-model.md` (§4.6, agent configuration written by `icx install`), the
  adapters doc (which agents are pinned by live fixtures; the Hermes plugins), and the
  deployment guide (§2.5 Hermes rewritten, §2.11 automated install).
- **Deferred to v1.5.0 (#89):** live runs for Hermes (the gateway stays stopped), Gemini CLI (not
  installed) and Cursor tool calls (blocked by its usage limit). Their v1.4.0 support follows the
  documented contracts, and the Phase 6 exit criteria say so.
- **Known issue (#90):** the redactor blanks non-secret values after key names containing
  `token`, `key` or `session` (for example `current_tokens: int = None`), to the end of the line.
  It over-redacts and never leaks; the Phase 7 structural redactor fixes it.
- **Roadmap for v1.4.0 through v2.0.0.** `to-dos/ROADMAP.md` is rewritten from v1.3.2. Phases 6 to
  12 are new: 56 files, 223 tickets, one minor release per phase and v2.0.0 last.
  - Each sprint opens with a research gate, and each ticket carries a source and a Research line,
    so research continues while the work is done.
  - Phase 12 builds the opt-in `icx-shm` bus in full, which resolves issue #32 and ADR-0007.
  - The v1.8.0 benchmark gate blocks only on a statistically significant regression across
    repeated paired runs, and has a cost budget per run.
- **Below-hook findings** (Phase 6 Sprint 1, issue #40): a new dated page,
  `ref-docs/agent-internals/below-hook-findings-2026-11-22.md`. It covers what each supported agent
  accepts beyond its documented hooks, and a go/no-go decision per candidate integration.
  Findings that change the plan:
  - **Codex:** `decision:"block"` reaches the model as a failed call. Stub substitution moves to
    `continue:false` with a reason (T-62-001, T-62-002).
  - **Claude Code:** plan mode holds MCP tools before it reads allow rules, so `readOnlyHint` is
    the only way to stop the icx prompts there (T-64-007). The `additionalContext` cap on icx's
    hook path is 8,000 characters and 200 lines (T-64-006).
  - **agy:** `user_markdown: ""` does not hide injected steps (T-65-003).
  - **New tickets:** T-62-006, T-65-007, T-65-008 and T-65-009.
- **Gemini CLI, Cursor and OpenCode setup** (T-63-004): deployment-guide §2.7 to §2.9 cover
  install, config, verification and limits for each, and the adapters spec gains §8 to §10. The
  generic MCP section moves to §2.10. OpenCode v2.2.7 nests MCP servers under `mcp.servers`.

## [1.3.2] - 2026-11-22

Fixes #33, #34, #36 and #37, plus problems found by mining four days of agent sessions for icx
errors. Release notes: [`docs/releases/v1.3.2/`](docs/releases/v1.3.2/README.md).

### Highlights

Stubs that could not be dereferenced are fixed at both causes: an agent working in a git
worktree now shares its project's workspace, and the `icx` MCP server follows the workspace its
own agent's hooks use, so a session that moved into a nested project reads the store its stubs
were written to. MCP results (Context7 and other docs servers) are stored as text with real
lines. Small outputs stay inline, recall noise is filtered, `slice` and `grep` are friendlier,
and several redactor over-redactions seen in the field are gone.

### Fixed

- **Worktree stubs not found (#37).** `project_root` resolves a linked git worktree to its main
  repository, so a subagent in `.claude/worktrees/agent-*` stores into the project's workspace.
  Accepted only when git's own back-link (`<main>/.git/worktrees/<name>/gitdir`) points at the
  worktree, so a planted `.git` file cannot redirect a project; submodules are unchanged.
- **MCP server on the wrong workspace (#34).** `icx hook` records the workspace it used for its
  agent (the nearest ancestor process that is not a shell or launcher) in
  `$XDG_RUNTIME_DIR/icx/agents/` (`0700`, files `0600`, named by PID and process start time, so a
  reused PID never matches; records of exited agents are pruned). `icx mcp-serve` resolves its
  workspace per request from its own agent's record; only a private record naming a workspace
  under `<state>/workspaces/` is accepted, and trust checks still apply. An explicit `--dir`/`ICX_DIR` pins the workspace, and
  `ICX_MCP_FOLLOW_HOOKS=0` disables following. A "not found" error now names the workspace it
  searched.
- **MCP results stored as one JSON line (#37).** A result made of `text` content blocks is stored
  as its text, so `slice` and line-level `grep` address real lines; results with image or
  resource blocks keep the JSON form.
- **CLI (#33).** Repeated value-taking flags (`daemon`, `deref`, `proxy`, `proxy-transform`,
  `encode-ahtl`) are rejected instead of silently keeping the last value; `deref` and
  `encode-ahtl` find their handle or file wherever it sits, so a flag is never taken as one;
  `--dir` uses one value rule everywhere.
- **Redactor (#33).** Dotted or spaced counter keys (`max.tokens`) are not secrets; a multi-line
  quoted value leaves no unmatched closing quote; a YAML block header whose key an earlier marker
  swallowed is not glued onto it; CRLF lines inside JSON strings get the same digest and key-context
  checks as LF lines. The code paths the v1.3.1 first-quote rule made unreachable are removed
  (163 lines).
- **Redactor over-redaction seen in the field.** A vendor prefix inside a word is not a token
  start (`autodesk-...` matched `sk-`); a `-`-joined name made of words (project-directory slugs,
  model names such as `Llama-4-Maverick-17B-Instruct`) is not a high-entropy secret, and a token
  count labelling a list (`512 tokens: ...`) is not a `tokens` key. Only the generic `sk-` prefix
  gets the in-word exemption, and only after an ordinary word (`disk-`, `autodesk-`); a slug with
  a part of 20+ characters or a random mixed-case part stays subject to the entropy gate. Real
  tokens are still caught.
- **`slice`** (and `icx deref --lines`) clamps an end past the last line instead of failing, for
  stored outputs and fold/epoch nodes alike (a start past the end is still an error). **`grep`** bounds each matched line to 400 characters and a reply to 32 KiB (a minified
  line produced a 238,000-character reply that the client rejected).

### Changed

- Hooks leave tool outputs under 2 KiB inline (`ICX_STUB_MIN_BYTES`, 256 B..1 MiB). In the field,
  most stubs replaced ~1 KB outputs with a ~200-byte stub plus a deref round trip. The library
  default (used by the benchmarks and simulations) is unchanged.
- Recall entries weak on every signal (HDC similarity and combined score under 0.10, no shared
  fractal suffix) are not injected; exact-identifier hits are kept.

- Version-numbered file names replaced with descriptive ones, so names stay valid for every
  release: the 42 `*_v1NN.rs` / `redact_issue24.rs` integration tests (for example
  `redact_v120.rs` -> `redact_regression_fixtures.rs`, `redact_v110.rs` ->
  `redact_recall_and_losslessness.rs`, `window_eviction_v121.rs` ->
  `window_eviction_accounting.rs`; the version a test was added in stays in its doc comment), the
  `redact-*-v120.bin` fuzz fixtures, the property-test regression file, and
  `to-dos/v1.2.0-remediation.md` -> `to-dos/codex-audit-remediation.md`. ADRs, frozen audits,
  release notes, the AHTL v1 format and `ref-docs/` keep their names. The gitleaks path allowlist
  names the same two fixture files under both names, since history keeps the old paths.
- `.gitignore`: `archive/` folders (local archives, never committed) and the docs-site build
  output (`.site-src/`, `site/`).

### Build

- Docs site (#36): a directory link resolves to its index page; the MathJax download retries with
  backoff and is cached in CI (`actions/cache`, pip cache); the navigation injection works on CRLF
  checkouts; `build-and-tooling.md` shows a virtual-environment setup.

### Documentation

- Documentation site built with Zensical and published to GitHub Pages
  (<https://doublegate.github.io/Infinity-Context/>): `zensical.toml`,
  `scripts/build_docs_site.py` (stages `docs/` and the root pages, points links that leave the
  site at GitHub, builds the navigation) and `.github/workflows/docs.yml` (strict build on pull
  requests, deploy from `main`; Zensical pinned, actions pinned by SHA).
- ADR-0007 (proposed): opt-in memory-mapped seqlock bus (`icx-shm`), tracked in #32.
- Markdown lint: blank-line fixes across non-frozen documents.

### Tests

- Regression tests for multi-word and multi-line values under sensitive JSON keys.

## [1.3.1] - 2026-11-22

Fixes #28, #29 and #30. Release notes:
[`docs/releases/v1.3.1/`](docs/releases/v1.3.1/README.md).

### Highlights

v1.3.1 hides a value after a sensitive key from its first quote to the end of the line, so secrets
inside code (`env::var("K").unwrap_or("...")`, `x or "..."`, template and prefixed literals) no
longer stay visible in icx's stored copy (#29). A call head such as `env::var(` stays visible;
later text on the line is hidden with the value. Complete JSON lines, such as minified API output,
are parsed instead, so only the sensitive values are hidden. It also rejects CLI
flags with a missing or flag-shaped value (#28), and aligns hook fail-open replies with each
adapter's own no-op (#30). Details: [v1.3.1 release notes](docs/releases/v1.3.1/README.md).

### Changed

- Redaction (#29): after a sensitive key, a value that holds any quote is hidden from its first
  quote to the end of the line, as one marker. A call head right before that quote stays visible
  (`password = env::var([REDACTED:ENV_SECRET]`); any other value is hidden from its start, since a
  bare head could itself be the secret (`hunter2 "note"`). There is deliberately no quote
  pairing, no field boundary and no key detection: each was bypassed in review (a stray quote
  shifted the pairing; redacted key names hid later sensitive keys). So later text on the same
  line is hidden with the value, JSON keys and later CLI flags included. Pinned by a corpus of
  every reported shape and a property test over random code shapes.
- Redaction (#29): after a sensitive key, no exemption (null default, type annotation, code
  expression, token counter) applies when a quote follows on the line.
- Redaction (#29): a line that is one complete JSON object or array, such as minified API
  output, is redacted structurally by a strict RFC 8259 scanner. The value of a sensitive key, of
  any type, becomes `"[REDACTED:CONFIG_SECRET]"`. Every other string is redacted on its own, with
  its key as context. Formatting and key order are kept, and the edited line must still parse.
  Anything that is not valid JSON, or nests deeper than 128 levels, keeps the first-quote rule.
  String values are decoded before redaction (UTF-16 surrogate pairs included), so escaped
  newlines and `\uXXXX` escapes cannot hide a key; embedded JSON and digest lines inside a string
  keep their own context, and a line with a complete inline key block stays on the JSON path.
- Redaction (#29): a backtick inside a type annotation, or a quote in a YAML block header's
  comment (`password: | # "..."`), is hidden too. A
  7 KB minified API list with `"token"` first now keeps over 99% of its text instead of 0.5%.

### Fixed

- CLI (#28): every value-taking flag (`--dir`, `--lines`, `--idle-exit`, `--max-requests`,
  `--listen`, `--provider`, `--upstream`, `--profile`, `--context-tokens`, ...) rejects a missing,
  flag-shaped (`--dir --raw`, `-h`) or blank value instead of taking the next flag or falling back
  to a default. `icx hook` still fails open.
- Hook fail-open replies (#30) come from one function in `icx-adapters`. They have the shape of
  each adapter's own no-op, pinned by a test against `AntigravityAdapter`: agy unknown events
  return `{}`, and agy Subagent events return `injectSteps` without `terminationBehavior`. Hermes
  gets `{}`. Blank `--agent` / `--event` values are rejected.
- Muse (#30): a Bash result with an empty `output` falls back to `stdout` / `stderr`.

### Documentation

- Muse loads MCP servers only in a trusted workspace; hooks run either way (deployment guide §2.6,
  adapter spec §6).
- README reworked as a user-facing landing page: a Getting Started section (verified release-binary
  install, per-agent setup links, first steps), a CLI Usage section, ADR-0006, the v1.2.0 audit
  and release notes in the documentation lists, and badges grouped in a titled table by topic.
  The per-release "What Changed" summaries and the phase-by-phase roadmap history moved into this
  changelog as "Highlights" under each version.
- `.gitignore`: an in-repo workspace store (`icx.sqlite3` and its WAL/SHM files), `install.key`,
  and local agent and environment files.
- Release notes state that stored content is not re-checked after an upgrade.

### Build

- The Antigravity PR review pins `AGY_PRINT_TIMEOUT` to 15 minutes (a 79 KB diff timed out at
  the scaled 367 s); `.gitignore` already excludes Playwright MCP output (`.playwright-mcp/`).

## [1.3.0] - 2026-11-21

Adds Meta Muse Code as a fifth agent. Release notes:
[`docs/releases/v1.3.0/`](docs/releases/v1.3.0/README.md).

### Highlights

v1.3.0 adds **Meta Muse Code** as a fifth agent (`icx hook --agent muse`). The adapter follows the
hook contract observed on Muse 1.4.25: session start and each prompt receive icx's tail slot and
recall, and every tool output is captured for recall and `deref`. Muse does not let hooks replace
tool output, so icx does not shrink output inside Muse; its value there is the shared working
context across agents. It also hides more quoted text inside function calls under secret-looking
keys and rejects `--dir` without a folder. Upgrade every agent to v1.3.0 before wiring Muse. Details:
[v1.3.0 release notes](docs/releases/v1.3.0/README.md), setup in the
[deployment guide](docs/deployment-and-agent-integration-guide.md) §2.6.

### Added

- **Meta Muse Code adapter** (`icx hook --agent muse`, `AgentKind::Muse`, `MuseAdapter`). Built
  from Muse Code 1.4.25's observed hook contract (probe hooks on a real session): Claude-Code-shaped
  payloads, `additionalContext` on SessionStart/UserPromptSubmit/PostToolUse, `hookEventName`
  required in `hookSpecificOutput`. SessionStart and UserPromptSubmit return the tail slot and
  recall; PostToolUse/PostToolUseFailure capture the output (the Bash tool's JSON result is
  unwrapped to its text and exit code). Muse rejects replacing tool output, so icx stores and
  recalls Muse output but does not shrink it. Every other event gets `{}`; no reply has a `null`.
  `icx doctor` detects the Muse binary and settings. Setup: deployment guide §2.6.
- `AgentKind::from_repr` is now the single decoder of the agent byte on the telemetry bus, and a
  test round-trips every variant, so a new agent cannot be decoded as `GenericMcp` (which would
  echo Muse's own events back to it as sibling activity).

### Fixed

- Redaction: a value that starts with a call (dotted names allowed, e.g. `pydantic.SecretStr("...")`,
  `field(default="...")`, `unwrap_or(x, "...")`) has every quoted literal in its argument list
  redacted, not only one directly after the first `(`; union type annotations
  (`str | None = "..."`) keep their literal defaults redacted (review of #25).
- CLI: a `--dir` with no value (trailing, followed by another flag, or empty) is rejected instead
  of falling back to the default workspace; every `icx hook` fail-open path replies with the
  agent's own no-op for that event (agy: `allow` on PreToolUse, `stop` on Stop).
- Muse adapter review (#27): only the Bash tool's JSON result is unwrapped (other tools' JSON text
  is stored verbatim); subagent identity falls back to `child_session_id`; a PostCompact
  `compact_summary` is stored.
- Deployment guide (review of #26): the ai-memory MCP JSON example carries `--server-url` and an
  expanded path; the agy wrapper requirement sits next to the global `hooks.json` example; the
  daemon save-failure text matches v1.2.2.

### Known limitations

- Redaction of quoted secrets inside code-shaped values (chained calls, operator chains, template
  and prefixed literals, lambdas) is incomplete, as in v1.2.2; the redesign is tracked in #29.

### Compatibility

- `AgentKind` gains value 8 (`muse`). A v1.2.x binary cannot read a workspace in which a Muse event
  was recorded; upgrade every agent before wiring Muse.

## [1.2.2] - 2026-11-21

Supersedes v1.2.1, which was tagged but never published: a redactor panic was found after
tagging. v1.2.2 contains every v1.2.1 change below plus these fixes (issue #24).
Release notes: [`docs/releases/v1.2.2/`](docs/releases/v1.2.2/README.md).

### Highlights

v1.2.2 is a follow-up to v1.2.0 and supersedes v1.2.1, which was tagged but not published
after a redactor crash was found (fixed here, with an exhaustive no-panic test). Commands and paths are now indexed by component, so searching
for `seq 1 400` finds the output of `Bash:seq 1 400`; the index format is `ICXHDC05` and existing
indexes are rebuilt on first load. Token counts (`max_tokens: 4096`) and code type annotations
(`token: String`) are no longer hidden as secrets. The daemon survives a transient save failure
without acknowledging it, and history eviction never empties the window when keeping some fits.
Details: [v1.2.2 release notes](docs/releases/v1.2.2/README.md).

### Fixed

- **Redactor panic (denial of service).** A quoted literal inside a call that ended in a backslash
  at the end of the input (`api_key = Some("abc\`) stepped past the last byte and panicked,
  failing that hook and stopping the workspace daemon until the next hook restarted it. The escape
  skip is now bounded to the line, and an exhaustive test runs every combination of up to five
  risky fragments after four sensitive prefixes without a panic.
- Literals inside nested call wrappers (`Some(String::from("..."))`, `Box::new(Secret::new('...'))`)
  are redacted.
- Python `Optional[str]`, `list[str]` and Rust `&'a mut T` annotations are recognised (they were
  over-redacted); a literal default after one (`password: Optional[str] = "..."`) is still
  redacted, while `= None` / `= True` are not secrets.
- Every subcommand rejects a repeated `--dir` instead of silently using the last one; `icx hook`
  still fails open (prints `{}`).
- `ICXHDC04` tokens are now shape-validated like `ICXHDC05` ones before their postings are dropped.

## [1.2.1] - 2026-11-21 (tagged, not published)

Follow-up to v1.2.0: the review suggestions left open on PR #22 and two regressions found by
re-checking the real v1.1.0 workspaces on this release's own machine after the upgrade.

### Fixed

- **Recall of commands and paths.** Lookup tokens joined by `:` or `/` (`Bash:seq`,
  `crates/icx-hdc`) are now also indexed by component, so a query for `seq 1 400` ranks the
  command's own output first instead of documents that merely mention it. The snapshot format is
  `ICXHDC05`; postings in `ICXHDC04` snapshots (older tokenizer) are validated and dropped, and the
  MMU rebuilds them from the CAS event log on first load. v1.2.0 cannot read `ICXHDC05`.
- **Token counts are no longer redacted.** `max_tokens: 4096`, `"input_tokens": 1234`,
  `--max-tokens 2048`, `MAX_TOKENS=8192` and other `*_TOKENS` counters lost their values to the
  v1.2.0 plural-key rule. Credentials under such keys are still caught by the value detectors;
  `NPM_READ_TOKEN` and other non-counter keys keep the key rule.
  A numeric value (at most 10 digits) under any plural `*_TOKENS` key (`epoch_rollover_tokens:
  16_384`) is a counter too. Neither exemption applies to keys naming a credential (`AUTH`,
  `ACCESS`, `SESSION`, `API`, ...).
- **Type annotations are no longer redacted.** `let token: String = ...`,
  `api_keys: HashMap<String, Key>,` and `password: &str` are code, not credentials (an exact
  recognised type name ending the value; anything else keeps the key rule).
  Code bindings to a call or path expression (`let q_tokens = Self::query_tokens(q)`) are not env
  assignments; a literal assigned in code (`let api_key = "..."`, `token: String = "..."`) is
  still redacted.
- **Literals wrapped in a call are redacted.** `api_key: Some("...")`, `String::from('...')`: the
  value span used to stop at the opening quote and leave the literal visible (also in v1.2.0).
- **CSV digests after a closed bracket.** `logs[1].txt,...,<sha256>` keeps its public digest; only
  an unclosed `[` before the value marks a flow sequence.
- **Step eviction no longer empties history needlessly.** When half the history allowance cannot
  be reached while keeping an epoch, a step evicts the fewest epochs that fit.
- **Daemon:**
  - a failed dispatch is never saved, so the client's in-process retry is the only ingestion;
  - a transient save failure is reported (never acknowledged) and the unsaved state is
    discarded, and the daemon keeps serving, exiting only after 5 consecutive save failures;
  - bytes after the request line are rejected as a protocol error (one request per connection).
- `icx context-budget` rejects a repeated option and names the flag in number errors.
- The vocabulary cap admits an entry's tokens in first-occurrence order rather than
  lexicographic order, and a stored subject cut at 64 KiB no longer indexes a token fragment.
- The release-gate self-test uses the system temp directory instead of `/tmp/Infinity-Context`.

### Checked

- Re-scanning all 2,092 artifacts stored by the v1.1.0/v1.2.0 install on the release machine with
  the v1.2.1 rules left 14 lines that would gain a marker: all are synthetic test fixtures, review
  quotes or the `16_384` literal now exempted. No stored credential was found.

### Verified, no change

- `AssembledWindow::total_cached_prefix_bytes` with an empty pinned head matches the rendered
  prefix (regression test added).
- YAML `''` escapes inside flow sequences and block scalars under sequence entries were already
  handled (regression tests added).

## [1.2.0] - 2026-11-21

Remediation and bounded-context release candidate for the independent audit in `docs/codex/`.
Qualification results and remaining release decisions are tracked in
[`docs/releases/v1.2.0/`](docs/releases/v1.2.0/README.md).

### Highlights

v1.2.0 remediates the independent audit of v1.1.0 in [`docs/codex/`](docs/codex/README.md)
(findings `ICX-CODEX-001` .. `ICX-CODEX-102`) plus the defects found in its pre-release review.
Measured results, compatibility and open items are in the
[v1.2.0 release notes](docs/releases/v1.2.0/README.md); current contracts are defined by
[ADR 0006](docs/adr/0006-v1-2-0-context-and-compatibility.md).

- **Bounded context:** the icx-managed window now has a total budget (`icx context-budget`).
  At saturation old history is evicted in large steps behind one stable
  `ICX:HISTORY_OMITTED` frame, so the prompt prefix stays byte-stable between steps; omitted
  history stays dereferenceable.
- **Recall:** exact lexical postings back the HDC/fractal recall, ranked by inverse document
  frequency, so a unique identifier is never filtered out and outranks common words.
- **Redaction:** YAML block, multi-line and flow-sequence secrets are hidden, redaction is
  idempotent, and public digests (checksums, commit ids) are no longer over-redacted.
- **Durability:** the daemon replies only after the store save succeeds, survives lock
  contention, and no longer holds the workspace lock while reading a request.
- **Integrations:** strict MCP request validation; Codex hook replies carry no
  unsupported output field and no JSON nulls.
- **Releases:** binaries are built by CI from the tag for static musl and glibc 2.35+, with
  `SHA256SUMS`, build provenance and third-party notices.
- **Upgrade:** v1.2.0 rebuilds workspaces into the ICXHDC04 snapshot format on first use.
  v1.1.0 cannot read an upgraded workspace (it fails closed and loses nothing), so upgrade the
  single `icx` binary used by all agents and keep a backup if you may downgrade. See
  [`UPGRADING.md`](docs/releases/v1.2.0/UPGRADING.md).

### Migration

- HDC derived snapshots now use `ICXHDC04` with exact token postings. Version 03 remains readable
  and the MMU rebuilds it from preserved CAS events. Older binaries cannot read 04: retain a
  pre-upgrade workspace backup for downgrade. CAS identities and unversioned Q15 `fc` words do
  not change; incorrect Q1.14 wire labels are superseded by ADR 0006.
- icx-managed context now defaults to a bounded 32,768-unit policy, reserving 4,096 units each for
  external input/tool schemas and completion. `icx context-budget` inspects/sets the persisted
  policy. The estimator is a conservative one-UTF8-byte allowance, not a provider tokenizer;
  callers must account for their actual external prompt. Model changes can invalidate prefixes.
- MCP requires valid JSON-RPC envelopes, typed arguments, an explicit operation and the canonical
  advertised tool name `icx`. Invalid aliases/arguments now return errors rather than status or
  accidental mutations. Native-hook alias recognition is unchanged.

### Fixed

- YAML sensitive scalars are redacted across literal/folded blocks and indented plain-value
  continuations, including `=`-containing values, tagged/anchored headers and list mappings.
  Public fields after dedent remain intact. Redaction uses normalized continuation classification
  so the audit's vendor-token/idempotence counterexamples stabilize on the first pass
  (ICX-CODEX-001, ICX-CODEX-006). Existing stored artifacts are not rewritten.
- Public Profile A skeleton rendering neutralizes raw packet/header delimiters as ASCII bracket
  pairs; standalone parsing rejects unescaped nested markers (ICX-CODEX-002). Existing ordinary
  skeletons and percent literals retain their interpretation. Exact CAS content is unchanged.
- Exact token postings prevent approximate candidate rejection of stored unique identifiers,
  including multilingual identifiers. Lexical match coverage ranks ahead of HDC/fractal ties;
  common-token queries can require exhaustive refinement (ICX-CODEX-004).
- Snapshot loads reject nonfinite/out-of-range numerical state and inconsistent cached summaries,
  bundler readouts and 04 posting/Bloom metadata (ICX-CODEX-003).
- Bus readers reject coherent slots whose sequence differs from the expected ring position;
  extreme public chunker sizes no longer overflow arithmetic (ICX-CODEX-032/033).
- Daemon success replies follow persistence under the workspace lock; save failure returns an
  error and nonzero daemon result. SQLite WAL uses FULL commit synchronization (ICX-CODEX-030).
- Codex PostToolUse captures eligible output without emitting the unsupported replacement field.
  Supported prompt/start recall remains; replacement capability is explicitly false (035).
- Window assembly selects fitting whole recent frames without deleting cold history. Structured
  recall preserves useful handles and leaves unsent sibling-bus events queued (101).
- Release packaging requires successful trusted CI and REUSE evidence for the exact tagged
  commit, and rechecks before external writes. Missing, partial, failed or expired evidence
  blocks release; a partial failed-job rerun may require a complete workflow rerun (070).
- Benchmarks measure 1k and 10k scans independently and preserve strict timing-gate failures;
  numerical, corruption, protocol and mutation regressions strengthen assertion coverage
  (072/073). Current specifications supersede historical capability guarantees, and the worked
  U+28C7 example now displays the matching Braille glyph (005/100/102).

Release-review fixes (independent review of the candidate before submission):

- Redaction: a line ending in a sensitive YAML block header (`password: |`) no longer exempts
  earlier assignments on the same line (`{api_key: x, password: |` leaked `x`). Multi-line plain
  scalars whose continuation lines are less indented than the first, multi-line single- and
  double-quoted scalars, and flow-sequence values (`password: [x]`) are now hidden. Plural
  collection keys (`api_keys`, `tokens`, `DB_PASSWORDS`) are sensitive; `max_passes`, `bypass`
  and `addresses` are not.
- Context window: once the budget is reached, history is evicted in large steps (down to half
  the history allowance) behind one stable `ICX:HISTORY_OMITTED` marker frame, instead of one
  epoch per turn. Consecutive windows now share their prefix between eviction steps (0.94 byte
  reuse at saturation in the 8,192-unit test, previously 0.01); omitted epochs stay
  dereferenceable through the handles in the marker. The eviction point persists with the
  controller state.
- Lexical index: tokens longer than 256 bytes are not posted, so a long hex/minified run can no
  longer make the persisted ICXHDC04 snapshot undecodable; stored subjects are capped at 64 KiB.
  Ranking weights matched query tokens by inverse document frequency, so a rare exact
  identifier outranks stop-word matches, and very common tokens no longer pull the whole corpus
  into exact scoring (the candidate set is bounded). Snapshot format and version are unchanged.
- Bus: `poll_since_with_loss` derives loss from the same head observation as the delivered
  events, so concurrent publishers can no longer make it under-report overwritten events.
- Redaction no longer hides public digests. The v1.1.0 rule is unchanged (a high-entropy hex
  run of 32+ characters on a line containing `=` or `:` is redacted) except for 40-, 64- and
  128-character digests in recognised contexts: `sha1:`/`sha256:`/`sha384:`/`sha512:`/`blake3:`
  labels, `sha256sum` lines, `commit <hex>` lines, CSV fields and Markdown table cells. Stored
  output can therefore still be checked against `SHA256SUMS`. Sensitive key names are
  unaffected.
- YAML/JSON comment and key-separator detection ignores `#` and `:` inside quoted values, so a
  colon inside an open quoted secret no longer leaves its continuation lines unredacted and a
  ` #` inside a closed quoted value no longer redacts the following public lines; single-quoted
  values honour the `''` escape.
- A `]` inside a quoted element of a sensitive flow sequence (`password: ["a]b", "c"]`) no
  longer ends redaction early.
- Text after the closing quote of a multi-line quoted secret is classified like any other line,
  so a key block or a new block/quoted secret opened there is still hidden.
- The CSV-cell digest exemption no longer applies to quoted elements or flow sequences
  (`keys: ["a", "<hex>", "b"]` keeps the strict hex rule).
- `icx context-budget --help` / `-h` print usage instead of a missing-value error.
- The `ICX:HISTORY_OMITTED` frame uses the same `⟪…⟫` delimiters as the other icx markers.
- Tests write to a per-user directory under the system temp dir instead of the fixed
  `/tmp/Infinity-Context/v120`, which could collide between users.
- Daemon: a workspace-lock timeout or reload failure now fails only that request (the client
  falls back to in-process handling) instead of stopping the daemon. The request is read and
  parsed before the lock is taken, under one 10 s deadline for the whole read, so a slow or
  silent client can no longer hold the workspace lock and starve hooks and `mcp-serve`.
- Codex hook replies omit null members (`"reason": null`, `"systemMessage": null`); Codex CLI
  0.171 marked hooks returning them as failed.
- The CLI integration test now runs `icx status`/`deref` against a private temporary state
  directory instead of the invoking user's default workspace.
- Exact parallel HDC scan: `icx_hdc::hv::hamming_distances`, `nearest` and `par_map_ordered`
  split inputs of at least 2,048 vectors across up to 8 scoped threads (no `unsafe`; results
  identical to a sequential pass, ties to the lowest index). Recall scores large candidate sets
  with it. The `icx benchmark` 10k gate now measures this scan against a measured 2,000 µs
  ceiling (slowest of 10 quiet-host runs, 1,354 µs, times 1.5); the single-threaded 10k figure
  is still reported, ungated. The previous 1,500 µs ceiling was an unmeasured extrapolation of
  the cache-resident 1k gate, and the 12.8 MB single-threaded scan is memory-bandwidth bound.

### Added

- `icx benchmark --json` emits measured component values, build flags, target, CPU, fixture hash
  and gate outcomes. `scripts/benchmark_series.py` records independent processes, warm-up
  exclusion and dispersion; measurements are not end-to-end hook/model guarantees.

- Release workflow (`.github/workflows/release.yml`): a version-tag push builds Linux x86-64
  binaries (baseline and x86-64-v3) as static musl and as glibc 2.35+ builds, writes
  `SHA256SUMS`, attests build provenance and attaches them to a draft GitHub release whose notes
  are the tag's CHANGELOG section. The tagged commit must be on `main`.
- Release archives include `THIRD-PARTY-NOTICES.txt` (generated by `cargo-about`) with the
  license texts of the statically linked crates, plus musl's notice in the musl builds; the file
  is also attached to each release on its own.

### Removed

- The Unicode and ISO/IEC JTC 1/SC 2/WG 2 publications in
  `ref-docs/unicode-braille-patterns/` (the U+2800 code chart, the Unicode 5.2.0 chapter 15
  extract, WG2 N1203/N1279/N1303/N1353/N1453/N1603/N1703/N1770 and UTC 1996-002/027.2) are no
  longer redistributed: their Unicode-3.0 annotation covers Unicode data files, not these
  documents (ICX-CODEX-071). `SOURCES.md` there lists each document with its official source
  (byte-identical copies verified for three) and the SHA-256 of the copy previously
  distributed; the Unicode Character Database files remain. Git history is unchanged.
- Raw audit/qualification logs and mutation inventories (42 files, 42.4 MB) moved out of the
  repository into the `icx-v1.2.0-raw-evidence.tar.gz` release asset, indexed with checksums in
  `docs/releases/v1.2.0/RAW-EVIDENCE.md`. Summaries, triage and all referenced evidence remain.

## [1.1.0] - 2026-11-20

Remediation release for the adversarial audit of v1.0.0 in [`docs/claude/`](docs/claude/README.md).
Finding IDs (`ICX-AUD-NNN`) refer to that audit; per-finding status is in
[`docs/claude/12-v1.1.0-remediation-status.md`](docs/claude/12-v1.1.0-remediation-status.md).

### Highlights

v1.1.0 is a remediation release that follows an internal audit of v1.0.0 (findings `ICX-AUD-001` through `ICX-AUD-055`, in [`docs/claude/`](docs/claude/README.md)). Workspaces now default to a private per-project directory under `$XDG_STATE_HOME/icx/workspaces/` and are stored in a single owner-sealed SQLite file. A repository-local `.icx/` from v1.0.x is no longer loaded implicitly and must be adopted with `icx trust`. Hook cost no longer grows with workspace size: on a 338-artifact workspace an in-process hook fell from 940 ms to 14 ms, and with the new resident `icx daemon` the median on a workspace of about 2,000 artifacts is 3.1 ms. The proxy now requires a per-install token. Hook outputs were aligned with the current Claude Code, Codex, Antigravity (`agy`) and MCP contracts. The cache figure was renamed to what it measures, a byte-weighted prefix-reuse proxy. The bus reports dropped publishes instead of reporting success. Ten arXiv PDFs that may not be redistributed were removed. The full list follows below; corrections to the research corpus are in [`ref-docs/corrections-2026-11-18.md`](ref-docs/corrections-2026-11-18.md).

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
- `icx trust` adopts a store's content but not its emitted-block digests, so a planted digest
  cannot exempt forged markers from escaping. MCP `grep` results carry the same
  `trust=untrusted_data` provenance header as `deref` and `slice`.
- `icx mcp-serve` and `icx proxy` re-verify the workspace per request, so a later `icx trust`
  takes effect and a workspace whose ownership, mode or seal changed is no longer used.
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
  since it was loaded). If the tables do not hold exactly the in-memory rows (rows removed or
  planted behind the process), they are rewritten from memory before sealing. A new
  workspace's unsaved state survives a first flush that failed after creating the store file. The daemon logs an error when another writer's newer
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
- `WireProxyTransformer::serve_http_listener_gated` with a per-request `WorkspaceGate`.
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
- Licensing and patent governance text (PATENTS.md, CLA.md, COMMERCIAL.md) is unchanged by owner
  decision (ICX-AUD-035).

## [1.0.0] - 2026-11-18

### Highlights

Phase 5: Long-Horizon Verification, 9-SLO Benchmark Suite, AHTL Exactness Closure & Production Release ([Overview (`Sprints 5.1–5.2`)](to-dos/phase-5-verification-benchmarks-and-release/overview.md)).

500-turn (`> 8.4 MB` / `> 2.1M` raw tokens) Needle-in-a-Haystack / OOLONG & prefix-cache simulator with passkey recall, 4-agent shared-file bus verification across Claude Code, Antigravity CLI, Codex, and Hermes, full 256-bit `Blake3Digest` collision guards in `CasStore::put_bytes_checked`, adaptive Stage-1 `HdcAssociativeIndex` candidate scaling (`(entries.len() / 64).clamp(16, 512)`), bijective `AhtlClause` key/value escaping, enhanced `icx doctor [--dir <PATH>]` (`verify_all_integrity` + `verify_all_merkle_integrity_with_cas`), and 9-SLO `icx benchmark` scorecard.

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

### Highlights

Phase 4: Cross-Agent Adapters, Polymorphic MCP Pager & Cache-Epoch Wire Proxy ([Overview (`Sprints 4.1–4.3`)](to-dos/phase-4-cross-agent-interception/overview.md)).

`AgentHookAdapter` trait, `ClaudeCodeAdapter`, `CodexAdapter` + bounded `CodexRolloutTailer`, `AntigravityAdapter` (`exit 0`, UI-silent `systemMessage`, `512 KiB` tail-window `transcriptPath` fallback), `HermesAdapter` (`ContextEngine` & `MemoryProvider` v2 with `tool_calls` group preservation and occurrence-aligned `ingest_or_reuse_at_index`), `< 180` token Single-Tool Polymorphic `McpPagerServer` (`icx` with `⟪ICX:PROVENANCE│...⟫` redaction honesty, 16-hex ambiguity recovery hints, and bidirectional `xagent`), `WireProxyTransformer` (`Anthropic`, `OpenAI`, `Gemini`, SSE `⟪!icx:deref(...)⟫` expansion, and safe `TcpListener` `icx proxy`), and the comprehensive [Deployment & Agent Integration Guide](docs/deployment-and-agent-integration-guide.md).

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

### Highlights

Phase 3: 10,240-Bit HDC, Contractive IFS Fractal Archive (`FCGA`), Titans-Inspired Surprise Gate & Bi-Temporal CSR Graph (`PPR`) ([Overview (`Sprints 3.1–3.3`)](to-dos/phase-3-hyperdimensional-and-fractal-index/overview.md)).

10,240-bit `Hypervector` algebraic engine (`BitXor`/`BitXorAssign`, `unbind`, `inverse_permute`, `from_seed_framed`, `bind_role_filler`, `encode_sequence`, `cosine_bipolar`, `sub_hypervector_64`, `dedup_fingerprint_u128`), Infini-attention Delta-Rule `HdcBundler` with coordinate-parity tie-breaking and 128-bit deduplication, two-stage 16-byte `EntrySummary` L2-cache pre-screened `HdcAssociativeIndex` (`AssociativeMemoryIndex`), Contractive IFS `FractalCoord` (`verified_suffix_depth`, `to_q1_14`/`from_q1_14`, 64-bit Morton Z-order `morton_z64`, `@o1:o2` octal quadrant addressing) and `FractalTrajectoryArchive` verified against Theorem 3.2 bounds ($K \in 1..=32$), `TitansSurpriseGate` (`apply_idle_decay`, `observe_with_exit_code`, Delta-Rule novelty gating), and `BiTemporalGraph` with automatic functional relation supersession, atomic JSON persistence, and `CsrGraphSnapshot` $O(V + E)$ Personalized PageRank (`PPR`).

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

### Highlights

Phase 2: Alien Holographic Token Lattice (`AHTL`), Braille-256, Multi-Language Skeletonizers & 4-Tier Secret Redaction ([Overview (`Sprints 2.1–2.3`)](to-dos/phase-2-alien-codec-and-compression/overview.md)).

Dual-Profile `AHTL v1.0` wire codec (`Profile A` `AhtlWirePacketV1` & `Profile B` `AhtlPacket`, `EpochHeader`, `AhtlStream`, `ContextCodec`) with bijective `%25`/`%23L` escaping, Unicode Braille-256 (`8`-byte & `64`-byte) bijection (`byte_to_braille_utf8`, `braille_utf8_to_byte`, `ChannelBHologram`), 1-Lipschitz 64-band HDC spectral folding (`Hypervector::spectral_bands_64`, `from_u64_seed`, `hv_digest_u64`, `hamming_distance_simd`), `FractalCoord::encode_fc_hex64` / `decode_fc_hex64`, multi-language `StructuralSkeletonizer` (`Rust`, `Python`, `TS/JS`, `Go`, `C/C++` + `cargo test`/`clippy`, `pytest`, `tsc`/`vitest`/`jest`, `go test` + unified `git diff` hunks), and 4-Tier `SecretRedactor` with Shannon entropy gating ($H(S)$) and `FLAG_IS_REDACTED` tagging, cross-referenced against `docs/ahtl/AHTL_v1_Complete_Technical_Specification_Volume.pdf`.

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

### Highlights

Phase 1: Core CAS, Merkle-DAG (`LCM`), `/dev/shm` Seqlock Bus & Zero-`unsafe` Enforcement ([Overview (`Sprints 1.1–1.4`)](to-dos/phase-1-foundations/overview.md)).

Two-threshold Normalized FastCDC with `LineIndexEntry` selective chunk line slicing, 48-bit (`#icx:<12-hex>`) and 64-bit (`#icx:<16-hex>`) BLAKE3 CAS handle resolution with ambiguity guards, 4-tier `MerkleContextDag` (`L1`..`L4` + `ToolArtifact`) with nested `Context-Folding` and `L3 -> L4` epoch compaction, a 200-turn prefix-cache simulation, the `AtomicU32`/`AtomicU64` in-process `SeqlockTelemetryBus` with a file-backed variant, permanent workspace-wide `unsafe_code = "forbid"`, Rust Edition `2024` / MSRV `1.100`, and `>= 95%` Codecov gate.

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

### Highlights

Phase 0: Research, REA Forensics, Specifications & Workspace Bootstrap ([Overview](to-dos/phase-0-precursory-setup/overview.md)).

38-source deep research report, 22 indexed arXiv papers (`ref-docs/papers/`; 10 of the PDFs were removed in v1.1.0 because their license does not permit redistribution), REA binary/SQLite forensics on Claude Code, Codex, Antigravity CLI (`agy`), and Hermes (`ref-docs/agent-internals/`), 13 normative specs + 4 ADRs (`docs/`), 9-chapter AHTL v1.0 specification volume (`docs/ahtl/`), multi-tier GPL-3.0/AGPL-3.0/CC-BY-NC-ND-4.0 + 5-family patent IP governance (`REUSE 3.3`), and 8-crate workspace bootstrap.

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
