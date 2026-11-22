//! v1.3.1 (#29): after a sensitive key, everything from the value's first quote to the end of
//! the line is one redaction marker; only a call head right before that quote (`env::var(`) stays
//! visible, and any other value is hidden from its start. No quote pairing, no field boundaries,
//! no key detection (owner decisions, PR #31). Placeholders are low-entropy on purpose (gitleaks).
use icx_codec::SecretRedactor;

const S: &str = "aaaa";

fn hidden(line: &str) -> String {
    let clean = SecretRedactor::redact(line);
    assert!(!clean.contains(S), "{line:?} -> {clean:?}");
    assert_eq!(
        SecretRedactor::redact(&clean),
        clean,
        "not idempotent: {clean:?}"
    );
    clean
}

/// Every shape listed on #29.
#[test]
fn reported_code_shapes_hide_their_literals() {
    for line in [
        "password = env::var(\"K\").unwrap_or(\"aaaa\")\n",
        "api_key = os.getenv(\"K\") or SecretStr(\"aaaa\")\n",
        "api_key = settings.key or \"aaaa\"\n",
        "const token = config.token ?? \"aaaa\";\n",
        "api_key = base + \"aaaa\"\n",
        "token = c ? \"aaaa\" : \"aaaa\";\n",
        "token = cond ? x: \"aaaa\"\n",
        "password = x if y else \"aaaa\"\n",
        "secret = b\"aaaa\"\n",
        "api_key = f\"prefix_{x}_aaaa\"\n",
        "api_key = r#\"aaaa\"#\n",
        "const password = `prefix_${x}_aaaa`;\n",
        "password = new Secret(\"aaaa\")\n",
        "token = await get_token() ?? \"aaaa\"\n",
        "password = not disabled and \"aaaa\"\n",
        "token = !c ? \"aaaa\" : \"aaaa\";\n",
        "password = lambda x: \"aaaa\"\n",
        "password = parse::<&str, String>(\"aaaa\")\n",
        "password = f::<&'a str>('aaaa')\n",
        "password = f::<&'a str>('v1 --aaaa')\n",
        "api_key = \"prefix_\" + \"aaaa\"\n",
        "password = \"{}\".format(\"aaaa\")\n",
        "api_key = \"prefix_\" \"aaaa\"\n",
        "password = SecretStr(x, user=\"a\", \"aaaa\")\n",
        "password: my_secret(1) # don't aaaa\n",
    ] {
        hidden(line);
    }
}

/// A call head right before the first quote stays visible; the rest of the line is one marker.
#[test]
fn call_heads_stay_visible() {
    let c = hidden("password = env::var(\"DB_PASSWORD\").unwrap_or(\"aaaa\")\n");
    assert_eq!(c, "password = env::var([REDACTED:ENV_SECRET]\n", "{c:?}");
    let c = hidden("api_key = os.getenv(\"K\") or SecretStr(\"aaaa\")\n");
    assert_eq!(c, "api_key = os.getenv([REDACTED:ENV_SECRET]\n", "{c:?}");
}

/// No stopping points (owner decision): the rest of the line is hidden with the value, so nothing
/// can slip past a guessed field boundary. The bypasses found against earlier stopping points are
/// pinned here.
#[test]
fn later_literals_on_the_line_are_hidden_too() {
    // A complete JSON line is redacted structurally instead (redact_json_lines_v131.rs); a line
    // that only starts like JSON keeps the first-quote rule.
    let c = hidden("{\"password\": \"aaaa\", \"user\": \"aaaa\"\n");
    assert_eq!(c, "{\"password\": [REDACTED:CONFIG_SECRET]\n", "{c:?}");
    hidden("password: SecretStr(\"aaaa\"), user: \"aaaa\"\n");
    hidden("curl --api-key=\"aaaa\" --user \"aaaa\"\n");
    // Bypasses of the stopping points tried before the owner chose the strict rule.
    hidden("password = f(x)) + \"aaaa\"\n");
    hidden("password = {\"aaaa\": 1}\n");
    hidden("api_key = x --  \"aaaa\"\n");
    hidden("password = f(x), user: \"aaaa\"\n");
    hidden("token = [a], \"b\": \"aaaa\"\n");
}

/// Ambiguity fails closed: everything after an ambiguous quote on the line is hidden.
#[test]
fn ambiguous_quotes_fail_closed() {
    hidden("password = f(&'a x) + \"aaaa\" --flag \"aaaa\"\n");
    hidden("token = \"unterminated aaaa\n");
}

/// Lines without a sensitive key are untouched.
#[test]
fn unrelated_lines_are_untouched() {
    for line in [
        "let name = env::var(\"HOME\").unwrap_or(\"x\");\n",
        "user = \"admin\" + \"suffix\"\n",
    ] {
        assert_eq!(SecretRedactor::redact(line), line);
    }
}

mod properties {
    use icx_codec::SecretRedactor;
    use proptest::prelude::*;

    /// A quoted literal whose contents are a recognisable placeholder.
    fn literal() -> impl Strategy<Value = String> {
        (prop_oneof![Just('"'), Just('`'), Just('\'')], "[a-z]{0,6}")
            .prop_map(|(q, pad)| format!("{q}qqzz{pad}{q}"))
    }

    /// One balanced unit of code: identifiers, operators, calls, chains, indexes, generics,
    /// prefixed literals and plain literals.
    fn unit() -> impl Strategy<Value = String> {
        prop_oneof![
            literal(),
            literal().prop_map(|l| format!("f({l})")),
            literal().prop_map(|l| format!("env::var(x, {l})")),
            literal().prop_map(|l| format!(".unwrap_or({l})")),
            literal().prop_map(|l| format!("[{l}]")),
            literal().prop_map(|l| format!("parse::<A, B>({l})")),
            literal().prop_map(|l| format!("b{l}")),
            literal().prop_map(|l| format!("f{l}")),
            Just("x".to_owned()),
            Just("cfg.key".to_owned()),
            Just(" or ".to_owned()),
            Just(" ?? ".to_owned()),
            Just(" + ".to_owned()),
            Just(" ? ".to_owned()),
            Just(" : ".to_owned()),
            Just(" if y else ".to_owned()),
            Just("!".to_owned()),
            Just("lambda v: ".to_owned()),
            Just("new ".to_owned()),
            Just("&'a ".to_owned()),
        ]
    }

    proptest! {
        /// No literal under a sensitive key keeps its contents, whatever code surrounds it.
        #[test]
        fn no_literal_survives_after_a_sensitive_key(
            key in prop_oneof![Just("password"), Just("api_key"), Just("token"), Just("secret")],
            sep in prop_oneof![Just(" = "), Just("="), Just(": ")],
            units in proptest::collection::vec(unit(), 1..8),
        ) {
            let line = format!("{key}{sep}{}\n", units.concat());
            let clean = SecretRedactor::redact(&line);
            prop_assert!(!clean.contains("qqzz"), "{:?} -> {:?}", line, clean);
            prop_assert_eq!(SecretRedactor::redact(&clean), clean);
        }
    }
}

/// Review of #31 (owner decision): hide from the value's first quote to the end of the line, so
/// quote pairing and key detection cannot leak anything. Only a call head stays visible.
#[test]
fn first_quote_to_end_of_line() {
    // Leaks found in review of the literal-by-literal rule.
    hidden("password = a\"b + \"aaaa\"\n");
    hidden("password = a\"b + \"aaaa\" + c\"d\n");
    hidden("{\"api_key\": \"x\", \"password\": aaaa}\n");
    hidden("{\"api_key\": \"x\", \"tokens\": [aaaa, 67890]}\n");
    hidden("password = \\\"x\\\" + \\\"aaaa\\\"\n");
    // Codex P1 on #31: a later quoted sensitive key with an unquoted value (JSON and YAML flow).
    hidden("{\"password\":\"x\",\"token\":aaaa}\n");
    hidden("{password: x, \"api_key\": aaaa}\n");
    // A call head stays visible; everything from the first quote is one marker.
    let c = hidden("password = env::var(\"K\").unwrap_or(\"aaaa\")\n");
    assert_eq!(c, "password = env::var([REDACTED:ENV_SECRET]\n", "{c:?}");
    // Any other head could itself be the secret: hidden from the key's value onward.
    let c = hidden("password=hunter2aaaa \"note\"\n");
    assert_eq!(c, "password=[REDACTED:ENV_SECRET]\n", "{c:?}");
    let c = hidden("api_key = settings.key or \"aaaa\"\n");
    assert_eq!(c, "api_key = [REDACTED:ENV_SECRET]\n", "{c:?}");
}

/// Review of #31 (04:54 UTC, blocking 2): no exemption (null default, type annotation, code
/// expression, token counter) skips the first-quote rule when a quote follows the key.
#[test]
fn exemptions_never_skip_a_later_quote() {
    for line in [
        "password: str = None or \"aaaa\"\n",
        "token: string = null ?? \"aaaa\"\n",
        "api_key: Optional[str] = None if prod else \"aaaa\"\n",
        "secret: str = False or \"aaaa\"\n",
        "password: str, \"aaaa\"\n",
        "{password: string, \"user\": \"aaaa\"}\n",
        "password: String, // \"aaaa\"\n",
        "let password = get_pw(); // \"aaaa\"\n",
        "password: str = get_pw(); # \"aaaa\"\n",
        "tokens = 0 or \"aaaa\"\n",
        "tokens = \"123\" + \"aaaa\"\n",
    ] {
        hidden(line);
    }
    // Without a quote the exemptions still apply.
    for line in [
        "max_tokens: 4096\n",
        "token: String,\n",
        "let password = get_pw();\n",
        "password: Optional[str] = None\n",
    ] {
        assert_eq!(icx_codec::SecretRedactor::redact(line), line);
    }
}
