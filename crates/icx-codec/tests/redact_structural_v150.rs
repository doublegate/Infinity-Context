// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later AND AGPL-3.0-or-later

//! The structural redactor (v1.5.0, ADR-0011, T-74-003): a sensitive assignment loses its value,
//! not the rest of its line, and values that are never secrets stay. The #29 bypass shapes are
//! pinned in `redact_line_literals.rs`; this file pins the #90 cases, the kept tails, field
//! lists and session ids, generates secrets in every supported format, and measures the corpus.
//! Placeholder secrets are low-entropy letters on purpose (gitleaks), so only the key rules can
//! catch them.

use icx_codec::SecretRedactor;
use proptest::prelude::*;

fn once(line: &str) -> String {
    let clean = SecretRedactor::redact(line);
    assert_eq!(
        SecretRedactor::redact(&clean),
        clean,
        "not idempotent: {clean:?}"
    );
    clean
}

/// #90: values under sensitive keys that cannot be secrets stay exactly as written.
#[test]
fn non_secret_values_under_sensitive_keys_stay() {
    for line in [
        "def f(current_tokens: int = None):\n",
        "proactive_prune_tokens=1000\n",
        "\"session_id\": \"8f2c1e9a-3b4d-4c5e-9f60-718293a4b5c6\",\n",
        "api_key = os.environ[\"API_KEY\"]\n",
        "api_key: Optional[str] = None\n",
        "token = \"\"\n",
        "\"api_key\": null,\n",
        "secret_key = settings.SECRET_KEY\n",
        "let session_token = get_token(user);\n",
        "token: true\n",
        "max_tokens: 4096\n",
        "let api_key = config.api_key.clone();\n",
        "password = $DB_PASSWORD\n",
        "TOKEN=${GITHUB_TOKEN}\n",
        "token = env::var(\"GITHUB_TOKEN\")\n",
        // Seen through v1.4.1 stubs while reading this repository's source (2026-11-25).
        "let mut total_raw_tokens = 0;\n",
        "let mut saw_pass = false;\n",
        "{\"api_key\": null, \"token\": \"\", \"session_id\": \"8f2c1e9a-3b4d-4c5e-9f60-718293a4b5c6\"}\n",
    ] {
        assert_eq!(once(line), line, "{line:?}");
    }
}

/// A real secret costs only its own bytes; a quote-free tail stays.
#[test]
fn a_secret_loses_only_its_literal() {
    for (line, expected) in [
        (
            "password = \"qqzzqqzz\"  # keep this comment\n",
            "password = \"[REDACTED:ENV_SECRET]\"  # keep this comment\n",
        ),
        // A bare-word tail is part of an INI value, so it is hidden (red-team review of #103).
        (
            "client_secret = 'qqzzqqzz' if prod else None\n",
            "client_secret = '[REDACTED:ENV_SECRET]' [REDACTED:ENV_SECRET]\n",
        ),
        (
            "PASSWORD=\"qqzzqqzz\" BAR=2\n",
            "PASSWORD=\"[REDACTED:ENV_SECRET]\" BAR=2\n",
        ),
        (
            "export GITHUB_TOKEN=\"qqzzqqzz\"\n",
            "export GITHUB_TOKEN=\"[REDACTED:ENV_SECRET]\"\n",
        ),
        (
            "password: \"qqzzqqzz\" # rotated monthly\n",
            "password: \"[REDACTED:CONFIG_SECRET]\" # rotated monthly\n",
        ),
    ] {
        assert_eq!(once(line), expected, "{line:?}");
    }
}

/// A quoted tail after the secret is hidden from its first quote, as in #29.
#[test]
fn a_quoted_tail_is_still_hidden() {
    for line in [
        "password = \"qqzzqqzz\" + \"rrssrrss\"\n",
        "token = None or \"rrssrrss\"\n",
        "let password = get_pw(); // \"rrssrrss\"\n",
        "api_key = os.environ[\"API_KEY\"] or \"rrssrrss\"\n",
    ] {
        let clean = once(line);
        assert!(
            !clean.contains("qqzz") && !clean.contains("rrss"),
            "{line:?} -> {clean:?}"
        );
    }
}

/// JSON members without braces are parsed strictly: only sensitive values go (ADR-0011 rule 5).
#[test]
fn json_field_lists_keep_their_other_fields() {
    assert_eq!(
        once("  \"access_token\": \"qqzzqqzz\", \"expires_in\": 3599,\n"),
        "  \"access_token\": \"[REDACTED:CONFIG_SECRET]\", \"expires_in\": 3599,\n"
    );
    // Not strict JSON: the line rule, so the quoted rest is hidden.
    let clean = once("\"access_token\": \"qqzzqqzz\", expires_in: 'x'\n");
    assert!(
        !clean.contains("qqzz") && !clean.contains("'x'"),
        "{clean:?}"
    );
}

/// ADR-0011 rule 6: an agent session UUID stays in a JSON member named exactly `session_id` or
/// `sessionId` (hook payloads); anything else under a session key does not (red-team review of
/// #103: header, env and prefixed session ids can be credentials).
#[test]
fn only_uuid_session_ids_stay() {
    let uuid = "8f2c1e9a-3b4d-4c5e-9f60-718293a4b5c6";
    for line in [
        format!("\"sessionId\": \"{uuid}\"\n"),
        format!("{{\"session_id\": \"{uuid}\", \"cwd\": \"/x\"}}\n"),
    ] {
        assert_eq!(once(&line), line, "{line:?}");
    }
    for line in [
        format!("session_id: {uuid}\n"),
        format!("SESSION_ID={uuid}\n"),
        format!("claude_session_id={uuid}\n"),
        format!("X-Session-Id: {uuid}\n"),
        format!("{{\"sso_session_id\": \"{uuid}\"}}\n"),
        format!("cookie_session_id={uuid}\n"),
        format!("token_session_id: {uuid}\n"),
        format!("password_session_id = {uuid}\n"),
        format!("session_token: {uuid}\n"),
        format!("auth_session_id={uuid}\n"),
        "session_id: qqzzqqzzqqzz\n".to_owned(),
        "session_cookie = \"qqzzqqzz\"\n".to_owned(),
    ] {
        let clean = once(&line);
        assert!(
            !clean.contains("qqzz") && !clean.contains(uuid),
            "{line:?} -> {clean:?}"
        );
    }
}

/// References stay only in code; in config text the same shape is a literal.
#[test]
fn references_stay_only_in_code() {
    assert_eq!(
        once("secret_key = settings.SECRET_KEY\n"),
        "secret_key = settings.SECRET_KEY\n"
    );
    for line in [
        "password: my.dog.rex\n",
        "PASSWORD=my.dog.rex\n",
        "password = hunter\n",
    ] {
        let clean = once(line);
        assert!(
            !clean.contains("my.dog.rex") && !clean.contains("hunter"),
            "{line:?} -> {clean:?}"
        );
    }
}

/// Security review of the T-74-003 commit: three ways a secret could survive.
#[test]
fn review_bypasses_stay_closed() {
    for line in [
        // A literal glued to more text is one shell word: the tail is part of the secret.
        "PASSWORD=\"qqzz\"rrssttuu\n",
        "token = 'qqzz'rrssttuu\n",
        // A password that starts with `$` is not a variable reference.
        "PASSWORD=$rrssttuu\n",
        "password: ${rrssttuu}\n",
        // INI and properties files allow spaces around `=`.
        "password = rrss.ttuu.vvww\n",
        "api_key = rrsstt?\n",
        "token = get(rrss ttuu!)\n",
        // Second review: comment characters glued to a literal, plain calls, short `$NAME`s.
        "PASSWORD=\"a\"#rrssttuu\n",
        "PASSWORD=\"a\"/rrssttuu\n",
        "PASSWORD=\"a\",rrssttuu\n",
        "password = rrss(ttuu)\n",
        "PASSWORD=$RRSSTTUU\n",
        // Review of #103: a harmless value or literal glued to more text, exponents, a
        // second assignment in the kept tail, an INI value ending in a short constant.
        "api_key: 1234,rrssttuu\n",
        "password: null,rrssttuu\n",
        "token: true,rrssttuu\n",
        "secret: \"\",rrssttuu\n",
        "password: \"a\",rrssttuu\n",
        "PASSWORD=null)rrssttuu\n",
        "API_KEY=1234]rrssttuu\n",
        "TOKEN=true}rrssttuu\n",
        "PASSWORD=null;rrssttuu\n",
        "PASSWORD=\"\";rrssttuu\n",
        "API_KEY=48291038e9182736rrss\n",
        "PASSWORD=\"qqzz\" TOKEN=rrssttuu # \"c\"\n",
        "password = rrss.PASS\n",
        // A comma inside brackets ends the value only before whitespace or structure.
        "{token: abc,rrssttuu}\n",
        "f(token=abc,rrssttuu)\n",
    ] {
        let clean = once(line);
        assert!(!clean.contains("rrss"), "{line:?} -> {clean:?}");
    }
    // Strong code signals (a binding keyword or a trailing `;`) still keep a reference visible;
    // spaces around `=` alone keep only a constant path (red-team review of #103).
    for line in [
        "let token = self.token;\n",
        "api_key = config.api_key;\n",
        "secret_key = settings.SECRET_KEY\n",
        "const token = load_token(path);\n",
        "password = $DB_PASSWORD\n",
        "let token = getToken(user);\n",
        "let api_key = os.getenv(KEY_NAME);\n",
    ] {
        assert_eq!(once(line), line, "{line:?}");
    }
}

/// Security review of 298bc52: a short bearer token is hidden too, and a long run of spaces in a
/// value is scanned in linear time.
#[test]
fn short_auth_tokens_are_hidden_and_space_runs_are_linear() {
    for line in [
        "Authorization: Bearer qqzz\n",
        "Authorization: Basic qqzz\n",
        "\"authorization\": \"Bearer qqzz\"\n",
    ] {
        let clean = once(line);
        assert!(!clean.contains("qqzz"), "{line:?} -> {clean:?}");
    }
    assert_eq!(
        once("Authorization: Bearer qqzzqqzz\n"),
        "Authorization: Bearer [REDACTED:BEARER]\n"
    );
    let long = format!("password: a{}b\n", " ".repeat(200_000));
    let start = std::time::Instant::now();
    let clean = once(&long);
    assert!(
        start.elapsed() < std::time::Duration::from_secs(2),
        "{:?}",
        start.elapsed()
    );
    assert!(!clean.contains('b'), "the value runs past the spaces");
}

const KEYS: &[&str] = &[
    "password",
    "api_key",
    "token",
    "secret",
    "client_secret",
    "ACCESS_TOKEN",
    "session_id",
];

/// Every supported shape of a sensitive assignment, with `{k}` the key and `{s}` the secret.
const SHAPES: &[&str] = &[
    "{k}={s}",
    "{k}=\"{s}\"",
    "export {k}=\"{s}\"",
    "{k}: {s}",
    "{k}: \"{s}\"",
    "\"{k}\": \"{s}\"",
    "\"{k}\": \"{s}\",",
    "{{\"{k}\": \"{s}\", \"n\": 1}}",
    "\"a\": 1, \"{k}\": \"{s}\", \"b\": 2",
    "{k} = '{s}'  # note",
    "{k} = \"{s}\" if ok else None",
    "let {k} = \"{s}\";",
    "const {k} = '{s}';",
    "{k} := \"{s}\"",
    "f({k}=\"{s}\", other=1)",
    "--{k} {s}",
    "--{k}=\"{s}\"",
    "{k} = x or \"{s}\"",
    "{k} = \"a\" + \"{s}\"",
    "{k} = env::var(\"K\").unwrap_or(\"{s}\")",
    "{k}: str = None or \"{s}\"",
    "{k} = None; other = \"{s}\"",
    "{k}=\"a\"{s}",
    "{k}=${s}",
    "{k}=\"a\"#{s}",
    "{k}: 1234,{s}",
    "{k}=null;{s}",
    "{k}=null){s}",
    "{k}=\"a\" other_token={s} # \"c\"",
    "{k}=\"a\"/{s}",
    "{k} = {s}({s})",
    "{k} = {s}.{s}",
];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2_000))]

    #[test]
    fn generated_secrets_never_survive(
        key in proptest::sample::select(KEYS),
        shape in proptest::sample::select(SHAPES),
        secret in "[q-z]{8,16}",
    ) {
        let line = shape.replace("{k}", key).replace("{s}", &secret).replace("{{", "{").replace("}}", "}");
        let clean = SecretRedactor::redact(&format!("{line}\n"));
        prop_assert!(!clean.contains(&secret), "{} -> {}", line, clean);
        prop_assert_eq!(SecretRedactor::redact(&clean), clean);
    }

    #[test]
    fn arbitrary_text_never_panics(text in "\\PC{0,200}") {
        let clean = SecretRedactor::redact(&text);
        prop_assert_eq!(SecretRedactor::redact(&clean), clean);
    }

    #[test]
    fn assignment_like_noise_never_panics(
        key in proptest::sample::select(KEYS),
        rest in "[ -~]{0,60}",
    ) {
        let _ = SecretRedactor::redact(&format!("{key} = {rest}\n{key}: {rest}\n\"{key}\": {rest}\n"));
    }
}

/// The measured corpus (T-74-003): each line with the secrets it holds. Lines from #90 and the
/// 2026-11-22 field log's shapes (re-typed, no session text), plus real secrets with tails.
const CORPUS: &[(&str, &[&str])] = &[
    ("def f(current_tokens: int = None):", &[]),
    ("proactive_prune_tokens=1000", &[]),
    (
        "\"session_id\": \"8f2c1e9a-3b4d-4c5e-9f60-718293a4b5c6\",",
        &[],
    ),
    ("api_key = os.environ[\"API_KEY\"]", &[]),
    ("api_key: Optional[str] = None", &[]),
    ("token = \"\"", &[]),
    ("\"api_key\": null,", &[]),
    ("secret_key = settings.SECRET_KEY", &[]),
    ("let session_token = get_token(user);", &[]),
    ("token: true", &[]),
    ("max_tokens: 4096", &[]),
    ("let api_key = config.api_key.clone();", &[]),
    (
        "password = \"qqzzqqzz\"  # keep this comment",
        &["qqzzqqzz"],
    ),
    ("API_KEY=qqzzrrss  # prod", &["qqzzrrss"]),
    (
        "\"access_token\": \"qqzzsstt\", \"expires_in\": 3599",
        &["qqzzsstt"],
    ),
    (
        "client_secret = 'qqzzuuvv' if prod else None",
        &["qqzzuuvv"],
    ),
    ("export GITHUB_TOKEN=\"qqzzwwxx\"", &["qqzzwwxx"]),
    (
        "password = \"qqzzyyzz\" + \"rrssttuu\"",
        &["qqzzyyzz", "rrssttuu"],
    ),
    (
        "token = env::var(\"K\").unwrap_or(\"rrssvvww\")",
        &["rrssvvww"],
    ),
];

/// Run with `cargo test -p icx-codec --test redact_structural_v150 measure -- --ignored
/// --nocapture`: missed secrets, and bytes hidden that were not part of a secret.
#[test]
#[ignore = "measurement"]
fn measure_corpus() {
    let (mut missed, mut over, mut total) = (0usize, 0usize, 0usize);
    for (line, secrets) in CORPUS {
        let clean = SecretRedactor::redact(line);
        missed += secrets.iter().filter(|s| clean.contains(*s)).count();
        let (a, b) = (line.as_bytes(), clean.as_bytes());
        let pre = a.iter().zip(b).take_while(|(x, y)| x == y).count();
        let suf = a[pre..]
            .iter()
            .rev()
            .zip(b[pre..].iter().rev())
            .take_while(|(x, y)| x == y)
            .count();
        let hidden = a.len() - pre - suf;
        let secret_bytes: usize = secrets.iter().map(|s| s.len()).sum();
        over += hidden.saturating_sub(secret_bytes);
        total += a.len();
    }
    println!(
        "lines={} bytes={total} missed_secrets={missed} over_redacted_bytes={over}",
        CORPUS.len()
    );
}
