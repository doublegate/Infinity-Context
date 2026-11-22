# icx v1.3.2

Released 2026-11-22. Fixes #33, #34, #36 and #37, and problems found by mining four days of agent
sessions (1,129 `icx` MCP calls, 4,239 hook runs) for icx errors.

## Stubs that could not be dereferenced

In the field, 57 `deref`/`slice`/`grep` calls failed with "artifact not found". Every handle
existed; it was stored in a different workspace from the one the MCP server read. Two causes:

- **Git worktrees (#37).** A subagent working in `<project>/.claude/worktrees/agent-*` got a
  workspace of its own, because a worktree's `.git` file counted as a project root. icx now
  resolves a linked worktree to its main repository. The link is accepted only when git's own
  back-link (`<main>/.git/worktrees/<name>/gitdir`) points at the worktree, so a planted `.git`
  file cannot redirect a directory into another project's workspace. Submodules stay their own
  project.
- **Sessions that change directory (#34).** The MCP server chose its workspace once, from the
  directory it was launched in, while hooks follow the agent's current directory. Each hook now
  records the workspace it used for its agent process, the nearest ancestor that is not a shell or
  launcher (in `$XDG_RUNTIME_DIR/icx/agents/`, owner-only, named by PID and process start time),
  and `icx mcp-serve` uses the workspace its own
  agent's hooks recorded. Only workspaces under icx's state directory are accepted, and the usual
  trust checks still run. `--dir`/`ICX_DIR` pin the workspace; `ICX_MCP_FOLLOW_HOOKS=0` turns
  following off. A "not found" error names the workspace it searched.

## Other changes

- MCP results made of text blocks (Context7, docs servers) are stored as their text, so `slice`
  and `grep` address real lines.
- Hooks leave outputs under 2 KiB inline (`ICX_STUB_MIN_BYTES`). 57% of field stubs replaced
  outputs under 1.5 KB with a ~200-byte stub and a deref round trip.
- Recall entries that are weak on every signal are no longer injected (6% of injected entries in
  the field had similarity under 0.1).
- `slice` clamps an end past the last line; `grep` bounds lines (400 characters) and replies
  (32 KiB).
- CLI: repeated value-taking flags are rejected; `deref` and `encode-ahtl` find their handle or file
  wherever it sits.
- Redactor: dotted counter keys, multi-line quoted values, swallowed block headers and CRLF lines
  inside JSON strings (#33); a vendor prefix inside a word, and word slugs such as project
  directories and model names, are no longer redacted. 163 lines of unreachable code removed.
- Docs site build hardening (#36).

`PASS:` lines (test-result labels) are still redacted: such a line cannot be told apart from a
multi-word passphrase, so it fails closed.

## Upgrading

No storage format change: v1.3.1 and v1.3.2 read the same workspaces. After installing, reconnect
the `icx` MCP server in each agent (a running server keeps the old binary). Stubs that an older
version stored in a worktree's own workspace stay there; `icx deref <handle>` run from inside
that worktree still reads them while it exists.

## Verification

- 354 workspace tests (0 failed), line coverage 95.45%; codec property tests at 5,000 cases.
- `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, rustdoc with
  `-D warnings`, the benchmark SLO gates, REUSE and gitleaks.
- End-to-end tests in a real git worktree (`crates/icx-cli/tests/workspace_follow.rs`).
