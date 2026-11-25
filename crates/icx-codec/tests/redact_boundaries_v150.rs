// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later AND AGPL-3.0-or-later

//! Boundaries of the v1.5.0 redactor (ADR-0011 and its red-team amendments, T-74-004): where an
//! unquoted value ends, which tails stay, and how multi-line values continue and stop. Each case
//! checks what is hidden and what stays, so a changed boundary is caught. Placeholders are
//! low-entropy (`qqzz`, `rrss`).

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
fn unquoted_values_end_only_at_real_boundaries() {
    check(&[
        // Shell and env text: comments, pipes, `&&`, `;` before space, the next query parameter.
        (
            "PASSWORD=qqzz rrss # note\n",
            &["qqzz", "rrss"],
            &["# note"],
        ),
        (
            "PASSWORD=qqzz rrss // note\n",
            &["qqzz", "rrss"],
            &["// note"],
        ),
        ("PASSWORD=qqzz | grep x\n", &["qqzz"], &["| grep x"]),
        ("PASSWORD=qqzz && run x\n", &["qqzz"], &["&& run x"]),
        ("PASSWORD=qqzz; run x\n", &["qqzz"], &["; run x"]),
        ("PASSWORD=qqzz;rrss\n", &["qqzz", "rrss"], &[]),
        ("PASSWORD=qqzz&rrss\n", &["qqzz", "rrss"], &[]),
        ("token=qqzz&user=bob\n", &["qqzz"], &["&user=bob"]),
        ("token=qqzz&&run\n", &["qqzz"], &["&&run"]),
        ("PASSWORD=qq\rzz\n", &["qq", "zz"], &[]),
        ("PASSWORD=qqzz \n", &["qqzz"], &[]),
        // Config text: `;`, `|` and `&&` are part of the value; comments still end it.
        ("password: qqzz; rrss\n", &["qqzz", "rrss"], &[]),
        ("password: qqzz | rrss\n", &["qqzz", "rrss"], &[]),
        ("password: qqzz && rrss\n", &["qqzz", "rrss"], &[]),
        (
            "password: qqzz rrss # note\n",
            &["qqzz", "rrss"],
            &["# note"],
        ),
        // Inside brackets, a comma or closing bracket before space or structure ends it.
        // The env pass keeps its own comma rule, so the rest of a call is hidden too (safe).
        ("f(token=qqzz, x=1)\n", &["qqzz"], &[]),
        ("f(token=qqzz,rrss)\n", &["qqzz", "rrss"], &[]),
        ("{token: qqzz, user: bob}\n", &["qqzz"], &["user: bob"]),
        // CLI flag values are one argument.
        ("run --token qqzz next\n", &["qqzz"], &["next"]),
    ]);
}

#[test]
fn harmless_values_end_only_at_real_boundaries() {
    check(&[
        ("password: null # rrss\n", &[], &["null", "# rrss"]),
        ("password: null // rrss\n", &[], &["null"]),
        ("password: null ;rrss\n", &["rrss"], &[]),
        // ` ;x` is one shell word with the value, so nothing is kept (review of #103).
        ("PASSWORD=null ;rrss\n", &["rrss"], &[]),
        // `null | x` is not one harmless value, so `null` is hidden too (safe direction).
        ("PASSWORD=null | x\n", &[], &["| x"]),
        ("f(token=None , x=1)\n", &[], &["None", "x=1"]),
        ("f(token=None ,rrss)\n", &[], &["None"]),
        ("token: true rrss\n", &["rrss"], &[]),
        ("{token: null , x: 1}\n", &[], &["null"]),
    ]);
}

#[test]
fn literal_tails() {
    check(&[
        ("password = \"qqzz\"  # note\n", &["qqzz"], &["# note"]),
        ("password = \"qqzz\" // note\n", &["qqzz"], &["// note"]),
        ("password = \"qqzz\" next=1\n", &["qqzz"], &["next=1"]),
        // On an `=` line, `name:` is part of the value (review of #103), so it is hidden.
        ("password = \"qqzz\" user: bob\n", &["qqzz", "bob"], &[]),
        ("password: \"qqzz\" user: bob\n", &["qqzz"], &["user: bob"]),
        // After a value that is not a CLI flag, `--x` is part of the value (review of #103).
        ("password = \"qqzz\" --flag x\n", &["qqzz", "flag"], &[]),
        ("run --token \"qqzz\" --flag x\n", &["qqzz"], &["--flag x"]),
        ("password = \"qqzz\" rrss\n", &["qqzz", "rrss"], &[]),
        ("password = \"qqzz\" 1rrss=x\n", &["qqzz", "rrss"], &[]),
        ("password = \"qqzz\" -rrss\n", &["qqzz", "rrss"], &[]),
        ("password = \"qqzz\"\t\n", &["qqzz"], &[]),
        ("password = \"qqzz\"; rrss\n", &["qqzz"], &["rrss"]),
        ("password: \"qqzz\"; rrss\n", &["qqzz", "rrss"], &[]),
        ("f(token=\"qqzz\"), x\n", &["qqzz"], &["x"]),
    ]);
}

#[test]
fn equals_values_continue_onto_later_lines() {
    check(&[
        // Nothing after `=`: the next line is the value; the one after is not.
        ("PASSWORD=\n\"qqzz\"\nnext=1\n", &["qqzz"], &["next=1"]),
        (
            "  password =\n    qqzz\nuser = bob\n",
            &["qqzz"],
            &["user = bob"],
        ),
        // Unterminated, triple and backtick quotes run to their close.
        (
            "PASSWORD=\"qq\nrrss\"\nnext=1\n",
            &["qq", "rrss"],
            &["next=1"],
        ),
        (
            "PASSWORD='qq\nrrss'\nnext=1\n",
            &["qq", "rrss"],
            &["next=1"],
        ),
        (
            "password = \"\"\"\nrrss\n\"\"\"\nnext = 1\n",
            &["rrss"],
            &["next = 1"],
        ),
        (
            "password = '''\nrrss\n'''\nnext = 1\n",
            &["rrss"],
            &["next = 1"],
        ),
        (
            "password = `\nrrss\n`\nnext = 1\n",
            &["rrss"],
            &["next = 1"],
        ),
        // A trailing backslash continues; the first line without one ends.
        (
            "PASSWORD=qq\\\nrrss\\\nzzqq\nnext=1\n",
            &["qq", "rrss", "zzqq"],
            &["next=1"],
        ),
        // Open brackets continue until they close.
        (
            "api_key = (\n  \"rrss\",\n  [\"qqzz\"],\n)\nnext = 1\n",
            &["rrss", "qqzz"],
            &["next = 1"],
        ),
        (
            "api_key = (\n  rrss)\nnext = 1\n",
            &["rrss"],
            &[")", "next = 1"],
        ),
        (
            "api_key = (\n  (rrss)\n  qqzz\n)\nnext = 1\n",
            &["rrss", "qqzz"],
            &["next = 1"],
        ),
        (
            "api_key = [\n  rrss\n]\nnext = 1\n",
            &["rrss"],
            &["next = 1"],
        ),
        // Not an assignment: comparisons and `=` inside quotes.
        ("if password ==\nvisible\n", &[], &["visible"]),
        ("if password !=\nvisible\n", &[], &["visible"]),
        ("if password <=\nvisible\n", &[], &["visible"]),
        ("note = \"password=\"\nvisible\n", &[], &["visible"]),
        ("x = 'a\\'b' password=\nhidden_rrss\n", &["rrss"], &[]),
        // A non-sensitive key does not continue.
        ("name =\nvisible\n", &[], &["visible"]),
    ]);
}

#[test]
fn yaml_plain_scalars_continue_only_on_indented_lines() {
    check(&[
        (
            "password: qqzz\n  rrss\nuser: bob\n",
            &["qqzz", "rrss"],
            &["user: bob"],
        ),
        (
            "password: qqzz\n\n  rrss\nuser: bob\n",
            &["qqzz", "rrss"],
            &["user: bob"],
        ),
        ("password: true\n  rrss\n", &["rrss"], &["true"]),
        ("password: qqzz\nnext line\n", &["qqzz"], &["next line"]),
        (
            "  password: qqzz\n  sibling: x\n",
            &["qqzz"],
            &["sibling: x"],
        ),
        // Quoted, flow and block values are not plain scalars.
        ("password: \"qqzz\"\n  visible\n", &["qqzz"], &["visible"]),
        ("password: {a: 1}\n  visible\n", &[], &["visible"]),
    ]);
}

#[test]
fn an_unclosed_bracket_stops_after_the_cap() {
    let mut text = String::from("api_key = (\n");
    for i in 0..250 {
        text.push_str(&format!("  line{i}\n"));
    }
    let clean = once(&text);
    assert!(
        !clean.contains("line0\n") && !clean.contains("line199\n"),
        "{clean:.200}"
    );
    assert!(clean.contains("line249"), "the cap ends the continuation");
}

/// Security review of d2d8031: what follows the bracket that closes a multi-line value is
/// redacted like any other line.
#[test]
fn text_after_a_closing_bracket_is_still_redacted() {
    check(&[
        (
            "api_key = (\n  rrss\n); password=qqzzqqzz\n",
            &["rrss", "qqzz"],
            &[");"],
        ),
        (
            "api_key = (\n  rrss\n) export GITHUB_TOKEN=\"qqzzqqzz\"\n",
            &["rrss", "qqzz"],
            &[")"],
        ),
        (
            "token = [\n  rrss\n] # note\nnext = 1\n",
            &["rrss"],
            &["# note", "next = 1"],
        ),
    ]);
}

/// Antigravity review of d2d8031: tails that only look like a new assignment or separator.
#[test]
fn review_round_tail_and_continuation_cases() {
    check(&[
        // A separator outside brackets is part of the value.
        ("password: \"a\" , qqzzqqzz\n", &["qqzz"], &[]),
        ("password = \"a\" ) qqzzqqzz\n", &["qqzz"], &[]),
        ("PASSWORD=\"\" ] qqzzqqzz\n", &["qqzz"], &[]),
        // `name:` on an `=` line, or `--x` outside a CLI flag, is part of the value.
        ("password = \"a\" foo:qqzzqqzz\n", &["qqzz"], &[]),
        ("password: \"a\" --qqzzqqzz\n", &["qqzz"], &[]),
        // `&&` inside a config value.
        ("password: abc&&qqzzqqzz\n", &["qqzz"], &[]),
        // Quotes and brackets win over a trailing backslash.
        (
            "password = \"\"\"\\\nrrss\nqqzz\n\"\"\"\nnext = 1\n",
            &["rrss", "qqzz"],
            &["next = 1"],
        ),
        (
            "api_key = (\\\n  rrss\n  qqzz\n)\nnext = 1\n",
            &["rrss", "qqzz"],
            &["next = 1"],
        ),
        // A later sensitive assignment on the line continues.
        (
            "token = None; password = \"\"\"\nqqzz\n\"\"\"\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        (
            "token = 1; password =\n  qqzz\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        // Only a triple quote closes a triple-quoted value.
        (
            "password = \"\"\"\n\"prefix\"\nqqzz\n\"\"\"\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        (
            "password = r\"\"\"\nqqzz\n\"\"\"\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        (
            "password = f\"\nqqzz\"\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        // Numeric passcodes under the new key spellings.
        ("{\"pwd\": 123456}\n", &["123456"], &[]),
        ("pw = 123456\n", &["123456"], &[]),
        ("DBPASS=123456\n", &["123456"], &[]),
    ]);
}

/// Antigravity review of 43c006c: statement scoping, quote state across bracketed lines, the
/// tail after a closing bracket, and the plain and backslash continuation limits.
#[test]
fn review_round_statement_and_bracket_cases() {
    check(&[
        // Each assignment inspects only its own statement.
        (
            "token = None; password = \"\"\"\\\nrrss\nqqzz\n\"\"\"\nnext = 1\n",
            &["rrss", "qqzz"],
            &["next = 1"],
        ),
        (
            "token = None; password = \"line1\\\nrrss\nqqzz\"\nnext = 1\n",
            &["rrss", "qqzz"],
            &["next = 1"],
        ),
        (
            "let token = None; foo(\n  normal_arg,\n)\n",
            &[],
            &["normal_arg"],
        ),
        // A bracket inside a string spanning lines is not structure.
        (
            "password = (\n  \"line1\n  line2)\n  qqzz\"\n)\nnext = 1\n",
            &["line1", "line2", "qqzz"],
            &["next = 1"],
        ),
        (
            "password = (\"line1\nline2)\nqqzz\")\nnext = 1\n",
            &["line1", "line2", "qqzz"],
            &["next = 1"],
        ),
        // A value continuing after its closing bracket stays hidden.
        (
            "api_key = (\n  \"rrss\"\n) + \"qqzz\"\nnext = 1\n",
            &["rrss", "qqzz"],
            &["next = 1"],
        ),
        // A quote opened inside the bracket closes before the bracket does: no runaway.
        (
            "password = f(\"\n  qqzz\n\")\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        // Rust raw strings.
        (
            "password = r#\"\nqqzz\n\"#\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        // A blank line ends a backslash continuation.
        ("PASSWORD=qq\\\n\nvisible\n", &[], &["visible"]),
        // In config text `&name=` is part of the value.
        ("password: abc&def=qqzzqqzz\n", &["qqzz", "def"], &[]),
    ]);
}

#[test]
fn a_plain_scalar_continuation_stops_after_the_cap() {
    let mut text = String::from("token: qqzz\n");
    for i in 0..250 {
        text.push_str(&format!("  line{i}\n"));
    }
    let clean = once(&text);
    assert!(
        !clean.contains("qqzz") && !clean.contains("line0\n"),
        "{clean:.200}"
    );
    assert!(clean.contains("line249"), "the cap ends the continuation");
}

/// Antigravity review of 2ef664e.
#[test]
fn review_round_assignment_tails_keys_and_continuations() {
    check(&[
        // A tail is a new assignment only with a real `name=value` or `name: value`.
        ("password = \"abc\" qqzzqqzz==\n", &["qqzz"], &[]),
        ("password = \"abc\" secret==qqzzqqzz\n", &["qqzz"], &[]),
        ("password = \"abc\" rrss=>qqzz\n", &["qqzz"], &[]),
        ("password: \"abc\" qqzzqqzz:rrss\n", &["qqzz", "rrss"], &[]),
        ("PASSWORD=\"abc\" BAR=2\n", &["abc"], &["BAR=2"]),
        // A lone carriage return after a space is still inside the value.
        ("PASSWORD=qqzz \rrrss\n", &["qqzz", "rrss"], &[]),
        // ` ;x` without a following space is part of the value.
        ("PASSWORD=abc ;rrss\n", &["abc", "rrss"], &[]),
        ("PASSWORD=null ;rrss\n", &["rrss"], &[]),
        // Dotted keys and prefixed PWD variables.
        ("headers.authorization = \"qqzz\"\n", &["qqzz"], &[]),
        ("client.auth = \"qqzz\"\n", &["qqzz"], &[]),
        ("db.pwd = \"qqzz\"\n", &["qqzz"], &[]),
        ("self.pw = \"qqzz\"\n", &["qqzz"], &[]),
        ("MYSQL_PWD=qqzz\n", &["qqzz"], &[]),
        ("DB_PWD=qqzz\n", &["qqzz"], &[]),
        ("db.pwd = 123456\n", &["123456"], &[]),
        // The shell's PWD and keys about a token, not the token, stay.
        ("PWD=/home/user/project\n", &[], &["/home/user/project"]),
        ("token_savings = \"12 percent\"\n", &[], &["12 percent"]),
        ("secret_redactor = \"structural\"\n", &[], &["structural"]),
        ("token_stream = \"lexer\"\n", &[], &["lexer"]),
        // Qualified keys stay sensitive unless the qualifier names a non-secret attribute
        // (security review of #103: an allowlist failed open).
        ("PASSWORD_ADMIN=qqzz\n", &["qqzz"], &[]),
        ("API_KEY_PROD=qqzz\n", &["qqzz"], &[]),
        ("TOKEN_GITHUB=qqzz\n", &["qqzz"], &[]),
        ("DB_PASSWORD_PROD=qqzz\n", &["qqzz"], &[]),
        ("password_value = \"qqzz\"\n", &["qqzz"], &[]),
        ("token_type = \"bearer\"\n", &[], &["bearer"]),
        // Type-annotated and lifetime-annotated assignments continue.
        (
            "api_key: str = \"\"\"\nqqzz\n\"\"\"\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        (
            "const password: string = `\nqqzz\n`;\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        (
            "let api_key: &'static str = (\n  \"qqzz\"\n);\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        // A quote opened later in the value.
        (
            "password = \"a\" + \"\nqqzz\"\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        (
            "password = prefix + \"\nqqzz\"\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        (
            "password = \"a\" + \"\"\"\nqqzz\n\"\"\"\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        // Comments are not structure.
        (
            "password = (\n  \"qqzz\", # note )\n  \"rrss\"\n)\nnext = 1\n",
            &["qqzz", "rrss"],
            &["next = 1"],
        ),
        ("api_key = None  # see (todo\nvisible\n", &[], &["visible"]),
        // A tail after the closing bracket that opens the next part.
        (
            "api_key = (\n  \"rrss\"\n) + (\n  \"qqzz\"\n)\nnext = 1\n",
            &["rrss", "qqzz"],
            &["next = 1"],
        ),
        (
            "api_key = (\n  \"rrss\"\n) + \\\n  \"qqzz\"\nnext = 1\n",
            &["rrss", "qqzz"],
            &["next = 1"],
        ),
        // An escaped backslash does not continue the line.
        ("PASSWORD=qqzz\\\\\nvisible\n", &["qqzz"], &["visible"]),
    ]);
}

/// Antigravity review of a5dc542 (the parts that are regressions from this PR).
#[test]
fn review_round_annotations_and_comment_markers() {
    check(&[
        (
            "let api_key: std::string::String = (\n  \"qqzz\"\n);\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        ("let api_key: Option<String> = None;\n", &[], &["None;"]),
        (
            "const token: string = config.api_key;\n",
            &[],
            &["config.api_key;"],
        ),
        // `//` glued to text (a URL, `a//b`) is not a comment.
        (
            "api_key = (\n  http://x//y,\n  \"qqzz\"\n)\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
    ]);
}

/// Commit security review of 63d21dd: each loosening applies only where the code is unambiguous.
#[test]
fn annotation_comment_and_code_rules_fail_closed() {
    check(&[
        // A sensitive name before any annotation colon.
        (
            "let a: u8 = 1; let api_key: &str = (\n  \"qqzz\"\n);\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        (
            "fn f(x: u8, api_key: &str = (\n  \"qqzz\"\n)) {}\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        // A glued `//` that is not a URL is a comment: the value stays open, so it is hidden.
        (
            "api_key = (\n  x//)\n  \"qqzz\"\n)\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        // An annotated value on a line without a binding keyword or `;` is config text, where `;`
        // is part of the value.
        ("password: Foo = abc; qqzz\n", &["qqzz"], &[]),
        ("let api_key: Option<String> = None;\n", &[], &["None;"]),
    ]);
}

/// Commit security review of 2916983: a glued `//` can keep a value open but never close it or
/// drop an opener.
#[test]
fn a_glued_double_slash_fails_closed() {
    check(&[
        (
            "api_key = a//b + (\n  \"qqzz\"\n)\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        (
            "api_key = (\n  x//y) \"rrss\"\n  \"qqzz\"\n)\nnext = 1\n",
            &["rrss", "qqzz"],
            &["next = 1"],
        ),
        (
            "api_key = (\n  // a real comment )\n  \"qqzz\"\n)\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        (
            "token = \"https://x\"; api_key = (\n  \"qqzz\"\n)\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
    ]);
}

/// Commit security review of 1fb07e8: an apostrophe after a glued `//` does not hide an opener.
#[test]
fn an_apostrophe_after_a_glued_double_slash_does_not_hide_an_opener() {
    check(&[
        (
            "api_key = a//it's (\n  \"qqzz\"\n)\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
        (
            "api_key = (\n  x//don't )\n  \"qqzz\"\n)\nnext = 1\n",
            &["qqzz"],
            &["next = 1"],
        ),
    ]);
}
