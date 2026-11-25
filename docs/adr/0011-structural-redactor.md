# ADR 0011: A structural redactor that hides values, not lines

Status: accepted, 2026-11-25 UTC, after owner review: rules 1 to 8 approved as drafted, including
the UUID session-id exemption (rule 6) and visible numbers with the password/PIN/OTP and 16-digit
exceptions (rule 2). Phase 7 Sprint 4 (T-74-002); implemented by T-74-003. Builds on the v1.3.1
decision in #29 (fixed in #31) and addresses #90.

## Context

`SecretRedactor::redact` runs before content enters the store, so whatever it removes is gone:
`deref` cannot bring it back. v1.3.1 (#29, #31) settled one trade-off by owner decision: after a
sensitive key, a value holding any quote is hidden from its first quote to the end of the line,
and only a call head such as `env::var(` stays visible. That closed every bypass the PR #27
reviews found (concatenation, ternaries, lambdas, chained calls, prefixed and template literals),
because each of them hides a second literal behind code the redactor cannot fully parse. The
same decision redacts complete JSON lines structurally.

The cost shows up in daily use (#90, and the field log of 2026-11-22). Probed on `main` at
24e44c9 with 17 representative lines, the redactor caught every real secret, and lost bytes it
should have kept on 10:

| Kind | Example (value shape) | Lost today |
| :--- | :--- | :--- |
| Non-secret value under a sensitive key | `token = ""`, `"api_key": null`, `token: true` | the value |
| Reference, no literal | `secret_key = settings.SECRET_KEY`, `session_token = get_token(user)` | the value |
| Environment lookup | `api_key = os.environ["API_KEY"]` | the variable name |
| Agent session id | `"session_id": "8f2c1e9a-…"` (a UUID) | the value |
| Rest of the line after a real secret | `password = "…"  # comment`, `"access_token": "…", "expires_in": 3599`, `'…' if prod else None` | comment, next field, tail |

Over-redaction has no security cost, but it makes stored code misleading, and it is permanent.

## Decision

Redaction keeps its tiers and its order (key blocks, structured vendor tokens, JWTs, URL
userinfo, CLI flags, sensitive assignments, high-entropy tokens). What changes is how much of a
line a sensitive assignment hides, through one rule: **hide exactly the value when the value is
proven harmless or is a single literal with a harmless tail; otherwise keep the #29 rule.**

1. **Find the value.** After a sensitive key and its operator (`=`, `:`, `:=`, `=>`, a JSON `:`),
   the value runs to the first unquoted, unbracketed terminator: `,`, `;`, a closing bracket of
   an enclosing pair, a comment start (`#` after whitespace, `//`), or the end of the line.
   Brackets and quotes are tracked; an unterminated quote or bracket means the value runs to the
   end of the line.
2. **Values that are never secrets** are left as they are, and the line continues:
   - `null`, `None`, `nil`, `undefined`, `true`, `false` (any case), and the empty strings `""`,
     `''`;
   - numbers, except under password, PIN, OTP, passcode and verification-code keys, and except
     numbers of more than 10 digits, which may be numeric tokens (the same bound as the counter
     rule; tightened from the 16 digits first drafted, because an existing test pins a 15-digit
     access token);
   - a variable reference with an upper-snake name holding an `_` (`$DB_PASSWORD`,
     `${GITHUB_TOKEN}`), so a password that merely starts with `$` (`$HUNTER2`) is not one;
   - in code only, a reference with no quote anywhere in the value: a call or subscript whose
     arguments are identifiers and simple punctuation and whose head is a dotted path, a
     snake_case name with an `_` or a camelCase name (`get_token(user)`), or a dotted path that
     ends in an upper-snake constant (`settings.SECRET_KEY`) or starts at a common code root
     (`self`, `config`, `settings`, ...). Code means a `let`/`const`/`var` binding or spaces
     around `=`; since INI and properties files also allow those spaces, a bare dotted value
     such as `my.dog.rex` stays a literal (security review of the first implementation);
   - an environment lookup whose only literal is an environment variable name in upper snake
     case, ending the value: `os.environ["API_KEY"]`, `os.getenv("API_KEY")`,
     `process.env["API_KEY"]`, `env::var("API_KEY")`, `ENV["API_KEY"]`, `System.getenv("…")`.
     Anything after it in the value (`.unwrap_or("…")`, `or "…"`) falls back to rule 4.
3. **A value that is one quoted literal** (with an optional prefix such as `b`, `r`, `f`, `u`),
   ending at whitespace, `;` or the end of the line, or in structured text (JSON, YAML, code)
   also at `,` or a closing bracket. A literal glued to more text (`"abc"def`, `"a"#b`) is one
   shell word and takes rule 4: the literal's contents are replaced by
   the redaction marker inside its own quotes, and the tail is kept when it holds no quote
   character (a comment, `if prod else None`, a closing bracket). A tail that holds a quote is
   hidden from its first quote to the end of the line, as in #29: a later literal on the same
   line may be a second secret.
4. **Everything else** (a value that is code holding a literal: concatenation, ternaries,
   lambdas, chained calls, template literals) keeps the #29 rule unchanged: hidden from its first
   quote to the end of the line, with a leading call head kept.
5. **Field lists are parsed, not guessed.** A line that is a sequence of JSON fields
   (`"k": v, "k2": v2`, with or without braces or a trailing comma) is redacted by the existing
   strict JSON scanner, so each sensitive field loses only its value. A line that does not parse
   strictly falls back to rules 1 to 4.
6. **Agent session ids.** A key whose only credential part is `session` (`session_id`,
   `sessionId`, `session`) is not sensitive when its value is exactly a UUID (8-4-4-4-12 hex).
   `session_token`, `session_secret`, `session_key`, `session_cookie` and non-UUID session values
   stay sensitive. Agent hook payloads carry such ids on every event; they identify a
   transcript, they do not authenticate anything.
7. **Formats covered** by rules 1 to 5: JSON lines and field lists; YAML `key: value` (block and
   flow); TOML `key = value`; env files (`KEY=value`, `export KEY=value`); shell assignments; code
   assignments and keyword arguments in Python, JavaScript/TypeScript, Rust, Go and similar
   (`let`, `const`, `var`, `:=`, `=>`). Multi-line values (YAML block scalars, key blocks,
   continued quotes) keep today's handling.
8. **Unknown text** keeps the value-independent detectors (vendor tokens, JWTs, URL userinfo,
   high entropy), which run over every line, including the parts rules 2 and 3 keep.

## Consequences

- The #90 lines keep their non-secret values, and a real secret costs only its own bytes, plus a
  quoted tail where one follows. Every shape in the #29 list still hides its literals, because
  each one reaches rule 4 or the quoted-tail fallback of rule 3.
- Rules 2 and 3 widen what survives redaction, so they are the security-relevant part of this
  change. The guard is that every kept span is either free of quotes or a literal that is itself
  replaced; the vendor, JWT and entropy detectors still run over all of it.
- Measured (T-74-003, 19-line corpus in `redact_structural_v150.rs`): no missed secret before or
  after; bytes hidden that were not part of a secret went from 183 to 24. Existing tests that
  pinned the old output shape (a literal hidden with its quotes and the rest of its line) now pin
  the new one; every assertion that a secret is absent is unchanged.
- Acceptance (T-74-003): every existing redaction test passes; the #29 bypass shapes
  and the #90 cases become a corpus; property tests generate secrets in every supported format
  and require that none survives; missed secrets (zero required) and over-redacted bytes are
  measured before and after and published in `docs/numbers.md`.
- Content stored by earlier versions is not changed by this; `icx rescan` (T-74-001) reports what
  the current redactor would hide in a store, and can re-redact it, but cannot restore what an
  earlier version removed.
- Rejected: a per-language parser (tree-sitter and similar). It would be exact for code, but
  icx sees mixed text (logs, diffs, shell output) where no grammar applies, and the fallback rules
  would still be needed. Rejected: trusting the "next field" boundary inside code (`, name=`),
  which is where most PR #27 bypasses lived; only strictly parsed JSON field lists split there.
