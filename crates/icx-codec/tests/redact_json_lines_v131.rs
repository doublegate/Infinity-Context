//! v1.3.1 (#29, owner decision on PR #31): a line that is a complete JSON object or array is
//! redacted structurally. The value of a sensitive key (any type) becomes one marker; every
//! other string is redacted on its own; formatting and key order are kept. Lines that are not
//! valid JSON keep the first-quote rule. Placeholders are low-entropy on purpose (gitleaks).
use icx_codec::SecretRedactor;

fn once(line: &str) -> String {
    let clean = SecretRedactor::redact(line);
    assert_eq!(
        SecretRedactor::redact(&clean),
        clean,
        "not idempotent: {clean:?}"
    );
    clean
}

#[test]
fn sensitive_values_are_hidden_and_the_rest_is_kept() {
    let line = r#"{"access_token":"aaaa","token_type":"bearer","expires_in":3600,"user":{"login":"octo","id":42}}"#;
    assert_eq!(
        once(line),
        r#"{"access_token":"[REDACTED:CONFIG_SECRET]","token_type":"bearer","expires_in":3600,"user":{"login":"octo","id":42}}"#
    );
}

#[test]
fn a_long_minified_response_keeps_almost_everything() {
    let items: Vec<String> = (0..40)
        .map(|i| format!(r#"{{"id":{i},"name":"repo-{i}","html_url":"https://example.com/org/repo-{i}","description":"Example {i}"}}"#))
        .collect();
    let line = format!(r#"{{"token":"aaaa","items":[{}]}}"#, items.join(","));
    let clean = once(&line);
    assert!(!clean.contains("aaaa"), "{clean:.200}");
    assert!(
        clean.len() * 100 >= line.len() * 99,
        "kept {} of {}",
        clean.len(),
        line.len()
    );
    assert!(clean.contains(r#""name":"repo-39""#));
}

#[test]
fn every_value_type_under_a_sensitive_key_is_hidden() {
    for (line, expect) in [
        (r#"{"password":aaaa}"#, None),
        (
            r#"{"api_key":"x","password":12345678}"#,
            Some(r#"{"api_key":"[REDACTED:CONFIG_SECRET]","password":"[REDACTED:CONFIG_SECRET]"}"#),
        ),
        (
            r#"{"tokens":[11112222,33334444]}"#,
            Some(r#"{"tokens":"[REDACTED:CONFIG_SECRET]"}"#),
        ),
        (
            r#"{"secret":{"inner":"aaaa"}}"#,
            Some(r#"{"secret":"[REDACTED:CONFIG_SECRET]"}"#),
        ),
        (
            r#"[{"password":"aaaa"},{"name":"ok"}]"#,
            Some(r#"[{"password":"[REDACTED:CONFIG_SECRET]"},{"name":"ok"}]"#),
        ),
    ] {
        let clean = once(line);
        assert!(
            !clean.contains("aaaa") && !clean.contains("1111") && !clean.contains("1234"),
            "{line} -> {clean}"
        );
        if let Some(e) = expect {
            assert_eq!(clean, e);
        }
    }
}

#[test]
fn strings_are_redacted_on_their_own() {
    // A vendor token and an assignment inside ordinary string values.
    let line = r#"{"note":"password=\"aaaa\" then more","cmd":"curl --token aaaa"}"#;
    let clean = once(line);
    assert!(!clean.contains("aaaa"), "{clean}");
    assert!(
        clean.starts_with(r#"{"note":"password="#) && clean.contains(r#","cmd":"curl --token "#),
        "{clean}"
    );
}

#[test]
fn formatting_is_kept_and_invalid_json_falls_back() {
    let pretty_one_line = "  { \"password\" : \"aaaa\" , \"user\" : \"octo\" }  ";
    assert_eq!(
        once(pretty_one_line),
        "  { \"password\" : \"[REDACTED:CONFIG_SECRET]\" , \"user\" : \"octo\" }  "
    );
    // Not valid JSON (trailing comma): the first-quote rule applies.
    let broken = r#"{"password":"aaaa","user":"octo",}"#;
    assert_eq!(once(broken), r#"{"password":[REDACTED:CONFIG_SECRET]"#);
    // Escaped and unicode keys are decoded before classification.
    assert!(!once(r#"{"password":"aaaa"}"#).contains("aaaa"));
}

#[test]
fn secret_free_json_is_unchanged() {
    let line = r#"{"a":[1,2.5e3,-0,true,false,null],"b":{"c":"d\né"}}"#;
    assert_eq!(SecretRedactor::redact(line), line);
}

mod properties {
    use icx_codec::SecretRedactor;
    use proptest::prelude::*;

    /// A JSON value; the values of sensitive keys are always `"qqzz..."` / `9191...` placeholders.
    fn json(depth: u32) -> BoxedStrategy<String> {
        let leaf = prop_oneof![
            "[a-z ]{0,8}".prop_map(|s| format!("\"{s}\"")),
            (0u32..1000).prop_map(|n| n.to_string()),
            Just("true".to_owned()),
            Just("null".to_owned()),
        ];
        if depth == 0 {
            return leaf.boxed();
        }
        let secret = prop_oneof![Just("\"qqzzqqzz\"".to_owned()), Just("91919191".to_owned())];
        let member = prop_oneof![
            ("[a-z]{1,6}", json(depth - 1)).prop_map(|(k, v)| format!("\"{k}\":{v}")),
            (
                prop_oneof![
                    Just("password"),
                    Just("api_key"),
                    Just("token"),
                    Just("secret")
                ],
                secret
            )
                .prop_map(|(k, v)| format!("\"{k}\":{v}")),
        ];
        prop_oneof![
            leaf,
            proptest::collection::vec(member, 0..5).prop_map(|m| format!("{{{}}}", m.join(","))),
            proptest::collection::vec(json(depth - 1), 0..4)
                .prop_map(|v| format!("[{}]", v.join(","))),
        ]
        .boxed()
    }

    proptest! {
        #[test]
        fn sensitive_values_never_survive_and_json_stays_valid(
            members in proptest::collection::vec(("[a-z]{1,6}", json(3)), 0..5),
            key in prop_oneof![Just("password"), Just("api_key"), Just("token")],
        ) {
            let mut body: Vec<String> = members.iter().map(|(k, v)| format!("\"{k}\":{v}")).collect();
            body.insert(0, format!("\"{key}\":\"qqzzqqzz\""));
            let line = format!("{{{}}}", body.join(","));
            let clean = SecretRedactor::redact(&line);
            prop_assert!(!clean.contains("qqzz") && !clean.contains("9191"), "{} -> {}", line, clean);
            prop_assert!(serde_json::from_str::<serde_json::Value>(&clean).is_ok(), "invalid JSON: {}", clean);
            prop_assert_eq!(SecretRedactor::redact(&clean), clean);
        }
    }
}

/// Security review of 1360669: separated key names are classified by their segments like the
/// line rules do, and a JSON line that opens a private-key block keeps the key-block handling.
#[test]
fn separated_keys_and_key_block_banners() {
    for line in [
        r#"{"db.password":"aaaa"}"#,
        r#"{"auth token":"aaaa"}"#,
        r#"{"svc/api_key":"aaaa"}"#,
        r#"{"x-api-key":"aaaa"}"#,
    ] {
        let clean = once(line);
        assert!(!clean.contains("aaaa"), "{line} -> {clean}");
    }
    let input = "{\"key\":\"-----BEGIN RSA PRIVATE KEY-----\"}\nMIIEaaaabbbbcccc\n-----END RSA PRIVATE KEY-----\n";
    let clean = SecretRedactor::redact(input);
    assert!(!clean.contains("MIIEaaaabbbbcccc"), "{clean:?}");
}
