// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later AND AGPL-3.0-or-later

//! v1.6.0 (#107, #108): redactor shapes found in the v1.5.1 review rounds (#106) that v1.5.0 also
//! leaked, each pinned as hidden, with neighbours that must stay visible. Placeholders are
//! low-entropy (`qqzz`, `rrss`) or plain digits.

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
    let mut failures = Vec::new();
    for (input, hidden, kept) in cases {
        let clean = once(input);
        for h in *hidden {
            if clean.contains(h) {
                failures.push(format!("{h:?} survived: {input:?} -> {clean:?}"));
            }
        }
        for k in *kept {
            if !clean.contains(k) {
                failures.push(format!("{k:?} was hidden: {input:?} -> {clean:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// #107: a single-quoted multi-line value after an apostrophe on the same line.
#[test]
fn apostrophe_before_a_single_quoted_multi_line_value() {
    check(&[
        (
            "don't log; password = '''\nqqzz\n'''\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        (
            "don't log; password = 'part1\nqqzz'\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        (
            "'outer: let api_key = 'part1\nqqzz';\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        // A closed double-quoted string with an apostrophe after it does not arm anything.
        (
            "note = \"password = (\" ; don't\nvisible line\n",
            &[],
            &["visible line"],
        ),
    ]);
}

/// #108: YAML comments, block keys and the name/value window.
#[test]
fn yaml_comments_block_keys_and_window() {
    let mut long_block = String::new();
    for i in 0..14 {
        long_block.push_str(&format!("    line{i}rrss\n"));
    }
    let long = format!("- value: |\n{long_block}  name: DB_PASSWORD\n- name: HOST\n  value: db\n");
    check(&[
        (
            "tokens: # prod\n  - qqzz\nnext: 1\n",
            &["qqzz"],
            &["next: 1"],
        ),
        (
            "- name: DB_PASSWORD # prod\n  value: qqzz\n",
            &["qqzz"],
            &[],
        ),
        (
            "- name: DB_PASSWORD\n  value: # wrapped\n    qqzz\n",
            &["qqzz"],
            &[],
        ),
        (
            "tokens:\n    - qqzz\n  [REDACTED:X]\nnext: 1\n",
            &["qqzz"],
            &["next: 1"],
        ),
        (&long, &["rrss"], &["value: db"]),
    ]);
}

/// #108: a glued planted marker followed by `#` or `\` is one shell word.
#[test]
fn glued_marker_then_punctuation() {
    check(&[
        ("password=[REDACTED:X]#qqzz\n", &["qqzz"], &[]),
        ("password=[REDACTED:X]\\qqzz\n", &["qqzz"], &[]),
        // A comment after a space stays a comment.
        ("password=[REDACTED:X] # note\n", &[], &["# note"]),
    ]);
}

/// #108: PIN and OTP numbers in URLs, form strings, quotes and calls.
#[test]
fn pin_and_otp_in_urls_quotes_and_calls() {
    check(&[
        (
            "https://h/verify?otp=482913&user=a\n",
            &["482913"],
            &["user=a"],
        ),
        ("user=a&pin=1234&next=/\n", &["1234"], &["next=/"]),
        ("https://h/?pin=1234&x=1\n", &["1234"], &["x=1"]),
        ("echo \"USER_PIN=482913\"\n", &["482913"], &[]),
        ("log(\"pin: 1234\")\n", &["1234"], &["log("]),
        ("https://h/?totp=482913\n", &["482913"], &[]),
    ]);
}

/// #108: subscript and `=>` literals.
#[test]
fn subscript_and_arrow_literals() {
    check(&[
        ("obj[\"password\"] = \"\"\"qqzz\"\"\"\n", &["qqzz"], &[]),
        ("obj[\"password\"] = r\"qqzz\"\n", &["qqzz"], &[]),
        ("obj[\"password\"] = f\"qqzz\"\n", &["qqzz"], &[]),
        ("'password' => `qqzz`\n", &["qqzz"], &[]),
        // Code on the right still stays.
        ("obj[\"password\"] = get_secret()\n", &[], &["get_secret()"]),
    ]);
}

/// #108: CLI segment and tool-scope edge cases.
#[test]
fn cli_segments_and_tool_scope() {
    check(&[
        (
            "sshpass -p \"part1; part2qqzz\" && echo \"unclosed\n",
            &["part2qqzz"],
            &[],
        ),
        ("sshpass -p qqzz ssh -p 2222 host\n", &["qqzz"], &["2222"]),
        (
            "echo ok; docker login --password-stdin registry\n",
            &[],
            &["echo ok"],
        ),
        (
            "sshpass -p \u{FF02}my qqzz\u{FF02} ssh host\n",
            &["qqzz"],
            &["ssh host"],
        ),
        (
            "log {\"cmd\": \"sshpass -p \\\"my qqzz\\\" ssh host\"}\n",
            &["qqzz"],
            &["ssh host"],
        ),
    ]);
}

/// #108: a full-width triple-quoted multi-line value.
#[test]
fn full_width_triple_quoted_multi_line() {
    check(&[(
        "password = \u{FF02}\u{FF02}\u{FF02}\nqqzz\n\u{FF02}\u{FF02}\u{FF02}\nnext = 1\n",
        &["qqzz"],
        &["next = 1"],
    )]);
}

/// A collapsed multi-line block elsewhere in the text does not normalise secret-free lines
/// (found by the v1.6.0 corpus check: the from-scratch look-alike pass returned the whole
/// normalised text when line counts differed).
#[test]
fn collapsed_blocks_keep_lookalike_lines_exact() {
    let text = concat!(
        "full-width \u{FF1D} and \u{FF02}quotes\u{FF02} stay\n",
        "-----BEGIN ",
        "PRIVATE KEY-----\nqqzzqqzzqqzz\nrrssrrssrrss\n-----END ",
        "PRIVATE KEY-----\n",
        "plain tail\n"
    );
    let out = SecretRedactor::redact(text);
    assert!(
        out.starts_with("full-width \u{FF1D} and \u{FF02}quotes\u{FF02} stay\n"),
        "{out:?}"
    );
    assert!(!out.contains("qqzz") && !out.contains("rrss"), "{out:?}");
    assert!(out.ends_with("plain tail\n"), "{out:?}");
}

/// A line with full-width quotes where several passes hide different values keeps every value
/// hidden (the merge is a union, not a choice of one pass; side-agent review of #110).
#[test]
fn full_width_lines_hide_what_any_pass_found() {
    check(&[
        (
            "api_key=\u{FF02}rrss abcd\u{FF02} and sshpass -p \u{FF02}my qqzz\u{FF02} host\n",
            &["rrss", "abcd", "qqzz"],
            &[],
        ),
        (
            "password=\u{FF02}a1b2 c3d4\u{FF02}; TOKEN=rrsszzqq \u{FF02}x\u{FF02}\n",
            &["a1b2", "c3d4", "rrsszzqq"],
            &[],
        ),
    ]);
}

/// Review round on #110 (Antigravity and Codex).
#[test]
fn review_round_on_110() {
    let key = concat!(
        "-----BEGIN ",
        "PRIVATE KEY-----\nqqzzqqzzqqzz\nrrssrrssrrss\n-----END ",
        "PRIVATE KEY-----\n"
    );
    check(&[
        // A collapsed key block in the same text does not drop the full-width pass.
        (
            &format!("{key}sshpass -p \u{FF02}my zzqq\u{FF02} ssh host\n"),
            &["zzqq", "qqzz"],
            &[],
        ),
        (
            &format!("{key}password = \u{FF02}\u{FF02}\u{FF02}\nzzqq\n\u{FF02}\u{FF02}\u{FF02}\n"),
            &["zzqq"],
            &[],
        ),
        // Several full-width-quoted values on one line.
        (
            "password = \u{FF02}s1s1\u{FF02} token = \u{FF02}my xyxy\u{FF02}\n",
            &["s1s1", "xyxy"],
            &[],
        ),
        ("password = \u{FF02}my zzqq\u{FF02}\n", &["zzqq"], &[]),
        // Embedded JSON after a multi-byte character.
        (
            "\u{2022} {\"cmd\": \"sshpass -p \\\"my qqzz\\\" ssh host\"}\n",
            &["qqzz"],
            &[],
        ),
        // An escaped triple quote inside a triple-quoted literal.
        (
            "obj[\"password\"] = \"\"\"foo\\\"\"\"qqzz\"\"\"\n",
            &["qqzz"],
            &[],
        ),
        // Each sshpass invocation in a segment.
        (
            "sshpass -p qqzz ssh gateway sshpass -p rrss ssh host\n",
            &["qqzz", "rrss"],
            &["host"],
        ),
    ]);
    // A bracket-heavy line stays fast.
    let line = format!("x {}{}\n", "[".repeat(20_000), "]}");
    let t = std::time::Instant::now();
    let _ = SecretRedactor::redact(&line);
    assert!(t.elapsed().as_millis() < 2_000, "{:?}", t.elapsed());
}
