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
