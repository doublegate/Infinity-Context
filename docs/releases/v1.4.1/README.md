<!--
SPDX-FileCopyrightText: 2026 DoubleGate
SPDX-License-Identifier: CC-BY-NC-ND-4.0
-->

# icx v1.4.1

Released 2026-11-24. A patch release: fixes found while installing v1.4.0 on a real machine, and
the remaining suggestions from the v1.4.0 release review (#92). No format or protocol changes.

## Fixed

- **`icx install` keeps your config's formatting.** v1.4.0 re-serialised every JSON file it
  edited with two-space indentation. On a tab-indented, hand-aligned `opencode.json`, adding one
  MCP server rewrote all 370 lines. Now every existing byte is kept and only the new entries are
  inserted, indented like their neighbours, with the file's colon spacing and line endings
  (CRLF files stay CRLF); a one-line file stays on one line. Verified against a
  real `opencode.json`: the diff is the seven lines of the new `icx` entry.
- **Out-of-date Claude Code matcher.** An icx `PostToolUse` hook registered before v1.4.0 often
  has a matcher that names only Bash, web and MCP tools, so v1.4.0's shrinking of Read, Grep,
  Glob and Agent results never ran. `icx install` never edits an existing hook; it now prints a
  note with the matcher to use.
- **Multi-line commands.** An unquoted newline ends a command, in both the Rust and Python
  readers, so a two-line `cd /repo` and `icx deref ...` is recognised as icx's own command and
  its output is not stubbed again. A backslash before a newline joins the lines.
- **Hook paths without `..`.** Running `../bin/icx install` wrote `/some/dir/../bin/icx` into
  hooks, which breaks once that directory is removed. The canonical path is used instead.
- **Cleanup ownership.** A partly written temporary or backup file is removed only when icx
  created it.

## Upgrading

Replace the binary. If `icx install` prints the Claude Code matcher note, update that hook's
`matcher` in `~/.claude/settings.json` to the one it prints; new sessions pick it up.

## Verification

525 workspace tests (0 failed, 3 ignored by design), 6 OpenCode and 27 Hermes plugin tests, line
coverage 95.24% (empty `HOME`, as in CI), clippy and rustfmt clean. The v1.4.0 notes gave 24
Hermes tests; the true count at v1.4.0 was already 27.
