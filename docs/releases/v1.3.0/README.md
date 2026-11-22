# icx v1.3.0

Released 2026-11-21 ([GitHub release](https://github.com/doublegate/Infinity-Context/releases/tag/v1.3.0)).
v1.3.0 adds **Meta Muse Code** as a fifth supported agent, next to Claude Code, Codex, agy and
Hermes, and folds in the review follow-ups to [v1.2.2](../v1.2.2/README.md) listed under
[Other changes](#other-changes), two of which change existing behaviour.

## How the adapter was built

Muse ships as a launcher that downloads a 469 MB stripped Rust binary, with no published hook
reference, so the contract was established on the binary itself (Muse Code 1.4.25):

- Strings and error messages in the binary: the `HookEventKind` event set, the requirement that
  `hookSpecificOutput` names its `hookEventName`, and the rule "PostToolUse output cannot replace
  tool output".
- A real headless session with a probe hook on every event, which recorded each payload, the
  working directory and the environment, and returned marker text through `additionalContext`.
  The markers from SessionStart, UserPromptSubmit and PostToolUse reached the model.
- A second real session with the new adapter (branch build, isolated state): the model saw the
  icx legend, the Bash output was stored as plain text (`499` / `500`), and `recall` found the
  command at similarity 1.00.

## What the adapter does

| Muse event | icx |
| --- | --- |
| `SessionStart`, `UserPromptSubmit` | Returns the tail slot and recall as `additionalContext` |
| `PostToolUse`, `PostToolUseFailure` | Captures the output (Bash's JSON result is unwrapped) |
| `SubagentStart`, `SubagentStop`, `PreCompact`, `PostCompact`, `SessionEnd` | Subagent branches and epoch seals; a PostCompact `compact_summary` is stored |
| anything else | `{}` |

Muse does not let hooks replace tool output, so icx does not shrink output inside Muse. It stores
and recalls it, and shares it with the other agents in the same project, as it does for Codex.

## Other changes

These come from the automated reviews of #25 and #26 and apply to every agent:

- **Code-shaped values are redacted whole.** Under a secret-looking key, a value that looks like
  code and contains a quote is replaced entirely, up to the next field (`name=`, `name: value`,
  `--flag`) or the end of the line: calls (`pydantic.SecretStr("...")`, `env::var("K").unwrap_or("...")`),
  chains (`settings.api_key or "..."`), concatenation and ternaries. The trade-off is deliberate:
  harmless code on such a line is hidden too, for example all of `password = getenv("DB_PASSWORD")`,
  in exchange for a rule that cannot leave a literal visible. A later `user="admin"` field on the
  same line is left as it was.
- **Union type annotations** (`str | None = "..."`, `string | null = '...'`) keep their literal
  default redacted; `= None` stays visible.
- **`--dir` without a folder is an error.** `icx init --dir` or `icx status --dir` used to fall
  back to the default workspace silently; now the command fails. `icx hook` still fails open,
  with the agent's own no-op reply.
- **Deployment guide corrections:** the ai-memory MCP JSON example, the agy wrapper requirement,
  and the daemon save-failure description.

## Upgrading

`AgentKind` gains value 8 (`muse`). A v1.2.x binary cannot read a workspace in which a Muse event
was recorded, so install v1.3.0 for every agent before adding icx's Muse hooks. Setup:
[deployment guide](../../deployment-and-agent-integration-guide.md) §2.6. The `muse` launcher
updates Muse hourly by default; re-check with one `muse exec` after an update.

## Verification

- Workspace tests pass (new: `muse_adapter_v130`, `muse_hook_cli_v130`); line coverage 95.37%
  (`muse.rs` 97.09%); `cargo clippy -D warnings`; `cargo doc -D warnings`; REUSE 3.3; gitleaks.
- Exact-commit CI on the merge commit; release assets checked against `SHA256SUMS` and build
  attestations.
