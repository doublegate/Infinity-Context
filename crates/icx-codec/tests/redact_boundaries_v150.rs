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
        ("PASSWORD=null ;rrss\n", &[], &["null"]),
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
