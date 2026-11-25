# ADR 0011: A structural redactor that hides values, not lines

Status: accepted, 2026-11-25 UTC, after owner review: rules 1 to 8 approved as drafted, including
the UUID session-id exemption (rule 6) and visible numbers with the password/PIN/OTP exceptions
(rule 2; the digit bound was later tightened from 16 to 10, see rule 2). Phase 7 Sprint 4 (T-74-002); implemented by T-74-003. Builds on the v1.3.1
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
   ending at whitespace, a `;` before whitespace or the end of the line, or, inside brackets
   opened before the key (a call, a flow mapping or sequence), also at `,` or a closing bracket
   followed by whitespace or more structure. A literal glued to more text (`"abc"def`, `"a"#b`,
   `"a",b`) is one shell word or scalar and takes rule 4. Otherwise the literal's contents are
   replaced by the redaction marker inside its own quotes, and the tail is kept when it holds no quote
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
6. **Agent session ids.** A `session_id` or `sessionId` key, or `<prefix>_session_id` whose
   prefix names no credential (not `cookie_`, `token_`, `password_`, `auth_`, ...), is not
   sensitive when its value is exactly a UUID (8-4-4-4-12 hex).
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

## Amendments after the red-team pass (2026-11-25)

Five rounds of automated review and an independent red-team pass on PR #103 found leak paths in
the first implementation and older gaps it exposed. By owner decision, the regressions and the
High groups are fixed in v1.5.0, and the remaining shapes are tracked for v1.5.1 (#104). The
changes, each with tests:

- **Rule 6, session ids:** a UUID stays only in a strictly parsed JSON member named exactly
  `session_id` or `sessionId`, as in agent hook payloads. Header, env and prefixed session ids
  stay hidden.
- **Rule 2, code references:** spaces around `=` alone are weak evidence, since INI and
  properties files have them too. On such a line only a constant path (`settings.SECRET_KEY`) is
  kept. A binding keyword, a type annotation or a trailing `;` is strong evidence and keeps calls
  and paths with identifier-only arguments.
- **Rule 3, the rest of the line:** after a literal, the rest stays only when it is a comment,
  structure, the next assignment (`BAR=2`, `user: bob`) or the next CLI flag. A bare word
  (`"a" b`, `'a' if x else b`) is part of the value to an INI parser, so it is hidden.
- **Value boundaries:** in config text (YAML, JSON-ish) `;` is part of a value. A harmless value
  followed by whitespace ends only at a comment, the end of the line, or a shell separator outside
  config text.
- **Key names:**
  - camelCase and PascalCase are split into words (`accessToken`, `APIKey`);
  - glued and qualified spellings match (`PGPASSWORD`, `SECRETKEY`, `password_value`,
    `api_key_value`, `API_KEY_2`), unless a following part names something that is not a secret
    (`token_type`, `password_file`, `secret_name`);
  - `Authorization`, a bare `auth`, `pw`, and a lower- or mixed-case `pwd` count (not the shell's
    `PWD`);
  - `Authorization: Bearer` / `Basic` keeps the scheme word and hides the token, whatever its
    length.
- **Unquoted values** run to the end of the line, not to the first space (passphrases, YAML plain
  scalars, INI values). They end before a comment, a shell `|`, `&&` or `;`, or the next query
  parameter. CLI flag values still end at whitespace.
- **Multi-line values:**
  - after `=`: an empty right-hand side, an unterminated quote, a triple quote or a backtick, a
    trailing backslash, and an unclosed bracket;
  - a YAML plain scalar continued on more-indented lines.
- **Kept by design and recorded in #104:**
  - text after a comment start;
  - values under keys that are not sensitive (`OTHER=` after a secret);
  - JSON `sessionId` UUIDs.

## Amendments in v1.5.1 (#104, 2026-11-25)

Issue #104 listed 82 inputs from the red-team pass that still leaked in v1.5.0, and in v1.4.1. The
owner chose the scope:

- **Prose:** only shell forms count as a key without `=` or `:`. Prose stays.
- **PIN and OTP:** `pin` and `otp` values are hidden when they are plain numbers of 4 to 10
  digits.
- **By design:** the cases above stay visible.

Added, each with a test in `crates/icx-codec/tests/redact_v151.rs`:

- **Markup and pairs:**
  - an XML element named by a sensitive key (`<password>…</password>`); a generic such as
    `Option<SecretString>` is not an element;
  - a tag's `value` attribute when its `key`, `name` or `id` attribute names a sensitive key;
  - JSON `name`/`key` plus `value` pairs;
  - Kubernetes-style YAML `name:` plus `value:` siblings, and list items and `value:` members
    under a sensitive block key. Other members keep their own per-key judgement.
- **Assignments without `=` between key and value:** subscripts (`obj["password"] = "…"`), `=>`
  pairs, and shell `set`/`set -x`/`setenv`. For subscripts and `=>`, only a quoted literal value
  is hidden and code on the right stays; a shell `set` value is hidden quoted or not, except a
  command or variable substitution.
- **CLI:**
  - a value that starts with `-` after a sensitive long flag;
  - `-p` only for `sshpass` and registry `login`, and glued `-pX` only for the MySQL and MariaDB
    clients (`-p` is a port almost everywhere else);
  - `curl -u user:pass`;
  - the input of `--password-stdin`;
  - JSON argv arrays.
- **URLs:** credential query and fragment parameters (signatures, OAuth `code`,
  `access_token`), and webhook keys, whose URL is the credential.
- **Unicode:** look-alikes are normalised to ASCII before redaction: no-break and other spaces,
  zero-width characters, the soft hyphen, and full-width forms. A line that redaction does not
  change keeps its original bytes.
- **Markers:** text glued to a marker at the start of a value is part of the value.
- **Multi-line values:**
  - escaped and interior triple quotes;
  - a quote at the start of the value;
  - prefixed and tagged literals;
  - unspaced and empty-RHS annotations;
  - a stray apostrophe before the `=`, handled by a quote-blind second scan, only when the
    line ends inside an apostrophe.

**Idempotence.** Line redaction runs to a fixed point, at most five rounds, because the line
passes pair quotes by position. Whether a line opens a multi-line value is decided on the
original line or else on the emitted one, so a quote that redaction removed cannot change the
decision on a second pass. One exception keeps this stable: a value that ends with one of the
value-span rule's own markers (`ENV_SECRET`, `CONFIG_SECRET`, `CLI_SECRET`) does not open a
multi-line value. That marker may have taken the value's closing bracket or quote with it. A
planted marker that copies one of those three kinds exactly is therefore a residual way to stop
a continuation. Any other planted marker, or text after one, still opens it.

**Kept by design**, with the reason for each:

- prose (`password reset flow`), and an INI `[password]` section header;
- `-p x` and `["-p", "x"]` with no password-taking tool named (usually a port);
- `--password=a b`, where the shell splits `b` into its own argument;
- a plain user name in a URL (`https://alice@host`); token-shaped userinfo is hidden;
- a bare `key` member;
- an XML element whose closing tag is on another line. Text without markup structure must stay
  byte-exact (ICX-AUD-021), and an opening tag alone is not that structure;
- a literal `\n` inside an unquoted value. In hook payloads this is almost always a JSON-escaped
  newline, so what follows it is the next command.

**Measured:**

- #104 inputs: 25 of 82 still leak, all of them in the by-design list.
- Repository corpus (588 secret-free files): no line outside `redact.rs`'s own examples is newly
  hidden.
- 19-line corpus: 43 over-redacted bytes, unchanged.

## Consequences

- The #90 lines keep their non-secret values, and a real secret costs only its own bytes, plus a
  quoted tail where one follows. Every shape in the #29 list still hides its literals, because
  each one reaches rule 4 or the quoted-tail fallback of rule 3.
- Rules 2 and 3 widen what survives redaction, so they are the security-relevant part of this
  change. The guard is that every kept span is either free of quotes or a literal that is itself
  replaced; the vendor, JWT and entropy detectors still run over all of it.
- Measured (T-74-003, 19-line corpus in `redact_structural_v150.rs`): no missed secret before or
  after; bytes hidden that were not part of a secret went from 183 to 24, and to 43 after the
  red-team amendments below (`docs/numbers.md`). Existing tests that
  pinned the old output shape (a literal hidden with its quotes and the rest of its line) now pin
  the new one; every assertion that a secret is absent is unchanged.
- Acceptance (T-74-003): every existing redaction test passes; the #29 bypass shapes
  and the #90 cases become a corpus; property tests generate secrets in every supported format
  and require that none survives; missed secrets (zero required) and over-redacted bytes are
  measured before and after and published in `docs/numbers.md`.
- Content stored by earlier versions is not changed by this; `icx rescan` (T-74-001) reports what
  the current redactor would hide in a store; re-redacting it is planned for v1.7.0 (#102), and
  nothing can restore what an earlier version removed.
- Rejected: a per-language parser (tree-sitter and similar). It would be exact for code, but
  icx sees mixed text (logs, diffs, shell output) where no grammar applies, and the fallback rules
  would still be needed. Rejected: trusting the "next field" boundary inside code (`, name=`),
  which is where most PR #27 bypasses lived; only strictly parsed JSON field lists split there.
