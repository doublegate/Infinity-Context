# icx v1.3.1

Released 2026-11-22 ([GitHub release](https://github.com/doublegate/Infinity-Context/releases/tag/v1.3.1)).
A patch release on [v1.3.0](../v1.3.0/README.md) that fixes
[#28](https://github.com/doublegate/Infinity-Context/issues/28),
[#29](https://github.com/doublegate/Infinity-Context/issues/29) and
[#30](https://github.com/doublegate/Infinity-Context/issues/30).

## Redaction of quoted secrets in code (#29)

icx redacts tool output before storing it, so anything hidden is gone from its store for every
agent. The automated reviews of v1.3.0 found that a quoted secret could stay visible when the value
under a sensitive key (`password`, `token`, `api_key`, ...) was code rather than a plain literal.

From v1.3.1, after a sensitive key, **a value that holds any quote is hidden from its first quote to
the end of the line**, as one marker. A call head right before that quote stays visible:

```text
password = env::var([REDACTED:ENV_SECRET]
api_key = os.getenv([REDACTED:ENV_SECRET]
```

Any other value is hidden from its start, because a bare head such as `hunter2 "note"` could
itself be the secret: `api_key = settings.key or "..."` becomes `api_key = [REDACTED:ENV_SECRET]`.

The rule is deliberately blunt. A first version hid each quoted string separately and kept the
code between them. Review of the release PR found that a stray quote earlier on the line shifted
the pairing and left a secret visible. Hiding later JSON key names also stopped icx from
recognising later sensitive keys, so `{"api_key": "x", "password": secret}` leaked the bare value.
Rules that stop at a "next field" were bypassed the same way. So there is no quote pairing, no
field boundary and no key detection, and later text on the same line is hidden with the value:
`{"password": [REDACTED:CONFIG_SECRET]`, `curl --api-key=[REDACTED:CLI_SECRET]`. Your files are not
changed; only icx's stored copy is.

**Complete JSON lines are redacted exactly.** Minified API responses and one-line JSON logs would
lose almost everything under the first-quote rule. In a 7 KB list with `"token"` first, only 0.5%
of the text survived. So a line that is one complete JSON object or array is parsed by a strict
RFC 8259 scanner instead:

```text
{"access_token":"[REDACTED:CONFIG_SECRET]","token_type":"bearer","expires_in":3600,"user":{"login":"octo"}}
```

The value of a sensitive key, of any type, becomes one marker. Every other string is redacted on
its own with its key as context. Formatting and key order are kept, and the edited line must still
parse. A line that is not valid JSON (a trailing comma, a log prefix, deeper than 128 levels) keeps
the first-quote rule, so a parsing problem can only hide more, never less.

A property test checks the rule over random code shapes. They combine calls, chains, ternaries,
generics, lifetimes, lambdas, and prefixed and template literals; a second one generates random
JSON with sensitive keys at any depth. The corpus also pins every bypass found in review.

## Other fixes

- **CLI flags (#28).** Every value-taking flag rejects a missing, flag-shaped (`--dir --raw`, `-h`)
  or blank value. Before, it took the next flag as its value or fell back to a default, so `icx
  proxy --upstream --dir /tmp/ws` silently used the default workspace. `icx hook` still fails open.
- **Fail-open hook replies (#30).** They come from one function in `icx-adapters`, in the shape of
  each adapter's own no-op, and a test pins them to the agy adapter's replies.
- **Muse (#30).** A Bash result with an empty `output` falls back to `stdout` / `stderr`. Muse
  loads MCP servers, `icx` included, only in a trusted workspace; hooks run either way (deployment
  guide §2.6).

## Upgrading

Install over v1.3.0; the on-disk format is unchanged.

**Stored content is not re-checked.** icx redacts tool output once, when it stores it. Anything
v1.3.0 or earlier stored before this release, including secrets inside code that the new rules
would now hide, stays in the workspace as it was and can still be recalled. If that matters for
a workspace, scan its stored artifacts with the v1.3.1 redactor, or start that workspace afresh.

## Verification

- 339 workspace tests, line coverage 95.21%; the #29 property test was also run with 20,000 cases.
- `cargo clippy -D warnings`, `cargo doc -D warnings`, REUSE 3.3, gitleaks.
- Exact-commit CI on the merge commit; release assets checked against `SHA256SUMS` and build
  attestations.
