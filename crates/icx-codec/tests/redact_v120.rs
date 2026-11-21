//! Regression fixtures from ICX-CODEX-001/006; all credentials are synthetic.

use icx_codec::SecretRedactor;

#[test]
fn yaml_block_scalar_credentials_are_removed_without_consuming_siblings() {
    for header in ["|", "|-", "|+", "|2-", ">", ">-", ">+", ">2"] {
        let input = format!(
            "db:\r\n  password: {header}\r\n    lowentropyfirst\r\n    secondsecretline\r\n\r\n  host: localhost\r\npublic: ok\r\n"
        );
        let clean = SecretRedactor::redact(&input);
        assert!(!clean.contains("lowentropyfirst"), "{header}: {clean:?}");
        assert!(!clean.contains("secondsecretline"), "{header}: {clean:?}");
        assert!(
            clean.contains("  host: localhost\r\npublic: ok\r\n"),
            "{clean:?}"
        );
        assert_eq!(SecretRedactor::redact(&clean), clean);
    }
}

#[test]
fn yaml_plain_equals_values_are_credentials_not_nested_mappings() {
    for value in ["user=correcthorse", "provider = vault", "ask-123456789=x"] {
        let input = format!("password:\n  {value}\npublic: ok\n");
        assert_eq!(
            SecretRedactor::redact(&input),
            "password:\n  [REDACTED:SECRET]\npublic: ok\n"
        );
    }
}

#[test]
fn redaction_reaches_a_stable_continuation_form_on_the_first_pass() {
    for input in [
        "TOKEN:\nPsk-ssVk-kP-P=S",
        "TOKEN:\nPsk-OyyyTOKEN:",
        "password:\n  ask-123456789=x\n",
        std::str::from_utf8(include_bytes!("fixtures/redact-original-v120.bin")).unwrap(),
    ] {
        let clean = SecretRedactor::redact(input);
        assert_eq!(SecretRedactor::redact(&clean), clean, "{input:?}");
    }
}

#[test]
fn yaml_public_siblings_and_nested_mappings_stay_exact() {
    for input in [
        "credentials:\n  username: alice\n  region: local\n",
        "password:\npublic: ok\n",
        "password: |\npublic: ok\n",
    ] {
        assert_eq!(SecretRedactor::redact(input), input);
    }
}

#[test]
fn yaml_list_block_stops_before_a_sibling_mapping() {
    assert_eq!(
        SecretRedactor::redact("- password: |2\n    firstpiece\n  public: ok\n"),
        "- password: |2\n    [REDACTED:SECRET]\n  public: ok\n"
    );
}

#[test]
fn yaml_tagged_and_anchored_block_scalars_are_redacted() {
    for header in ["&anchor |", "!!str >", "|2- # vault", ">-2\t# vault"] {
        let input = format!("password: {header}\n  firstpiece\npublic: ok\n");
        let clean = SecretRedactor::redact(&input);
        assert!(!clean.contains("firstpiece"), "{clean:?}");
        assert!(clean.ends_with("public: ok\n"), "{clean:?}");
        assert_eq!(SecretRedactor::redact(&clean), clean);
    }
}

#[test]
fn yaml_scalar_contents_are_not_reclassified_as_comments_or_structures() {
    let input = "password: |\n  # firstpiece\n  user: secondpiece\n  -----BEGIN PRIVATE KEY-----\n  thirdpiece\npublic: ok\n";
    assert_eq!(
        SecretRedactor::redact(input),
        "password: |\n  [REDACTED:SECRET]\n  [REDACTED:SECRET]\n  [REDACTED:SECRET]\n  [REDACTED:SECRET]\npublic: ok\n"
    );
    assert_eq!(
        SecretRedactor::redact("password:\n  firstpiece\n  secondpiece\npublic: ok\n"),
        "password:\n  [REDACTED:SECRET]\n  [REDACTED:SECRET]\npublic: ok\n"
    );
}

#[test]
fn adjacent_sendgrid_credentials_are_removed_on_the_first_pass() {
    let first = format!("SG.{}.{}", "A".repeat(22), "B".repeat(43));
    let second = format!("SG.{}.{}", "C".repeat(22), "D".repeat(43));
    let input = format!("before {first}{second} after");
    let once = SecretRedactor::redact(&input);
    assert!(!once.contains(&first), "{once}");
    assert!(!once.contains(&second), "{once}");
    assert_eq!(
        once,
        "before [REDACTED:SENDGRID_KEY][REDACTED:SENDGRID_KEY] after"
    );
    assert_eq!(SecretRedactor::redact(&once), once);
    let fuzz = std::str::from_utf8(include_bytes!("fixtures/redact-sendgrid-v120.bin")).unwrap();
    let once = SecretRedactor::redact(fuzz);
    assert_eq!(SecretRedactor::redact(&once), once);
}

#[test]
fn quoted_inline_values_cannot_turn_prose_into_pending_secret_headers() {
    let input = "TOKEN:' '_TOKEN:\n:";
    assert_eq!(
        SecretRedactor::redact(input),
        "TOKEN:'[REDACTED:CONFIG_SECRET]'_TOKEN:\n:"
    );
    let once = SecretRedactor::redact(input);
    assert_eq!(SecretRedactor::redact(&once), once);
    let original = std::str::from_utf8(include_bytes!("fixtures/redact-header-v120.bin")).unwrap();
    let once = SecretRedactor::redact(original);
    assert_eq!(SecretRedactor::redact(&once), once);
}

#[test]
fn low_entropy_jwt_and_escaped_quoted_cli_secrets_are_recognized() {
    let jwt = "eyJhbGciOiJub25lIn0.eyJhIjoxfQ.aaaaaa";
    assert_eq!(SecretRedactor::redact(jwt), "[REDACTED:JWT_TOKEN]");
    let command = r#"run --password=\"firstpiece\" --api-key 'secondpiece' --public keep"#;
    let clean = SecretRedactor::redact(command);
    assert!(!clean.contains("firstpiece"));
    assert!(!clean.contains("secondpiece"));
    assert!(clean.ends_with("--public keep"));
    assert_eq!(SecretRedactor::redact(&clean), clean);
}

#[test]
fn nested_url_scheme_does_not_hide_a_credential_until_after_vendor_replacement() {
    let input = format!("sk-abcdefghijklmn://inner://{}@host/path", "A".repeat(32));
    let clean = SecretRedactor::redact(&input);
    assert!(!clean.contains(&"A".repeat(32)), "{clean}");
    assert!(!clean.contains("sk-abcdefghijklmn"));
    assert!(clean.ends_with("@host/path"));
    assert_eq!(SecretRedactor::redact(&clean), clean);
    let original =
        std::str::from_utf8(include_bytes!("fixtures/redact-nested-url-v120.bin")).unwrap();
    let once = SecretRedactor::redact(original);
    assert_eq!(SecretRedactor::redact(&once), once);
}

#[test]
fn adjacent_jwt_credentials_do_not_require_a_second_redaction_pass() {
    let jwt = "eyJhbGciOiJub25lIn0.eyJhIjoxfQ.aaaaaa";
    let input = format!("before {jwt}{jwt} after");
    let once = SecretRedactor::redact(&input);
    assert!(!once.contains(jwt), "{once}");
    assert_eq!(SecretRedactor::redact(&once), once);
    assert!(once.starts_with("before ") && once.ends_with(" after"));
}

#[test]
fn mixed_escaped_quotes_keep_following_unicode_on_a_character_boundary() {
    for (input, expected) in [
        (
            "token:\\'secret'\u{1D538}:public",
            "token:\\'[REDACTED:CONFIG_SECRET]'\u{1D538}:public",
        ),
        (
            "TOKEN=\\'secret'\u{1D538} public",
            "TOKEN=\\'[REDACTED:ENV_SECRET]'\u{1D538} public",
        ),
    ] {
        let clean = SecretRedactor::redact(input);
        assert_eq!(clean, expected);
        assert_eq!(SecretRedactor::redact(&clean), clean);
    }
}

#[test]
fn extra_authority_at_signs_do_not_hide_an_earlier_url_credential() {
    let input = format!("https://{}@public-host@trailing", "A".repeat(32));
    assert_eq!(
        SecretRedactor::redact(&input),
        "https://[REDACTED:URL_TOKEN]@public-host@trailing"
    );
}

#[test]
fn additional_fuzzer_artifacts_are_panic_free_and_stable() {
    for bytes in [
        include_bytes!("fixtures/redact-unicode-panic-v120.bin").as_slice(),
        include_bytes!("fixtures/redact-overlapping-authority-v120.bin").as_slice(),
    ] {
        let input = std::str::from_utf8(bytes).unwrap();
        let once = SecretRedactor::redact(input);
        assert_eq!(SecretRedactor::redact(&once), once);
    }
}

#[test]
fn pending_scalar_is_redacted_before_emitting_a_private_key_marker() {
    for kind in ["PRIVATE KEY", "PGP PRIVATE KEY BLOCK"] {
        let input = format!(
            "TOKEN:\nfirstpiece -----BEGIN {kind}-----\nsecondpiece\n-----END {kind}-----\npublic: ok\n"
        );
        let clean = SecretRedactor::redact(&input);
        assert_eq!(clean, "TOKEN:\n[REDACTED:SECRET]\npublic: ok\n");
        assert_eq!(SecretRedactor::redact(&clean), clean);
        let inline = format!(
            "TOKEN:\nfirstpiece -----BEGIN {kind}-----secondpiece-----END {kind}-----\npublic: ok\n"
        );
        assert_eq!(SecretRedactor::redact(&inline), clean);
    }
}

#[test]
fn original_pending_scalar_private_key_fuzz_artifact_is_stable() {
    let input =
        std::str::from_utf8(include_bytes!("fixtures/redact-pending-key-block-v120.bin")).unwrap();
    let once = SecretRedactor::redact(input);
    assert_eq!(SecretRedactor::redact(&once), once);
}

/// Asserts that no listed fragment survives redaction, that the public tail survives, and that
/// redaction is idempotent.
fn assert_hidden(input: &str, secrets: &[&str], public: &str) {
    let clean = SecretRedactor::redact(input);
    for secret in secrets {
        assert!(
            !clean.contains(secret),
            "{secret:?} leaked from {input:?}: {clean:?}"
        );
    }
    assert!(
        clean.contains(public),
        "public text lost from {input:?}: {clean:?}"
    );
    assert_eq!(
        SecretRedactor::redact(&clean),
        clean,
        "not idempotent for {input:?}"
    );
}

#[test]
fn block_header_after_sibling_assignments_still_redacts_the_siblings() {
    // A trailing `password: |` header must not exempt earlier assignments on the same line.
    assert_hidden(
        "{api_key: hunter2zzlow, password: |\n  blocksecretline\npublic: ok\n",
        &["hunter2zzlow", "blocksecretline"],
        "public: ok",
    );
    assert_hidden(
        "\"api_key\": \"hunter2zzlow\", \"password\": |\n  blocksecretline\npublic: ok\n",
        &["hunter2zzlow", "blocksecretline"],
        "public: ok",
    );
}

#[test]
fn plain_multiline_scalar_continuations_less_indented_than_the_first_line_are_hidden() {
    // YAML only requires continuation lines to be indented past the key (PyYAML parses the
    // value as "hunter2zzlow qwertyuiop9").
    assert_hidden(
        "password:\n    hunter2zzlow\n  qwertyuiop9\npublic: ok\n",
        &["hunter2zzlow", "qwertyuiop9"],
        "public: ok",
    );
}

#[test]
fn double_and_single_quoted_multiline_scalars_are_hidden_until_the_closing_quote() {
    assert_hidden(
        "password: \"hunter2zzlow\n  qwertyuiop9\"\npublic: ok\n",
        &["hunter2zzlow", "qwertyuiop9"],
        "public: ok",
    );
    assert_hidden(
        "token: 'hunter2zzlow\n  qwertyuiop9\n  thirdlinesecret'\npublic: ok\n",
        &["hunter2zzlow", "qwertyuiop9", "thirdlinesecret"],
        "public: ok",
    );
}

#[test]
fn flow_sequence_values_of_sensitive_keys_are_hidden() {
    assert_hidden(
        "password: [hunter2zzlow]\npublic: ok\n",
        &["hunter2zzlow"],
        "public: ok",
    );
    assert_hidden(
        "api_key: [firstkeylow, secondkeylow]\npublic: ok\n",
        &["firstkeylow", "secondkeylow"],
        "public: ok",
    );
}

#[test]
fn plural_collection_keys_are_sensitive_but_ordinary_plurals_are_not() {
    assert_hidden(
        "api_keys: [firstkeylow, secondkeylow]\npublic: ok\n",
        &["firstkeylow", "secondkeylow"],
        "public: ok",
    );
    assert_hidden(
        "tokens:\n  - firstkeylow\npublic: ok\n",
        &["firstkeylow"],
        "public: ok",
    );
    assert_hidden(
        "DB_PASSWORDS=firstkeylow\npublic: ok\n",
        &["firstkeylow"],
        "public: ok",
    );
    for prose in [
        "max_passes: 3\n",
        "bypass: true\n",
        "addresses: 10.0.0.1\n",
        "status: passes\n",
    ] {
        assert_eq!(SecretRedactor::redact(prose), prose);
    }
}

#[test]
fn public_hex_digests_survive_but_hex_values_of_sensitive_keys_do_not() {
    // Checksums and commit ids in recognised digest contexts are public identifiers that
    // release verification depends on; any other long hex on a `=`/`:` line stays hidden.
    let sha256 = "e8ab57722d7d794fbffd585ad1499391b7707ea68f848aa9b20ad189605e26e0";
    let sha1 = "9d59826c3f1f4f0bf8bd6b5f0b0d3e9a1c2b4d6e";
    for public in [
        format!("{sha256}  icx-v1.1.0-x86_64-unknown-linux-musl.tar.gz\n"),
        format!("digest: sha256:{sha256}\n"),
        format!("path,bytes,sha256\nREADME.md,65220,{sha256}\n"),
        format!("commit {sha1}\nAuthor: DoubleGate\n"),
        format!("| `file.pdf` | 99,115 | `{sha256}` |\n"),
    ] {
        assert_eq!(
            SecretRedactor::redact(&public),
            public,
            "public digest redacted"
        );
    }
    for secret in [
        format!("api_key={sha256}\n"),
        format!("password: {sha1}\n"),
        format!("export GITHUB_TOKEN={sha1}\n"),
        format!("{{\"stdout\":\"\\n{sha256}\"}}\n"),
    ] {
        let clean = SecretRedactor::redact(&secret);
        assert!(
            !clean.contains(&sha256[..20]) && !clean.contains(&sha1[..20]),
            "{clean:?}"
        );
    }
}

#[test]
fn quoted_values_containing_colons_or_hashes_are_classified_outside_the_quotes() {
    // A `:` inside an open quoted value must not move the key separator (PR #22 review).
    assert_hidden(
        "password: \"lowsecret:part\n  continuedsecret\"\npublic: ok\n",
        &["lowsecret", "continuedsecret"],
        "public: ok",
    );
    // A ` #` inside a closed quoted value is not a comment, so no quote is left open and the
    // following public lines survive.
    for input in [
        "password: \"lowsecret #frag\"\npublic: ok\nnext: line\n",
        "password: 'it''s #frag'\npublic: ok\nnext: line\n",
    ] {
        let clean = SecretRedactor::redact(input);
        assert!(clean.contains("public: ok\nnext: line\n"), "{clean:?}");
        assert!(
            !clean.contains("lowsecret") && !clean.contains("frag"),
            "{clean:?}"
        );
        assert_eq!(SecretRedactor::redact(&clean), clean);
    }
    // A real trailing comment after a closed value is still a comment.
    let clean = SecretRedactor::redact("password: \"lowsecret\" # rotated\npublic: ok\n");
    assert!(
        !clean.contains("lowsecret") && clean.contains("public: ok"),
        "{clean:?}"
    );
}

#[test]
fn quoted_brackets_inside_flow_sequences_do_not_end_redaction_early() {
    assert_hidden(
        "password: [\"ab]lowsecret\", \"secondsecret\"]\npublic: ok\n",
        &["lowsecret", "secondsecret"],
        "public: ok",
    );
}

#[test]
fn closing_quote_of_a_multiline_secret_does_not_hide_later_assignments() {
    // After the quoted scalar closes, the rest of the line is classified normally, so a
    // trailing sensitive assignment is still redacted.
    assert_hidden(
        "password: \"lowsecret\n  tailsecret\", api_key: \"afterquote\"\npublic: ok\n",
        &["lowsecret", "tailsecret", "afterquote"],
        "public: ok",
    );
}
