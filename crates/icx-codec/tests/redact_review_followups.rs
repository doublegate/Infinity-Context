//! Follow-ups from the reviews of PR #31 (#33), released in v1.3.2. Placeholders are
//! low-entropy on purpose (gitleaks).
use icx_codec::SecretRedactor;

fn once(text: &str) -> String {
    let clean = SecretRedactor::redact(text);
    assert_eq!(
        SecretRedactor::redact(&clean),
        clean,
        "not idempotent: {clean:?}"
    );
    clean
}

/// A dotted or spaced counter key is a count, as `max_tokens` is; a dotted sensitive key is not.
#[test]
fn dotted_counter_keys_are_not_secrets() {
    for line in [
        r#"{"max.tokens": 4096}"#,
        r#"{"usage": {"prompt tokens": 12}}"#,
    ] {
        assert_eq!(once(line), line);
    }
    let clean = once(r#"{"db.password": "aaaa", "secret.value": "aaaa"}"#);
    assert!(!clean.contains("aaaa"), "{clean}");
}

/// A multi-line quoted value leaves no unmatched closing quote behind.
#[test]
fn multi_line_quoted_values_leave_no_stray_quote() {
    let clean = once("password: \"aaaa\n  bbbb\"\npublic: ok\n");
    assert!(
        !clean.contains("aaaa") && !clean.contains("bbbb"),
        "{clean:?}"
    );
    assert!(clean.contains("public: ok"), "{clean:?}");
    assert_eq!(clean.matches('"').count(), 0, "{clean:?}");
}

/// A block header whose key was swallowed by an earlier value's marker is not glued onto it.
#[test]
fn swallowed_block_header_is_dropped() {
    let clean = once("\"api_key\": \"aaaa\", \"password\": |\n  bbbb\nnext: ok\n");
    assert!(
        !clean.contains("aaaa") && !clean.contains("bbbb"),
        "{clean:?}"
    );
    assert!(
        !clean.contains("]: |"),
        "header glued onto the marker: {clean:?}"
    );
    assert!(clean.contains("next: ok"), "{clean:?}");
}

/// CRLF lines inside a JSON string get the same digest and key-context treatment as LF lines.
#[test]
fn crlf_lines_inside_json_strings_match_lf_lines() {
    let sha = "e8ab57722d7d794fbffd585ad1499391b7707ea68f848aa9b20ad189605e26e0";
    let lf = once(&format!(r#"{{"stdout":"{sha}  file.tar.gz\nok"}}"#));
    let crlf = once(&format!(r#"{{"stdout":"{sha}  file.tar.gz\r\nok"}}"#));
    assert!(lf.contains(sha) && crlf.contains(sha), "{lf} / {crlf}");
    let hidden = once(&format!(r#"{{"stdout":"x\r\n{sha}\r\n"}}"#));
    assert!(!hidden.contains(sha), "{hidden}");
}

/// v1.3.2 field over-redactions: a vendor prefix inside a word is not a token start, and a
/// `-`-joined name made of words (project slugs, model names) is not a high-entropy secret.
#[test]
fn prefixes_inside_words_and_word_slugs_are_kept() {
    for line in [
        "https://www.autodesk-example.invalid/autodesk-products-overview/page\n",
        "foreign agent slug -home-user-Code-OSS-Public-Projects-StarGazer-ab12cd34\n",
        "model Llama-4-Maverick-17B-128E-Instruct-FP8 loaded\n",
        "Unit dev-disk-by-uuid-0f3a.device and task-runner-queue started\n",
    ] {
        assert_eq!(once(line), line);
    }
    // Tagged or mixed-case random segments are not words (review of #38).
    for line in [
        "opaque AbCxQ-zzZq-Kp7Lm2Nq8Rs4Tv6Wx0Yz3\n",
        "deploy myapp-prod-aB3dE5fG7hJ9kL1mN3pQ5rS7tU9vW1xY\n",
        "org-live-Kp7Lm2Nq8Rs4Tv6Wx0Yz3Ab9Cd5Ef1Gh\n",
    ] {
        assert!(once(line).contains("[REDACTED:"), "{line}");
    }
    // Real tokens are still caught, at a boundary or by the entropy gate.
    for line in [
        "key sk-proj-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n",
        "token aB3dE5fG7hJ9kL1mN3pQ5rS7tU9vW1xY-aB3dE5fG7hJ9-kL1mN3pQ5r\n",
        "x Ab9xQ-zzzq-Kp7Lm2Nq8Rs4Tv6Wx0Yz3\n",
        // Distinctive prefixes are redacted even when glued to a letter or digit (security
        // review of #38): a URL-encoded `=` and a literal escaped newline.
        "https://example.invalid/cb?t%3Dghp_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n",
        "{\"log\":\"x\\nAKIAAAAAAAAAAAAAAAAA\"}\n",
    ] {
        assert!(once(line).contains("[REDACTED:"), "{line}");
    }
}

/// v1.3.2 field over-redaction: a token count labelling a list is not a `tokens` key.
#[test]
fn token_counts_are_not_token_keys() {
    for line in [
        "512 tokens: Fable 5.2, Opus 5.5\n",
        "| 1,024 tokens: Sonnet 5.5 |\n",
    ] {
        assert_eq!(once(line), line);
    }
    for line in [
        "tokens: aaaa\n",
        "api tokens: aaaa\n",
        "x512 tokens: aaaa\n",
    ] {
        assert!(once(line).contains("[REDACTED:"), "{line}");
    }
}
