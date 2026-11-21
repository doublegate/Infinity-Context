//! v1.2.1: source-code type annotations under credential-like names are not secrets.
use icx_codec::SecretRedactor;

#[test]
fn type_annotations_are_not_redacted() {
    for code in [
        "let tokens: Vec<String> = f();\n",
        "let token: String = String::new();\n",
        "api_keys: HashMap<String, Key>,\n",
        "fn f(password: &'a str) -> bool {\n",
        "    secret: Option<SecretString>,\n",
        "def login(password: str) -> None:\n",
        "function f(token: string) {\n",
    ] {
        assert_eq!(SecretRedactor::redact(code), code, "redacted: {code:?}");
    }
}

#[test]
fn values_that_only_start_like_a_type_are_still_secrets() {
    for (line, secret) in [
        ("password: Stringent-lowsecret\n", "lowsecret"),
        ("password: \"String\"\n", "String"),
    ] {
        let clean = SecretRedactor::redact(line);
        assert!(!clean.contains(secret), "{line:?} -> {clean:?}");
    }
}

#[test]
fn numeric_token_counters_and_code_bindings_are_not_secrets() {
    for code in [
        "epoch_rollover_tokens: 16384\n",
        "            epoch_rollover_tokens: 16_384,\n",
        "  \"epoch_rollover_tokens\": 8192,\n",
        "let q_tokens = Self::query_tokens(query_text);\n",
        "let mut before_pair_tokens = estimate(&pair);\n",
        "const MAX_TOKENS = 4096;\n",
        "var api_token = getToken();\n",
    ] {
        assert_eq!(SecretRedactor::redact(code), code, "redacted: {code:?}");
    }
    // Shell/env assignments and non-numeric values keep the key rules.
    for (line, secret) in [
        ("API_TOKEN=lowsecretvalue\n", "lowsecretvalue"),
        ("export GITHUB_TOKENS=lowsecretvalue\n", "lowsecretvalue"),
        ("epoch_rollover_tokens: lowsecretvalue\n", "lowsecretvalue"),
    ] {
        let clean = SecretRedactor::redact(line);
        assert!(!clean.contains(secret), "{line:?} -> {clean:?}");
    }
}

#[test]
fn literals_after_exempted_code_are_still_redacted() {
    for (line, secret) in [
        (
            "let token: String = \"lowsecretliteral\";\n",
            "lowsecretliteral",
        ),
        (
            "let api_key: &str = 'lowsecretliteral';\n",
            "lowsecretliteral",
        ),
        ("let api_key = \"lowsecretliteral\";\n", "lowsecretliteral"),
        (
            "const AUTH_TOKEN = 'lowsecretliteral';\n",
            "lowsecretliteral",
        ),
        ("var password = lowsecretliteral\n", "lowsecretliteral"),
        ("MAX_AUTH_TOKEN=lowsecretliteral\n", "lowsecretliteral"),
        ("max_session_token: lowsecretliteral\n", "lowsecretliteral"),
        ("ACCESS_TOKENS=123456789012345\n", "123456789012345"),
        ("auth_tokens: 4096\n", "4096"),
    ] {
        let clean = SecretRedactor::redact(line);
        assert!(!clean.contains(secret), "{line:?} -> {clean:?}");
    }
}
