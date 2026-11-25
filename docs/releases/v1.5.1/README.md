<!--
SPDX-FileCopyrightText: 2026 DoubleGate
SPDX-License-Identifier: CC-BY-NC-ND-4.0
-->

# icx v1.5.1

Released 2026-11-25. A patch release that closes the redactor shapes the v1.5.0 red-team pass left
open ([#104](https://github.com/doublegate/Infinity-Context/issues/104)). Each of those shapes also
leaked in v1.4.1. There are no format or protocol changes.

## What the redactor now hides

Every shape below has a test in `crates/icx-codec/tests/redact_v151.rs`.

- **Markup and name/value pairs:**
  - an XML element named by a sensitive key (`<password>…</password>`);
  - a tag's `value` attribute when its `key` or `name` attribute is sensitive
    (`<add key="Password" value="…"/>`);
  - JSON `name`/`value` and `key`/`value` pairs;
  - Kubernetes-style YAML `name:` with a sibling `value:`;
  - list items and `value:` members under a sensitive block key such as `tokens:`.
- **Assignments that are not `key = value`:** subscripts (`obj["password"] = "…"`), `=>` pairs,
  and shell `set`, `set -x` and `setenv`. For subscripts and `=>` only a quoted literal is hidden,
  and code on the right stays visible; a shell `set` value is hidden quoted or not, except a
  command substitution.
- **Password-taking CLI tools:**
  - a value starting with `-` after a sensitive long flag;
  - `sshpass -p` and `docker`/`podman login -p`;
  - glued `mysql -pX`;
  - `curl -u user:pass`;
  - the input of `--password-stdin` (`<<< x`, `echo x |`);
  - JSON argv arrays (`["--password", "x"]`).
- **URLs:** signature and token parameters (`sig`, `X-Amz-Signature`, OAuth `code`,
  `#access_token`), and webhook URLs, since the URL is the credential.
- **Unicode look-alikes:** text with no-break or other Unicode spaces, zero-width characters, a
  soft hyphen or full-width forms is redacted in normalised form. A line with nothing to hide keeps
  its original bytes.
- **Planted markers:** text glued to a `[REDACTED:…]` marker at the start of a value is hidden.
- **Multi-line values:**
  - escaped and interior triple quotes;
  - a quote at the start of the value;
  - prefixed and tagged literals (`+ r"""`, `` String.raw` ``);
  - unspaced and empty annotations (`api_key: str="""`, `password: str =`);
  - an apostrophe before the `=` (`don't log; password = """`).
- **PIN and OTP:** plain numbers of 4 to 10 digits under `pin` or `otp` keys (`pin: 1234`,
  `otp2: 482913`). `pin: 3.11` stays visible.

## Kept visible by design

These are owner decisions, recorded in [ADR-0011](../../adr/0011-structural-redactor.md) with the
reasons:

- **Text around a value:** prose after a sensitive word (`password reset flow`), and comment
  text after a value.
- **Not credentials:** JSON `sessionId` UUIDs, and values under keys that are not sensitive.
- **Ambiguous shapes:** `-p` when no password-taking tool is named (it is usually a port), a plain
  user name in a URL, and a bare `key` member.

Of the 82 inputs in #104, 25 still leak, and every one of them is in this list.

## Idempotence

Each line is redacted again until it stops changing, at most five rounds, and whether a line
opens a multi-line value is decided the same way on every pass. This is a bounded loop rather
than a proof: in fuzz runs of 700,000 generated lines, including the new shapes, no line changed
on a second pass.

## Cost

- **Readability:** on this repository's 588 source and doc files, the only newly hidden lines are
  the redactor's own placeholder examples. The 19-line corpus is unchanged: 0 missed secrets and
  43 over-redacted bytes.
- **Speed:** the in-memory hook median is 323 µs against 308 µs for v1.5.0, the median of three
  runs on the same host. All benchmark gates pass.

## Upgrading

Replace the binary, then reconnect each agent's MCP server (`/mcp` in Claude Code). The Hermes and
OpenCode plugins have not changed. Content stored by an earlier version keeps that version's
redaction. `icx rescan` reports what v1.5.1 would hide in it.

## Verification

- **Tests:** 661 workspace tests pass, 0 fail, and 8 are ignored by design. The 6 OpenCode and
  27 Hermes plugin tests are unchanged.
- **Coverage:** 95.59% of lines at v1.5.0, 95.57% now (empty `HOME`, as in CI). The new
  `redact_shapes.rs` is at 95.56%.
- **Lint:** clippy, rustfmt, gitleaks and REUSE are clean.
- **Red-team inputs:** the 272 inputs pinned in v1.5.0 still pass.
