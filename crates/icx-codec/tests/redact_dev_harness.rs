// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later AND AGPL-3.0-or-later

//! Developer harness for redactor changes: a deterministic fragment fuzz and a case-file runner.
//! Both are `#[ignore]`d and only report; nothing here is a pass/fail gate. They are the tools
//! the v1.5.x red-team and review rounds used, kept so the next redactor change can be compared
//! against its base with `scripts/redactor-differential.sh`.
//!
//! `qqzz` stands for a secret value and `aaaa-aaaa` for a UUID-shaped one. A `qqzz` that survives
//! is a leak only where a sensitive key precedes it, so a fuzz "leak" line is a candidate: compare
//! two trees, and read the lines that leak on one and not the other.
//!
//! ```text
//! # fragment fuzz: N lines (default 20000) from a fixed seed; LEAKS=1 prints each leaking line
//! FUZZ_N=200000 LEAKS=1 cargo test --release -p icx-codec --test redact_dev_harness fuzz -- --ignored --nocapture
//! # case file: a JSON array of strings; ALL=1 also prints the cases that are fine
//! CASES=cases.json cargo test -p icx-codec --test redact_dev_harness cases -- --ignored --nocapture
//! ```

use icx_codec::SecretRedactor;

/// Fragments the fuzz joins into lines: sensitive keys, separators, quotes, shell and config
/// forms, markers, look-alike characters and placeholder values.
const FRAGMENTS: &[&str] = &[
    "password",
    "api_key",
    "token",
    "session_id",
    "secret",
    "PASSWORD",
    "--token",
    "=",
    " = ",
    ":",
    ": ",
    "\"",
    "'",
    "`",
    " ",
    ",",
    ";",
    "(",
    ")",
    "[",
    "]",
    "{",
    "}",
    "#",
    " #",
    "//",
    "null",
    "None",
    "1234",
    "$DB_PASSWORD",
    "os.environ[\"API_KEY\"]",
    "config.x",
    "get_token(u)",
    "qqzz",
    "\\",
    "\\\"",
    "\n",
    "  ",
    "- ",
    "|",
    ">",
    "&",
    "export ",
    "let ",
    "https://u:",
    "@h",
    "Bearer ",
    "[REDACTED:ENV_SECRET]",
    "r\"",
    "b'",
    "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
    ".",
    "=>",
    "\r\n",
    "<password>[REDACTED:SECRET]</password>",
    "<add key=\"Password\" value=\"",
    "set -x ",
    "setenv ",
    "[\"password\"] = ",
    "\u{00A0}",
    "\u{FF1D}",
    "\u{200B}",
    "pin: ",
    "otp=",
    "4821",
    "sshpass -p ",
    "mysql -p",
    "docker login -p ",
    "curl -u a:",
    "?sig=",
    "&code=",
    "- name: ",
    "value: ",
    "\"name\": \"PASSWORD\", \"value\": ",
    "[\"--password\", ",
    "<<< ",
    "--password-stdin",
];

/// One printable line: separators and quotes swapped for look-alikes so output stays greppable
/// and `sort -u`/`comm` can compare two runs.
fn show(s: &str) -> String {
    s.replace("qqzz", "§")
        .replace('=', "≡")
        .replace(':', "¦")
        .replace('"', "ʺ")
        .replace('\'', "ʹ")
        .replace('\n', "⏎")
        .replace('\r', "␍")
        .replace(' ', "␠")
}

fn leaks(redacted: &str) -> bool {
    redacted.contains("qqzz") || redacted.contains("aaaa-aaaa")
}

fn env_flag(name: &str) -> bool {
    std::env::var_os(name).is_some()
}

#[test]
#[ignore = "developer harness: reports, never fails"]
fn fuzz() {
    let n: usize = std::env::var("FUZZ_N")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20_000);
    let print_leaks = env_flag("LEAKS");
    // xorshift64 from a fixed seed: the same N gives the same lines on every tree and machine.
    let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let (mut leaking, mut non_idempotent) = (0usize, 0usize);
    for _ in 0..n {
        let parts = 2 + next() % 10;
        let mut line = String::new();
        for _ in 0..parts {
            let pick = usize::try_from(next() % FRAGMENTS.len() as u64).unwrap_or(0);
            line.push_str(FRAGMENTS[pick]);
        }
        let once = SecretRedactor::redact(&line);
        let twice = SecretRedactor::redact(&once);
        if leaks(&once) {
            leaking += 1;
            if print_leaks {
                println!("L {}", show(&line));
            }
        }
        if once != twice {
            non_idempotent += 1;
            if non_idempotent <= 12 {
                println!(
                    "NONIDEM\n   in : {}\n   1st: {}\n   2nd: {}",
                    show(&line),
                    show(&once),
                    show(&twice)
                );
            }
        }
    }
    println!(
        "fuzz: {n} lines, {leaking} with a surviving placeholder, {non_idempotent} non-idempotent"
    );
}

#[test]
#[ignore = "developer harness: reports, never fails"]
fn cases() {
    let Some(path) = std::env::var_os("CASES") else {
        println!("cases: set CASES to a JSON array of strings");
        return;
    };
    let text = std::fs::read_to_string(&path).expect("CASES file is readable");
    let cases: Vec<String> = serde_json::from_str(&text).expect("CASES is a JSON array of strings");
    let all = env_flag("ALL");
    for (i, case) in cases.iter().enumerate() {
        let once = SecretRedactor::redact(case);
        let twice = SecretRedactor::redact(&once);
        let tag = match (leaks(&once), once == twice) {
            (true, true) => "LEAK",
            (true, false) => "LEAK+NONIDEM",
            (false, false) => "NONIDEM",
            (false, true) => "ok",
        };
        if all || tag != "ok" {
            println!(
                "#{i} {tag}\n   in : {}\n   out: {}",
                show(case),
                show(&once)
            );
            if once != twice {
                println!("   2nd: {}", show(&twice));
            }
        } else {
            println!("#{i} ok");
        }
    }
}
