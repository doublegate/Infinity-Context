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

/// Review of #27: typed `SecretStr`/`SecretBytes` fields (dotted too), unions led by a null type,
/// keep their literal redacted (closures and other code-shaped values: #29). A dotted path
/// ending in a generic type name is a value, not a type.
#[test]
fn secret_type_annotations_null_led_unions_and_closures() {
    for (line, secret) in [
        ("api_key: SecretStr = \"aaaa\"\n", "aaaa"),
        ("secret: SecretBytes = \"aaaa\"\n", "aaaa"),
        ("api_key: pydantic.SecretStr = \"aaaa\"\n", "aaaa"),
        ("password: None | str = \"aaaa\"\n", "aaaa"),
        ("token: null | string = 'aaaa';\n", "aaaa"),
        ("api_key: str | pydantic.SecretStr = \"aaaa\"\n", "aaaa"),
        ("api_key: sk_live_bbbbcccc.str = 1\n", "bbbbcccc"),
    ] {
        let clean = SecretRedactor::redact(line);
        assert!(!clean.contains(secret), "{line:?} -> {clean:?}");
    }
    let code = "    api_key: pydantic.SecretStr,\n";
    assert_eq!(SecretRedactor::redact(code), code);
}

/// Review of #27 (01:24 UTC): exact markers inside a call are left alone (idempotent); text that
/// only starts like a marker is still redacted.
#[test]
fn call_wrapped_markers_are_idempotent() {
    let once = SecretRedactor::redact("api_key = SecretStr(\"aaaa\")\n");
    assert!(!once.contains("aaaa"), "{once:?}");
    assert_eq!(SecretRedactor::redact(&once), once);
    let fake = SecretRedactor::redact("api_key = Foo(\"[REDACTED:x] aaaa\")\n");
    assert!(!fake.contains("aaaa"), "{fake:?}");
}
