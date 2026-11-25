<!--
SPDX-FileCopyrightText: 2026 DoubleGate
SPDX-License-Identifier: CC-BY-NC-ND-4.0
-->

# icx v1.5.0

Released 2026-11-25. This release closes Phase 7, retrieval and compaction quality:
[#45](https://github.com/doublegate/Infinity-Context/issues/45) to
[#48](https://github.com/doublegate/Infinity-Context/issues/48), plus #90 and the #95 install
follow-ups.

## Redaction hides the secret, not the line

Before v1.5.0, a value after a sensitive key (`token`, `api_key`, `session_id`, ...) was hidden
from its first quote to the end of the line. Comments, later JSON fields and harmless values
went with it (#90). The structural redactor ([ADR-0011](../../adr/0011-structural-redactor.md))
hides only the secret:

- **Kept visible:** values that cannot be secrets. These are `null`, booleans, `""`, short
  numbers (not under password, PIN or OTP keys), `$VAR` references, upper-snake environment
  lookups and, in code, quote-free references such as `settings.SECRET_KEY`. Agent session UUIDs
  in JSON hook payloads also stay.
- **A single literal:** loses only its contents, inside its quotes. A comment or the next
  assignment after it stays.
- **Anything else:** keeps the v1.3.1 rule (#29), so it is hidden from its first quote to the end
  of the line.

An independent red-team pass ran before the redactor shipped as the default. It tried about 330
inputs and found three regressions in the new rules, plus older gaps. Several further review
rounds followed. Fixed in this release:

- **Key names:** camelCase, glued, dotted, numbered and qualified key names (`accessToken`,
  `PGPASSWORD`, `db.pwd`, `API_KEY_2`, `password_value`, `MYSQL_PWD`).
- **Unquoted values with spaces:** hidden to the end of the line. CLI flag values still end at
  whitespace.
- **Multi-line values after `=`:**
  - quoted, triple-quoted and backtick strings;
  - backslash-continued lines;
  - bracketed values, with quote state carried across lines;
  - type-annotated assignments and `:=`.
- **YAML plain scalars:** continued on indented lines.
- **Uncertain cases fail closed:** a glued `//` or `#` can keep a hidden value open, but never
  close it.

On the 19-line corpus, bytes hidden that were not a secret went from 183 (the v1.4.1 rule) to 43,
with no missed secret. 272 distinct red-team inputs are pinned as a regression suite. The shapes
that still leak are tracked for v1.5.1 in
[#104](https://github.com/doublegate/Infinity-Context/issues/104). They leaked in v1.4.1 too.

The red-team fixes hide more than the first version of the new rules did: 43 over-redacted bytes
instead of 24 on the corpus. Of the 159 bytes the first version stopped hiding, 140 (88%) stay
visible.

## `icx rescan`

Redaction runs once, when content is stored, so stored content keeps the rules of the version
that stored it. `icx rescan` runs today's redactor over every stored artifact and indexed subject.
It reports, as JSON, how many would change, the markers that would be added by kind, the bytes
affected, and up to `--limit` artifact handles. It never prints the text or the values, and it
writes nothing. Re-redacting a workspace (`--apply`) is planned for v1.7.0
([#102](https://github.com/doublegate/Infinity-Context/issues/102)).

## Compaction and prompt recall

- **Resume packet:** after Claude Code, Codex or Hermes compacts its context, the next hook that
  carries context adds, once, a bounded (3 KB) deterministic packet. It holds the most recent
  outputs with their `#icx:` handles, subjects that are failing now, and open fold branches.
  `ICX_RESUME_PACKET=0` turns it off.
- **Typed folds:** `icx_write` `fold` takes an optional `type` (`subtask`, `build_log`,
  `superseded`, `exploration`, `other`).
- **Prompt recall:** each user prompt gets at most one recall line, shown only when the hit's
  subject shares a distinctive word with the prompt, and no graph lines
  ([ADR-0010](../../adr/0010-recall-ranking-default.md)). Before, a prompt got up to about
  eight recall and graph lines, mostly unrelated. On 118 real prompts the line now appears twice.

## Retrieval (opt-in) and evaluation

- **`ICX_RECALL_RANKING=rrf`** keeps a contentless SQLite FTS5 index (BM25 over words, trigrams
  for substrings). It fuses BM25 with HDC recall by reciprocal rank. Off by default.
- **`icx eval-recall`** scores rankers by recall@k and MRR@10 on a synthetic set or a private
  query set. On real sessions, the answers were almost always produced just before they were
  needed, so the default ranking stays `legacy` and local embeddings are not built (ADR-0010).
- **Fractal suffix:** a shared fractal suffix is reported only for a history that really repeats
  (ICX-AUD-052).

## Upgrading

Replace the binary. After an agent's MCP server reconnects (`/mcp` in Claude Code), it runs the
new version. Nothing in an existing workspace changes. To see what the new redactor would hide in
older stored content, run `icx rescan`.

## Known issues

- Redactor shapes that leak in this release and in v1.4.1 are listed with their inputs in #104
  (v1.5.1).
- Live-run fixtures for Hermes, Gemini CLI and Cursor are deferred to v1.6.0 (#89).
- `icx rescan` only reports; stored content is not re-redacted until #102.

## Verification

- **Tests:** 641 workspace tests pass, 0 fail. 8 are ignored by design: 1 long-horizon
  qualification, 2 daemon child scenarios and 5 measurement runs. 6 OpenCode and 27 Hermes
  plugin tests also pass.
- **Coverage:** 95.59% of lines (empty `HOME`, as in CI).
- **Lint and security scans:** clippy, rustfmt, gitleaks and REUSE are clean.
- **Mutation testing:** on the Sprint 4 `redact.rs` diff, 551 mutants were caught and 15 timed out
  of 635 viable, 89.1% (ICX-AUD-039).
