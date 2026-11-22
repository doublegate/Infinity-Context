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

A property test checks the rule over random code shapes. They combine calls, chains, ternaries,
generics, lifetimes, lambdas, and prefixed and template literals. The corpus also pins every
bypass found in review.

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

Install over v1.3.0; the on-disk format is unchanged. Content that v1.3.0 already stored is not
re-redacted.

## Verification

- 324 workspace tests, line coverage 95.17%; the #29 property test was also run with 20,000 cases.
- `cargo clippy -D warnings`, `cargo doc -D warnings`, REUSE 3.3, gitleaks.
- Exact-commit CI on the merge commit; release assets checked against `SHA256SUMS` and build
  attestations.
