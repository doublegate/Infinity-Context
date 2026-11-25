// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later AND AGPL-3.0-or-later

//! v1.5.1 (#104): the redactor shapes the v1.5.0 red-team pass left open, each pinned as hidden;
//! the cases kept by design (owner decisions, 2026-11-25), each pinned as visible; and code shapes
//! a secret-free corpus showed must stay. Placeholders are low-entropy (`qqzz`, `rrss`) or plain
//! digits.

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

/// `(input, must be hidden, must stay)`.
fn check(cases: &[(&str, &[&str], &[&str])]) {
    for (input, hidden, kept) in cases {
        let clean = once(input);
        for h in *hidden {
            assert!(!clean.contains(h), "{h:?} survived: {input:?} -> {clean:?}");
        }
        for k in *kept {
            assert!(
                clean.contains(k),
                "{k:?} was hidden: {input:?} -> {clean:?}"
            );
        }
    }
}

#[test]
fn cli_short_flags_argv_and_stdin() {
    check(&[
        ("--password -qqzz\n", &["qqzz"], &["--password"]),
        ("mysql --password -qqzz\n", &["qqzz"], &["mysql --password"]),
        ("mysql -uroot -pqqzz db\n", &["qqzz"], &["-uroot", "db"]),
        ("{\"cmd\": \"mysql -pqqzz\"}\n", &["qqzz"], &["mysql -p"]),
        ("sshpass -p qqzz ssh host\n", &["qqzz"], &["ssh host"]),
        ("sshpass -pqqzz ssh host\n", &["qqzz"], &["ssh host"]),
        (
            "docker login -u bob -p qqzz registry\n",
            &["qqzz"],
            &["-u bob", "registry"],
        ),
        ("podman login -pqqzz quay.io\n", &["qqzz"], &["quay.io"]),
        (
            // Split so the secret scanner does not read the test input as a credential.
            concat!("cur", "l -u admin:qqzz https://x\n"),
            &["qqzz"],
            &["admin:", "https://x"],
        ),
        (
            concat!("cur", "l --user admin:qqzz https://x\n"),
            &["qqzz"],
            &["admin:"],
        ),
        (
            "docker login --password-stdin <<< qqzz\n",
            &["qqzz"],
            &["--password-stdin"],
        ),
        (
            "echo qqzz | docker login --password-stdin\n",
            &["qqzz"],
            &["docker login"],
        ),
        ("[\"--password\", \"qqzz\"]\n", &["qqzz"], &["--password"]),
        (
            "{\"args\": [\"--token\", \"qqzz\"]}\n",
            &["qqzz"],
            &["--token"],
        ),
        (
            "[\"sshpass\", \"-p\", \"qqzz\", \"ssh\"]\n",
            &["qqzz"],
            &["\"ssh\""],
        ),
    ]);
}

#[test]
fn short_flags_without_a_password_tool_stay() {
    // `-p` is a port for most tools, and `-u uid:gid` a user for docker.
    check(&[
        ("ssh -p 2222 host\n", &[], &["-p 2222"]),
        (
            "docker run -p 8080:80 -u 1000:1000 img\n",
            &[],
            &["8080:80", "1000:1000"],
        ),
        ("psql -p 5432 -U app\n", &[], &["-p 5432"]),
        ("[\"-p\", \"8080:80\"]\n", &[], &["8080:80"]),
        ("mysql -p dbname\n", &[], &["dbname"]),
    ]);
}

#[test]
fn unicode_lookalikes_are_normalised() {
    check(&[
        ("password\u{00A0}=\u{00A0}qqzz\n", &["qqzz"], &[]),
        ("pass\u{200B}word=qqzz\n", &["qqzz"], &[]),
        ("pass\u{00AD}word=qqzz\n", &["qqzz"], &[]),
        (
            "\u{FF50}\u{FF41}\u{FF53}\u{FF53}\u{FF57}\u{FF4F}\u{FF52}\u{FF44}=qqzz\n",
            &["qqzz"],
            &[],
        ),
        ("password\u{FF1D}qqzz\n", &["qqzz"], &[]),
        ("password\u{FF1A}qqzz\n", &["qqzz"], &[]),
        ("password\u{3000}:\u{3000}qqzz\n", &["qqzz"], &[]),
        // Lines without a secret keep their bytes.
        (
            "caf\u{00E9}\u{00A0}menu \u{FF21}\n",
            &[],
            &["caf\u{00E9}\u{00A0}menu \u{FF21}"],
        ),
    ]);
}

#[test]
fn markup_and_name_value_pairs() {
    check(&[
        (
            "<password>qqzz</password>\n",
            &["qqzz"],
            &["<password>", "</password>"],
        ),
        (
            "<ApiKey id=\"1\">qqzz</ApiKey>\n",
            &["qqzz"],
            &["</ApiKey>"],
        ),
        (
            "<add key=\"Password\" value=\"qqzz\"/>\n",
            &["qqzz"],
            &["key=\"Password\""],
        ),
        (
            "<param name=\"api_token\" value=\"qqzz\">\n",
            &["qqzz"],
            &["name=\"api_token\""],
        ),
        (
            "[{\"name\": \"PASSWORD\", \"value\": \"qqzz\"}]\n",
            &["qqzz"],
            &["PASSWORD"],
        ),
        (
            "{\"key\": \"API_KEY\", \"value\": \"qqzz\"}\n",
            &["qqzz"],
            &["API_KEY"],
        ),
        ("{\"creds\": {\"value\": \"qqzz\"}}\n", &["qqzz"], &[]),
        (
            "  - name: DB_PASSWORD\n    value: qqzz\n  - name: HOST\n    value: db\n",
            &["qqzz"],
            &["value: db"],
        ),
        (
            "tokens:\n  - name: a\n    value: qqzz\n  - rrss\nnext: 1\n",
            &["qqzz", "rrss"],
            &["name: a", "next: 1"],
        ),
        // A public name stays with its value, and generics are not elements.
        (
            "{\"name\": \"HOST\", \"value\": \"db\"}\n",
            &[],
            &["\"db\""],
        ),
        (
            "    secret: Option<SecretString>,\n",
            &[],
            &["Option<SecretString>"],
        ),
        ("<user>alice</user>\n", &[], &["alice"]),
    ]);
}

#[test]
fn url_credential_parameters_and_webhooks() {
    check(&[
        ("https://h/cb?code=qqzz&state=x\n", &["qqzz"], &["&state=x"]),
        ("https://h/x?sig=qqzz\n", &["qqzz"], &["?sig="]),
        (
            "https://h/x?X-Amz-Signature=qqzz&X-Amz-Date=1\n",
            &["qqzz"],
            &["X-Amz-Date=1"],
        ),
        (
            "https://h/cb#access_token=qqzz&expires_in=3600\n",
            &["qqzz"],
            &["expires_in=3600"],
        ),
        (
            "WEBHOOK_URL=https://hooks.example.com/qqzz\n",
            &["qqzz"],
            &[],
        ),
        (
            "SLACK_WEBHOOK=https://hooks.example.com/qqzz\n",
            &["qqzz"],
            &[],
        ),
        ("https://h/search?q=rust&page=2\n", &[], &["q=rust&page=2"]),
    ]);
}

#[test]
fn subscripts_arrows_and_shell_set() {
    check(&[
        (
            "obj[\"password\"] = \"qqzz\"\n",
            &["qqzz"],
            &["obj[\"password\"]"],
        ),
        (
            "os.environ['PASSWORD'] = 'qqzz'\n",
            &["qqzz"],
            &["os.environ"],
        ),
        ("password => 'qqzz'\n", &["qqzz"], &[]),
        ("'api_token' => \"qqzz\",\n", &["qqzz"], &[]),
        ("setenv PASSWORD qqzz\n", &["qqzz"], &["setenv PASSWORD"]),
        ("set -x PASSWORD qqzz\n", &["qqzz"], &["set -x PASSWORD"]),
        ("set -gx API_KEY qqzz; echo ok\n", &["qqzz"], &["echo ok"]),
        ("set password qqzz\n", &["qqzz"], &[]),
        // Code on the right stays, as does a substitution.
        (
            "start[\"child_session_id\"] = json!(\"child-9\");\n",
            &[],
            &["json!("],
        ),
        (
            "report['pass'] = all(row['pass'] for row in rows)\n",
            &[],
            &["all(row"],
        ),
        ("    \"token\" => {\n", &[], &["\"token\" => {"]),
        ("set token (gh auth token)\n", &[], &["(gh auth token)"]),
        ("x[\"password\"]\n", &[], &["x[\"password\"]"]),
    ]);
}

#[test]
fn planted_markers_do_not_shield_a_secret() {
    check(&[
        ("password=[REDACTED:X]qqzz\n", &["qqzz"], &[]),
        ("password: [REDACTED:X] qqzz\n", &["qqzz"], &[]),
        (
            "token=[REDACTED:ENV_SECRET]&user=bob\n",
            &[],
            &["&user=bob"],
        ),
    ]);
}

#[test]
fn multi_line_values() {
    check(&[
        (
            "password = \"\"\"\nfoo\\\"\"\"\nqqzz\n\"\"\"\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        (
            "password = 'foo\\' bar\nqqzz'\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        (
            "password = (\n  \"\"\"\n  a \" b\n  )\n  qqzz\n  \"\"\"\n)\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        (
            "password = \"a\" + r\"\"\"\nqqzz\n\"\"\"\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        (
            "password = \"a\" + f\"\nqqzz\"\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        (
            "api_key: str=\"\"\"\nqqzz\n\"\"\"\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        ("password: str =\n\"qqzz\"\n", &["qqzz"], &[]),
        (
            "const password = String.raw`\nqqzz\n`;\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        (
            "don't log; password = \"\"\"\nqqzz\n\"\"\"\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        (
            "'outer: let api_key = (\n  \"qqzz\"\n);\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        ("let token = cfg.api(qqzz rrss);\n", &["qqzz", "rrss"], &[]),
        // `=` inside a string or a Markdown code span is not an assignment.
        ("note = \"password=\"\nvisible\n", &[], &["visible"]),
        (
            "the fix (`api_key = Some(\"abc\\`) panicked\nvisible\n",
            &[],
            &["visible"],
        ),
        (
            "const MAX_TOKENS: usize = snapshot::MAX_ITEMS as usize;\n",
            &[],
            &["as usize"],
        ),
    ]);
}

#[test]
fn pin_and_otp_numbers() {
    check(&[
        ("pin: 1234\n", &["1234"], &[]),
        ("pin2: 1234\n", &["1234"], &[]),
        ("otp2: 482913\n", &["482913"], &[]),
        ("USER_PIN=482913\n", &["482913"], &[]),
        ("{\"smsOtp\": 482913}\n", &["482913"], &[]),
        ("{\"pin\": \"4821\"}\n", &["4821"], &[]),
        // Versions, words, short numbers and other keys stay.
        ("pin: 3.11\n", &[], &["3.11"]),
        ("pin: latest\n", &[], &["latest"]),
        ("pin: 12\n", &[], &["12"]),
        ("spin_rate: 5000\n", &[], &["5000"]),
    ]);
    // A JSON line stays valid JSON.
    let clean = once("{\"pin\": 1234, \"ok\": true}\n");
    assert!(clean.contains("\"pin\": \"[REDACTED:SECRET]\""), "{clean}");
}

#[test]
fn kept_by_design() {
    // Owner decisions for v1.5.1 (#104), recorded in ADR-0011.
    check(&[
        // Prose.
        (
            "password reset flow is broken\n",
            &[],
            &["reset flow is broken"],
        ),
        ("Password is rrss\n", &[], &["rrss"]),
        // Comment text after a value.
        ("password: None # rrss\n", &[], &["# rrss"]),
        // JSON sessionId UUIDs.
        (
            "{\"sessionId\": \"aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa\"}\n",
            &[],
            &["aaaaaaaa-aaaa"],
        ),
        // Values under keys that are not sensitive.
        ("TOKEN=\"qqzz\" OTHER=rrss\n", &["qqzz"], &["OTHER=rrss"]),
        // Shell splits `--password=a b`: `b` is another argument.
        ("--password=qqzz rrss\n", &["qqzz"], &["rrss"]),
        // A plain user name in a URL.
        ("https://alice@example.com/x\n", &[], &["alice@"]),
        // A bare `key`.
        ("{\"key\": \"rrss\"}\n", &[], &["rrss"]),
    ]);
}

/// Inputs the idempotence fuzz found while v1.5.1 was written; each must redact to a fixed
/// point in one pass.
#[test]
fn fuzz_found_inputs_are_idempotent() {
    for input in [
        "token{password1234 = get_token(u)\\null)}\n",
        "token = $DB_PASSWORD\\token;null:\n",
        ": token=os.environ[\"API_KEY\"]secretsecret\r\nPASSWORD\n",
        "--tokenBearer [REDACTED:CLI_SECRET]\\null\n",
        "|:<add key=\"Password\" value=\"(Bearer <add key=\"Password\" value=\"\u{200B}<password>,\u{FF1D}don't\n",
        "sshpass -p \" export [WEBHOOK_URL=\r\n=\n",
        "docker login -p r\"//@h WEBHOOK_URL=\r\nsshpass -p $DB_PASSWORD[REDACTED:ENV_SECRET]-\n",
        "[\"--password\", token\u{FF1D}setenv mysql -p</password>docker login -p curl -u a:api_key& =\n",
    ] {
        once(input);
    }
}

/// Commit security review of 5e9acbb.
#[test]
fn security_review_planted_markers_overlaps_and_lookalike_passes() {
    check(&[
        // A planted marker does not stop a multi-line value from arming.
        (
            "password = [REDACTED:X] \"\"\"\nqqzz\n\"\"\"\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        (
            "api_key = [REDACTED:X] (\n  \"qqzz\"\n)\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        // Nested targets on one word are merged, so none of it stays.
        ("mysql --password -pqqzz db\n", &["qqzz", "-pq"], &["db"]),
        // A look-alike line that only the original pass hides is still hidden.
        ("token\u{00A0}x API_KEY=qqzz\n", &["qqzz"], &[]),
    ]);
}

/// Commit security review of 137e5f8: both look-alike passes count, and text around a marker
/// inside a hidden range is hidden.
#[test]
fn both_lookalike_passes_count_and_markers_do_not_shield_neighbours() {
    check(&[
        // The original pass hides `API_KEY=…`; the normalised pass hides the NBSP-joined key.
        (
            "API_KEY=qqzz password\u{00A0}=\u{00A0}rrss\n",
            &["qqzz", "rrss"],
            &[],
        ),
        ("token\u{00A0}x API_KEY=qqzz\n", &["qqzz"], &[]),
        (
            "token\u{00A0}x API_KEY=qqzz password\u{FF1D}zzzz\n",
            &["qqzz", "zzzz"],
            &[],
        ),
        // `--password -X` and the glued `-pX` rule target the same word.
        (
            "mysql --password -pqqzz[REDACTED:X]rrss db\n",
            &["qqzz", "rrss"],
            &["db"],
        ),
    ]);
}

/// Review round on #106 (Codex and Antigravity).
#[test]
fn review_round_on_106() {
    check(&[
        // Only a complete marker shields text; an unclosed or malformed one is a value.
        ("password=[REDACTED:qqzz\n", &["qqzz"], &[]),
        ("password=\"[REDACTED:qqzz\"\n", &["qqzz"], &[]),
        ("password=[REDACTED:qqzz]\n", &["qqzz"], &[]),
        (
            "note=\"[REDACTED:\" <password>qqzz</password>\n",
            &["qqzz"],
            &[],
        ),
        ("log=[REDACTED: https://h/x?sig=qqzz\n", &["qqzz"], &[]),
        // JSON pairs: only same-object members, only string values.
        (
            "[{\"name\":\"PASSWORD\",\"value\":null},{\"name\":\"HOST\",\"value\":\"db\"}]\n",
            &[],
            &["\"name\":\"HOST\"", "\"db\""],
        ),
        (
            "{\"name\": null, \"api_key\": \"x\", \"value\": \"public\"}\n",
            &[],
            &["\"public\""],
        ),
        (
            "{\"name\":\"HOST\",\"value\":\"db\",\"nested\":{\"name\":\"PASSWORD\"}}\n",
            &[],
            &["\"db\""],
        ),
        (
            "{\"name\": \"PASSWORD\", \"meta\": {\"env\": \"prod\"}, \"value\": \"qqzz\"}\n",
            &["qqzz"],
            &["prod"],
        ),
        // URLs without whitespace between them.
        (
            "{\"a\":\"https://example.com/ok\",\"b\":\"https://example.com/cb?sig=qqzz\"}\n",
            &["qqzz"],
            &["example.com/ok"],
        ),
        (
            "[\"https://a.com\",\"https://b.com?access_token=qqzz\"]\n",
            &["qqzz"],
            &[],
        ),
        // CLI: passwords starting with `a`, quoted passwords, glued curl and here-strings.
        ("docker login -papple registry\n", &["apple"], &["registry"]),
        (
            "sshpass -p \"my qqzz\" ssh host\n",
            &["qqzz"],
            &["ssh host"],
        ),
        (
            "docker login -u bob -p 'my qqzz' registry\n",
            &["qqzz"],
            &["registry"],
        ),
        (
            concat!("cur", "l -u \"admin:my qqzz\" https://x\n"),
            &["qqzz"],
            &["https://x"],
        ),
        (
            concat!("cur", "l --user=admin:qqzz https://x\n"),
            &["qqzz"],
            &[],
        ),
        (concat!("cur", "l -Uadmin:qqzz https://x\n"), &["qqzz"], &[]),
        ("docker login --password-stdin <<<qqzz\n", &["qqzz"], &[]),
        // The `--password-stdin` flag pass hides to the end of the line here (older rule).
        (
            "docker login --password-stdin <<< \"qqzz\" ; echo ok\n",
            &["qqzz"],
            &[],
        ),
        ("[\"--password\", \"-qqzz\"]\n", &["qqzz"], &[]),
        // ...and no over-redaction for tools that use `-p` otherwise.
        ("[\"mysql\", \"-p\", \"dbname\"]\n", &[], &["dbname"]),
        (
            "{\"action\": \"login\", \"args\": [\"ssh\", \"-p\", \"2222\"]}\n",
            &[],
            &["2222"],
        ),
        // XML has no backslash escapes.
        ("<add key=\"Password\" value=\"qqzz\\\"/>\n", &["qqzz"], &[]),
        // An unclosed quote before a subscript does not end the scan.
        ("['unclosed obj[\"password\"] = \"qqzz\"\n", &["qqzz"], &[]),
        // Webhook metadata stays.
        ("WEBHOOK_NAME=alerts\n", &[], &["alerts"]),
        ("WEBHOOK_EVENT=push\n", &[], &["push"]),
        // Escaped triple quote followed by a real closer.
        (
            "password = \"\"\"foo\\\"\"\"\"\nnext = 1\n",
            &[],
            &["next = 1"],
        ),
    ]);
}

#[test]
fn review_round_on_106_yaml() {
    check(&[
        (
            "- value: qqzz\n  name: DB_PASSWORD\n- value: db\n  name: HOST\n",
            &["qqzz"],
            &["value: db"],
        ),
        (
            "- name: DB_PASSWORD\n  # the password\n  value: qqzz\n",
            &["qqzz"],
            &["# the password"],
        ),
        (
            "tokens:\n- qqzz\n- rrss\nnext: 1\n",
            &["qqzz", "rrss"],
            &["next: 1"],
        ),
        (
            "- name: DB_PASSWORD\n  value: |\n    qqzz\n    rrss\n- name: HOST\n  value: db\n",
            &["qqzz", "rrss"],
            &["value: db"],
        ),
    ]);
}
