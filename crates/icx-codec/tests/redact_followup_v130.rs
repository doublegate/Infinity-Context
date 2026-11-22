//! v1.3.0: Antigravity review of #25 — dotted and keyword call wrappers, union type annotations.
use icx_codec::SecretRedactor;

#[test]
fn literals_in_dotted_or_keyword_call_wrappers_are_redacted() {
    for (line, secret) in [
        (
            "api_key = pydantic.SecretStr(\"lowsecretliteral\")\n",
            "lowsecretliteral",
        ),
        (
            "password: Secret.new('lowsecretliteral'),\n",
            "lowsecretliteral",
        ),
        (
            "password: str = field(default=\"lowsecretliteral\")\n",
            "lowsecretliteral",
        ),
        (
            "token = unwrap_or(x, \"lowsecretliteral\")\n",
            "lowsecretliteral",
        ),
    ] {
        let clean = SecretRedactor::redact(line);
        assert!(!clean.contains(secret), "{line:?} -> {clean:?}");
    }
}

#[test]
fn union_type_annotations_keep_their_defaults_redacted() {
    for (line, secret) in [
        (
            "password: str | None = \"lowsecretliteral\"\n",
            "lowsecretliteral",
        ),
        (
            "token: string | null = 'lowsecretliteral';\n",
            "lowsecretliteral",
        ),
    ] {
        let clean = SecretRedactor::redact(line);
        assert!(!clean.contains(secret), "{line:?} -> {clean:?}");
    }
    for code in [
        "    password: str | None = None,\n",
        "function f(token: string | undefined) {\n",
    ] {
        assert_eq!(SecretRedactor::redact(code), code, "redacted: {code:?}");
    }
}

/// Review of #27 (round 4, blocking): literals in chained calls and unions that start with a null
/// type stay redacted.
#[test]
fn chained_calls_and_null_led_unions_keep_literals_redacted() {
    for (line, secret) in [
        (
            "password = env::var(\"DB_PASSWORD\").unwrap_or(\"lowsecretliteral\")\n",
            "lowsecretliteral",
        ),
        (
            "api_key = os.getenv(\"KEY\").strip() or SecretStr(\"lowsecretliteral\")\n",
            "lowsecretliteral",
        ),
        (
            "api_key = client.auth().set_secret(\"lowsecretliteral\")\n",
            "lowsecretliteral",
        ),
        (
            "password: None | str = \"lowsecretliteral\"\n",
            "lowsecretliteral",
        ),
        (
            "token: null | string = 'lowsecretliteral';\n",
            "lowsecretliteral",
        ),
    ] {
        let clean = SecretRedactor::redact(line);
        assert!(!clean.contains(secret), "{line:?} -> {clean:?}");
    }
}

/// Review of #27 (round 4): lifetimes are not literals, empty literals and existing markers are
/// left alone, and redaction is idempotent.
#[test]
fn lifetimes_empty_literals_and_markers_survive_call_scanning() {
    let code = "    let token = parse::<&'a str>(buf);\n";
    assert_eq!(SecretRedactor::redact(code), code);
    for line in [
        "api_key = SecretStr(\"lowsecretliteral\")\n",
        "auth: Any = Client(api_key=\"lowsecretliteral\")\n",
    ] {
        let once = SecretRedactor::redact(line);
        assert!(!once.contains("lowsecretliteral"), "{once:?}");
        assert_eq!(
            SecretRedactor::redact(&once),
            once,
            "not idempotent: {once:?}"
        );
    }
}

/// Security review of 6e9cfd1: a quoted literal right after `&`/`<` is still a literal unless it
/// is a real lifetime, and only exact redactor markers count as already redacted.
#[test]
fn fake_lifetimes_and_fake_markers_do_not_hide_secrets() {
    for (line, secret) in [
        ("api_key = f(&'lowsecretliteral')\n", "lowsecretliteral"),
        ("api_key = f(<'lowsecretliteral'>)\n", "lowsecretliteral"),
        (
            "api_key = f(&'my-lowsecretliteral.value')\n",
            "lowsecretliteral",
        ),
        ("api_key = f(&'abc lowsecretliteral')\n", "lowsecretliteral"),
        (
            "api_key = Foo(\"[REDACTED:x] lowsecretliteral\")\n",
            "lowsecretliteral",
        ),
        (
            "api_key = Foo(\"[REDACTED:CONFIG_SECRET]lowsecretliteral\")\n",
            "lowsecretliteral",
        ),
    ] {
        let clean = SecretRedactor::redact(line);
        assert!(!clean.contains(secret), "{line:?} -> {clean:?}");
    }
    // A call holding only empty literals is not treated as handled: the plain value rule runs.
    let r = SecretRedactor::redact("api_key = SecretStr(\"\")\n");
    assert!(r.contains("[REDACTED:"), "{r:?}");
}

/// Review of #27 (23:25 UTC, blocking): typed `SecretStr` fields and code expressions that are
/// not led by a plain call keep every literal redacted; trailing comments end the expression.
#[test]
fn secret_types_and_non_call_code_values_keep_literals_redacted() {
    for (line, secret) in [
        (
            "api_key: SecretStr = SecretStr(\"lowsecretliteral\")\n",
            "lowsecretliteral",
        ),
        (
            "api_key: SecretStr | None = \"lowsecretliteral\"\n",
            "lowsecretliteral",
        ),
        (
            "api_key: pydantic.SecretStr = pydantic.SecretStr(\"lowsecretliteral\")\n",
            "lowsecretliteral",
        ),
        (
            "password = env::var::<String>(\"K\").unwrap_or(\"lowsecretliteral\")\n",
            "lowsecretliteral",
        ),
        (
            "api_key = os.environ[\"KEY\"] or SecretStr(\"lowsecretliteral\")\n",
            "lowsecretliteral",
        ),
        (
            "api_key = settings.api_key or \"lowsecretliteral\"\n",
            "lowsecretliteral",
        ),
        (
            "const token = config.token ?? \"lowsecretliteral\";\n",
            "lowsecretliteral",
        ),
    ] {
        let clean = SecretRedactor::redact(line);
        assert!(!clean.contains(secret), "{line:?} -> {clean:?}");
    }
    // Fail closed: `#` / `//` may be operators, so scanning continues to the end of the line.
    let c = SecretRedactor::redact("api_key = get_key(\"x\") // 2 or \"lowsecretliteral\"\n");
    assert!(!c.contains("lowsecretliteral"), "{c:?}");
    // A plain YAML value with a later quoted word still has its bare value hidden.
    let y = SecretRedactor::redact("password: aaaabbbbcccc 'note'\n");
    assert!(!y.contains("aaaabbbbcccc"), "{y:?}");
}

/// Security review of 8b1d37f: heuristics fail closed.
#[test]
fn bare_heads_and_fake_dotted_types_do_not_escape() {
    for (line, secret) in [
        // A dotted bare value next to a quoted word: the bare token is still hidden.
        ("password: aaaabbbbcccc.prod \"note\"\n", "aaaabbbbcccc"),
        ("api_key = ddddeeeeffff.x or \"fallback\"\n", "ddddeeeeffff"),
        // A dotted path ending in a generic type name is not a type annotation.
        (
            "api_key: sk_live_lowsecretliteral.str = 1\n",
            "lowsecretliteral",
        ),
        ("password: Aaaabbbbcccc.SecretStr\n", "Aaaabbbbcccc"),
    ] {
        let clean = SecretRedactor::redact(line);
        assert!(!clean.contains(secret), "{line:?} -> {clean:?}");
    }
    // Real dotted Secret types are still annotations.
    let code = "    api_key: pydantic.SecretStr,\n";
    assert_eq!(SecretRedactor::redact(code), code);
}

/// Review of #27 (23:40 UTC, blocking): a dotted bare value is not code, so later assignments on
/// the line are still found; dotted Secret types work in any union position.
#[test]
fn dotted_bare_values_and_dotted_union_members() {
    for (line, secrets) in [
        (
            "password=aaaa.bbbb user=\"admin\" token=cccccccc\n",
            &["aaaa.bbbb", "cccccccc"][..],
        ),
        (
            "API_KEY=v1.lowsecret curl \"https://example.com\"\n",
            &["v1.lowsecret"][..],
        ),
        (
            "api_key: None | pydantic.SecretStr = \"lowsecretliteral\"\n",
            &["lowsecretliteral"][..],
        ),
        (
            "api_key: str | pydantic.SecretStr = \"lowsecretliteral\"\n",
            &["lowsecretliteral"][..],
        ),
    ] {
        let clean = SecretRedactor::redact(line);
        for s in secrets {
            assert!(!clean.contains(s), "{line:?} -> {clean:?}");
        }
    }
}

/// Review of #27 (23:59 UTC, blocking): a code-shaped value ends at whitespace unless an operator
/// continues it, so later assignments on the line are still found; generics with commas work.
#[test]
fn code_values_end_at_the_expression_and_generics_nest() {
    for (line, secrets) in [
        (
            "password=SecretStr(\"aaaa\") token=bbbbbbbb\n",
            &["aaaa", "bbbbbbbb"][..],
        ),
        (
            "--api-key SecretStr(\"aaaa\") --password bbbbbbbb\n",
            &["aaaa", "bbbbbbbb"][..],
        ),
        (
            "password=my_secret(1) user=\"admin\" token=bbbbbbbb\n",
            &["my_secret(1)", "bbbbbbbb"][..],
        ),
        (
            "password = parse::<&str, String>(\"aaaa\")\n",
            &["aaaa"][..],
        ),
        ("password: cccc<x> \"note\"\n", &["cccc"][..]),
        ("api_key = settings.key\tor\t\"aaaa\"\n", &["aaaa"][..]),
    ] {
        let clean = SecretRedactor::redact(line);
        for s in secrets {
            assert!(!clean.contains(s), "{line:?} -> {clean:?}");
        }
    }
    // Non-sensitive fields after the value are left as they were.
    let r = SecretRedactor::redact("password=SecretStr(\"aaaa\") user=\"admin\"\n");
    assert!(r.contains("user=\"admin\""), "{r:?}");
}
