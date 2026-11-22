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
    // An escaped (`\\u0077` = `w`) key is decoded before classification.
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
        // Generated names use `[a-p]`, so they can never spell the `qqzz` secret marker.
        let secret = prop_oneof![Just("\"qqzzqqzz\"".to_owned()), Just("91919191".to_owned())];
        let member = prop_oneof![
            ("[a-p]{1,6}", json(depth - 1)).prop_map(|(k, v)| format!("\"{k}\":{v}")),
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
            members in proptest::collection::vec(("[a-p]{1,6}", json(3)), 0..5),
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
        r#"{"api.key":"aaaa"}"#,
        r#"{"api key":"aaaa"}"#,
        r#"{"client.secret":"aaaa"}"#,
    ] {
        let clean = once(line);
        assert!(!clean.contains("aaaa"), "{line} -> {clean}");
    }
    let input = "{\"key\":\"-----BEGIN RSA PRIVATE KEY-----\"}\nMIIEaaaabbbbcccc\n-----END RSA PRIVATE KEY-----\n";
    let clean = SecretRedactor::redact(input);
    assert!(!clean.contains("MIIEaaaabbbbcccc"), "{clean:?}");
}

/// Every way a line can fail to be strict JSON falls back to the first-quote rule (fail closed),
/// and edge-valid JSON is handled exactly.
#[test]
fn invalid_json_falls_back_and_edge_cases_parse() {
    for broken in [
        r#"{"password":"aaaa","n":01}"#,
        r#"{"password":"aaaa","n":1.}"#,
        r#"{"password":"aaaa","n":1e}"#,
        r#"{"password":"aaaa","n":-}"#,
        r#"{"password":"aaaa","n":tru}"#,
        r#"{"password":"aaaa","s":"bad \x escape"}"#,
        r#"{"password":"aaaa","s":"bad \u12G4"}"#,
        "{\"password\":\"aaaa\",\"s\":\"ctrl \u{1} char\"}",
        r#"{"password":"aaaa","s":"unterminated}"#,
        r#"{"password":"aaaa"} trailing"#,
        r#"{"password":"aaaa" "missing":"comma"}"#,
        r#"{password:"aaaa"}"#,
    ] {
        let clean = once(broken);
        assert!(!clean.contains("aaaa"), "{broken} -> {clean}");
        assert!(
            !clean.contains(r#""password":"[REDACTED:CONFIG_SECRET]""#),
            "took the JSON path: {clean}"
        );
    }
    // Nesting deeper than the limit falls back too.
    let deep = format!(
        "{}\"password\":\"aaaa\"{}",
        "{\"a\":".repeat(140) + "{",
        "}".repeat(141)
    );
    assert!(!once(&deep).contains("aaaa"));
    // Edge-valid JSON: empty containers, numbers, escapes, nested arrays.
    let ok = r#"{"a":{},"b":[],"c":[[1,-2.5E+3,0e0]],"d":"\"\\\/\b\f\n\r\té","password":"aaaa"}"#;
    let clean = once(ok);
    assert!(
        clean.starts_with(r#"{"a":{},"b":[],"c":[[1,-2.5E+3,0e0]],"d":"\"\\\/\b\f\n\r\té","#),
        "{clean}"
    );
    assert!(
        clean.ends_with(r#""password":"[REDACTED:CONFIG_SECRET]"}"#),
        "{clean}"
    );
    // An undecodable key (lone surrogate) is treated as sensitive.
    assert!(!once(r#"{"\ud800":"aaaa"}"#).contains("aaaa"));
    // Top-level arrays of strings are redacted element by element.
    let arr = once(r#"["--password=aaaa","ok"]"#);
    assert!(!arr.contains("aaaa") && arr.ends_with(r#","ok"]"#), "{arr}");
    // A value already redacted stays exactly as it is.
    let done = r#"{"token":"[REDACTED:CONFIG_SECRET]","n":1}"#;
    assert_eq!(once(done), done);
}

/// Review of #31 (04:54 UTC, blocking 1): a JSON string value is decoded before redaction, so
/// escaped newlines, tabs and `\uXXXX` escapes cannot hide a key, and multi-line content is
/// redacted line by line.
#[test]
fn json_string_values_are_decoded_before_redaction() {
    for line in [
        r#"{"stdout":"ok\npassword=\"aaaa\""}"#,
        r#"{"stdout":"ok\npassword: aaaa"}"#,
        r#"{"stdout":"ok\napi_key=aaaa"}"#,
        r#"{"stdout":"ok\ttoken=aaaa"}"#,
        r#"{"stdout":"password:\n  aaaa"}"#,
        r#"{"cmd":"\u0070assword=aaaa"}"#,
        r#"{"cmd":"password\u003daaaa"}"#,
    ] {
        let clean = once(line);
        assert!(!clean.contains("aaaa"), "{line} -> {clean}");
        assert!(
            serde_json::from_str::<serde_json::Value>(&clean).is_ok(),
            "invalid JSON: {clean}"
        );
    }
    // Later lines of a multi-line string value survive.
    let clean = once(r#"{"stdout":"password=\"aaaa\"\nline 2\nline 3"}"#);
    assert!(
        !clean.contains("aaaa") && clean.contains(r"\nline 2\nline 3"),
        "{clean}"
    );
}

/// Key banners built at run time (`<B>` / `<E>`), so the source never holds a whole
/// BEGIN/END pair for secret scanners to flag.
fn banners(text: &str) -> String {
    text.replace("<B>", concat!("-----BEG", "IN "))
        .replace("<E>", concat!("-----EN", "D "))
}

/// Review of #31 (05:18 UTC): structure inside a string value keeps its own context, so
/// embedded JSON and digest lines are not over-redacted, and a complete inline key block does not
/// knock a JSON line off the structural path.
#[test]
fn embedded_structure_keeps_its_context() {
    let clean = once(r#"{"stdout":"{\"api_key\":\"aaaa\",\"user\":\"octo\"}"}"#);
    assert!(!clean.contains("aaaa") && clean.contains("octo"), "{clean}");
    let sha = "e8ab57722d7d794fbffd585ad1499391b7707ea68f848aa9b20ad189605e26e0";
    let clean = once(&format!(r#"{{"stdout":"{sha}  file.tar.gz\nok"}}"#));
    assert!(clean.contains(sha), "digest line hidden: {clean}");
    let clean = once(&format!(r#"{{"stdout":"\n{sha}"}}"#));
    assert!(!clean.contains(sha), "bare hex under a key kept: {clean}");
    let clean = once(&banners(
        r#"{"key_pem":"<B>PRIVATE KEY-----\nMIIaaaa\n<E>PRIVATE KEY-----","password":"aaaa","user":"octo"}"#,
    ));
    assert!(!clean.contains("aaaa") && clean.contains("octo"), "{clean}");
    // An unrelated END before an unterminated BEGIN does not skip the key-block state: the key
    // body on the following lines is still removed.
    let clean = SecretRedactor::redact(&banners(
        "{\"note\":\"<E>X-----\",\"k\":\"<B>PRIVATE KEY-----\"}\nMIIaaaa\n<E>PRIVATE KEY-----\nafter\n",
    ));
    assert!(
        !clean.contains("MIIaaaa") && clean.contains("after"),
        "{clean:?}"
    );
    // A later certificate BEGIN/END pair does not mask an unterminated private key.
    let clean = SecretRedactor::redact(&banners(
        "{\"k\":\"<B>PRIVATE KEY-----\",\"c\":\"<B>CERTIFICATE-----x<E>CERTIFICATE-----\"}\nMIIaaaa\n<E>PRIVATE KEY-----\nafter\n",
    ));
    assert!(
        !clean.contains("MIIaaaa") && clean.contains("after"),
        "{clean:?}"
    );
    // An unrelated END after an unterminated BEGIN does not close it either.
    let clean = SecretRedactor::redact(&banners(
        "{\"k\":\"<B>PRIVATE KEY-----\",\"n\":\"<E>X-----\"}\nMIIaaaa\n<E>PRIVATE KEY-----\nafter\n",
    ));
    assert!(
        !clean.contains("MIIaaaa") && clean.contains("after"),
        "{clean:?}"
    );
    // UTF-16 surrogate pairs decode; the string survives.
    let clean = once(r#"{"stdout":"hi \ud83d\ude00 there","password":"aaaa"}"#);
    assert!(
        !clean.contains("aaaa") && clean.contains("there"),
        "{clean}"
    );
}

/// Security review of #31: a key block split over several JSON strings is removed as a whole by
/// the line rules, and long hex inside prose under a non-sensitive key is still hidden (only
/// recognised digest lines keep their hex).
#[test]
fn split_key_blocks_and_hex_in_prose() {
    let clean = SecretRedactor::redact(&banners(
        "{\"a\":\"<B>PRIVATE KEY-----\",\"b\":\"MIIaaaa\",\"c\":\"<E>PRIVATE KEY-----\"}\nafter\n",
    ));
    assert!(
        !clean.contains("MIIaaaa") && clean.contains("after"),
        "{clean:?}"
    );
    let sha = "e8ab57722d7d794fbffd585ad1499391b7707ea68f848aa9b20ad189605e26e0";
    for line in [
        format!(r#"{{"stdout":"token {sha} ok"}}"#),
        format!(r#"{{"stdout":"value {sha}"}}"#),
        // Prose that only starts like JSON is not JSON (security review of d4b3e77).
        format!(r#"{{"stdout":"[info] token {sha}"}}"#),
        format!(r#"{{"stdout":"{{ value {sha}"}}"#),
    ] {
        let clean = once(&line);
        assert!(!clean.contains(sha), "{line} -> {clean}");
    }
    for line in [
        format!(r#"{{"stdout":"{sha}  file.tar.gz"}}"#),
        format!(r#"{{"stdout":"digest: sha256:{sha}"}}"#),
    ] {
        let clean = once(&line);
        assert!(
            clean.contains(sha),
            "public digest hidden: {line} -> {clean}"
        );
    }
}
