//! v1.2.2 (issue #24): nested call wrappers and Python/Rust annotation forms.
use icx_codec::SecretRedactor;

#[test]
fn literals_inside_nested_call_wrappers_are_redacted() {
    for (line, secret) in [
        (
            "let api_key = Some(String::from(\"lowsecretliteral\"));\n",
            "lowsecretliteral",
        ),
        (
            "password: Box::new(Secret::new('lowsecretliteral')),\n",
            "lowsecretliteral",
        ),
        (
            "token: Arc::new(Some(String::from(\"lowsecretliteral\")))\n",
            "lowsecretliteral",
        ),
    ] {
        let clean = SecretRedactor::redact(line);
        assert!(!clean.contains(secret), "{line:?} -> {clean:?}");
    }
}

#[test]
fn python_optional_and_rust_mut_lifetime_annotations_are_not_secrets() {
    for code in [
        "def login(password: Optional[str] = None) -> None:\n",
        "    token: Optional[str],\n",
        "fn f(secret: &'a mut String) {\n",
        "    api_keys: list[str] = field(default_factory=list)\n",
    ] {
        assert_eq!(SecretRedactor::redact(code), code, "redacted: {code:?}");
    }
    // A literal default after a Python annotation is still a secret.
    let clean = SecretRedactor::redact("password: Optional[str] = \"lowsecretliteral\"\n");
    assert!(!clean.contains("lowsecretliteral"), "{clean:?}");
}
