# icx v1.2.2

Released 2026-11-21 from pull requests #23 and #25 ([GitHub release](https://github.com/doublegate/Infinity-Context/releases/tag/v1.2.2)).
v1.2.2 is a follow-up to [v1.2.0](../v1.2.0/README.md) and supersedes v1.2.1.

## Why there is no v1.2.1 release

v1.2.1 (#23) was tagged, and its release workflow built a draft. An automated security review
then found that the redactor panicked on a call-wrapped literal ending in a backslash at the end
of the input (`api_key = Some("abc\`). The panic failed that hook and stopped the workspace
daemon until the next hook restarted it; nothing stored was lost. The draft was never published
and was deleted. The `v1.2.1` tag remains, because published tags are not rewritten.

## What changed

- **Recall of commands and paths.** Tokens joined by `:` or `/` are also indexed by component, so
  `seq 1 400` finds the output of `Bash:seq 1 400`. On the release machine the command's output
  ranked first (similarity 1.00) after the upgrade; under v1.2.0 it was outside the top 10.
- **Redaction precision.** Token counters (`max_tokens: 4096`, usage blocks,
  `epoch_rollover_tokens: 16_384`), source type annotations and code bindings to call or path
  expressions are no longer hidden. Literals assigned in code, wrapped in calls (also nested) or
  given as defaults stay redacted, and keys naming a credential (`AUTH`, `ACCESS`, `SESSION`,
  `API`, ...) are never exempt.
- **Redactor robustness.** The panic above is fixed; a test runs every combination of up to five
  risky fragments after four sensitive prefixes (about 2.1 million inputs in release builds).
- **Context window.** Step eviction never empties the retained history when keeping some fits.
- **Daemon.** A failed dispatch is never saved; a save failure is reported, never acknowledged,
  and the daemon keeps serving (it exits after 5 consecutive failures); bytes after the request
  line are a protocol error.
- **CLI.** A repeated `--dir` is rejected by every subcommand (`icx hook` fails open), and
  `icx context-budget` rejects repeated options.

Full list: [`CHANGELOG.md`](../../../CHANGELOG.md) sections `[1.2.2]` and `[1.2.1]`.

## Upgrading

Replace the single `icx` binary used by all agents. On first use v1.2.2 writes the `ICXHDC05`
index format and rebuilds the token postings of a v1.2.0 (`ICXHDC04`) index from the stored
event log; no stored content changes. v1.2.0 cannot read `ICXHDC05` and fails closed, so keep a
pre-upgrade backup if a downgrade might be needed. Restart or reconnect long-running `icx`
processes (MCP servers, daemons) after the swap: an old process cannot read the new format.
See [`UPGRADING.md`](../v1.2.0/UPGRADING.md).

## Verification

- 283 workspace tests, 0 failures; line coverage 95.39%; `cargo clippy -D warnings`;
  `cargo doc -D warnings`; benchmark SLOs; REUSE 3.3; gitleaks.
- Exact-commit CI and REUSE on the merge commit; all release assets match `SHA256SUMS` and pass
  build-provenance attestation.
- On the release machine, 27 real workspaces were upgraded from v1.2.0 with unchanged artifact,
  chunk and DAG counts, and all 2,092 artifacts stored by v1.1.0/v1.2.0 were re-scanned with the
  new rules: no stored credential was found.
