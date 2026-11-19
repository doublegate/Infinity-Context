//! v1.1.0 redactor recall and losslessness (ICX-AUD-015, ICX-AUD-016, ICX-AUD-021).
//! Fixtures are synthetic, non-functional credential-shaped strings.

use icx_codec::SecretRedactor;

fn assert_redacted(input: &str, secret: &str) {
    let out = SecretRedactor::redact(input);
    assert!(
        !out.contains(secret),
        "leaked `{secret}`:\n in: {input:?}\nout: {out:?}"
    );
    assert!(out.contains("[REDACTED:"), "no redaction marker: {out:?}");
}

#[test]
fn url_userinfo_passwords_are_redacted_for_every_scheme() {
    for url in [
        "git clone https://alice:S3cr3tPassw0rdX@github.com/org/repo.git",
        "curl http://bob:Hunter2Hunter2@internal:8080/x",
        "ftp://carol:Ftp!Passw0rd@files.example/",
        "ssh://dave:SshPassw0rd99@host/",
        "postgres://app:PgPassw0rd77@db:5432/app",
    ] {
        let secret = url.split(':').nth(2).unwrap().split('@').next().unwrap();
        assert_redacted(url, secret);
        let out = SecretRedactor::redact(url);
        assert!(out.contains("://"), "scheme kept: {out}");
    }
    let plain = "see https://example.com/a:b@c for docs";
    assert_eq!(
        SecretRedactor::redact(plain),
        plain,
        "no userinfo, nothing to redact"
    );
}

/// Joins fixture fragments at runtime so no complete credential-shaped literal appears in the
/// source (secret scanners such as GitHub push protection flag even synthetic tokens).
fn fx(parts: &[&str]) -> String {
    parts.concat()
}

#[test]
fn additional_vendor_token_formats_are_redacted() {
    let hex32 = "0123456789abcdef0123456789abcdef";
    let cases: Vec<(String, String)> = vec![
        (
            fx(&[
                "SENDGRID=",
                "SG",
                ".ngeVfQFYQlKU0ufo8x5d1A",
                ".TwL2iGABf9DHoTf-09kqeF8tAmbihYzrnopKc-1s5cr",
            ]),
            "ngeVfQFYQlKU0ufo8x5d1A".into(),
        ),
        (
            fx(&["token ", "glpat", "-xY3kP9qLm2Nw8RtVz4Ab used"]),
            "xY3kP9qLm2Nw8RtVz4Ab".into(),
        ),
        (
            fx(&[
                "app=",
                "xapp",
                "-1-A0123456789-0123456789012-abcdef0123456789abcdef",
            ]),
            "abcdef0123456789abcdef".into(),
        ),
        (
            fx(&["do=", "dop_v1", "_", hex32, hex32]),
            "0123456789abcdef0123456789abcdef0123".into(),
        ),
        (fx(&["shop=", "shpat", "_", hex32]), hex32.into()),
        (
            fx(&[
                "hook https://hooks.slack.com/services/",
                "T0000000/B0000000/",
                "XXXXXXXXXXXXXXXXXXXXXXXX",
            ]),
            "XXXXXXXXXXXXXXXXXXXXXXXX".into(),
        ),
        (fx(&["twilio ", "SK", hex32]), hex32.into()),
    ];
    for (input, secret) in &cases {
        assert_redacted(input, secret);
    }
}

#[test]
fn additional_sensitive_key_names_are_redacted() {
    for (input, secret) in [
        (
            "ENCRYPTION_KEY=plainvalue-not-entropic",
            "plainvalue-not-entropic",
        ),
        (
            "SIGNING_KEY: plainvalue-not-entropic",
            "plainvalue-not-entropic",
        ),
        ("DB_PASS=correcthorse", "correcthorse"),
        (
            "\"passphrase\": \"correct horse battery\"",
            "correct horse battery",
        ),
        ("SENTRY_DSN=https://abc@o1.ingest.sentry.io/1", "abc@o1"),
        ("cookie: session=abcdef", "session=abcdef"),
    ] {
        assert_redacted(input, secret);
    }
    // Common non-secret look-alikes stay intact.
    for benign in [
        "PWD=/home/user/project",
        "cache_key: build-v2",
        "KEY_COUNT=3",
    ] {
        assert_eq!(SecretRedactor::redact(benign), benign);
    }
}

#[test]
fn multi_line_json_and_yaml_values_are_redacted() {
    let json = "{\n  \"password\":\n    \"hunter2-correct-horse\",\n  \"user\": \"alice\"\n}\n";
    let out = SecretRedactor::redact(json);
    assert!(!out.contains("hunter2-correct-horse"), "{out}");
    assert!(out.contains("\"user\": \"alice\""), "{out}");
    let yaml = "db:\n  password:\n    s3cret-value-here\n  host: x\n";
    let out = SecretRedactor::redact(yaml);
    assert!(!out.contains("s3cret-value-here"), "{out}");
    assert!(out.contains("host: x"), "{out}");
}

#[test]
fn single_line_and_unterminated_pem_blocks_do_not_swallow_the_rest() {
    let json_pem = fx(&[
        "{\"key\":\"-----BEGIN RSA PRIVATE ",
        "KEY-----\\nMIIEabc\\n-----END RSA PRIVATE ",
        "KEY-----\"}\nline 2 keep\nerror: E0308 keep\n",
    ]);
    let out = SecretRedactor::redact(&json_pem);
    assert!(!out.contains("MIIEabc"), "{out}");
    assert!(
        out.contains("line 2 keep") && out.contains("error: E0308 keep"),
        "{out}"
    );

    let mut unterminated = fx(&["-----BEGIN OPENSSH PRIVATE ", "KEY-----\n"]);
    for _ in 0..300 {
        unterminated.push_str("AAAAB3NzaC1yc2EAAAADAQABAAABAQ\n");
    }
    unterminated.push_str("after the runaway block\n");
    let out = SecretRedactor::redact(&unterminated);
    assert!(
        out.contains("after the runaway block"),
        "bounded block swallow"
    );
}

#[test]
fn redaction_is_lossless_for_secret_free_input() {
    for input in [
        "a\r\nb\r\n",
        "tail\n",
        "no trailing newline",
        "x ⟪y⟫ and ⟦z⟧ literal <<w>>",
        "\n\n\r\n",
        "",
    ] {
        assert_eq!(SecretRedactor::redact(input), input, "{input:?}");
    }
    // Model-facing sanitize still escapes delimiters (Tier 0).
    assert_eq!(SecretRedactor::sanitize("x ⟪y⟫\r\n"), "x <<y>>\r\n");
}
