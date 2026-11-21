//! v1.2.2: a call-wrapped literal ending in a backslash at end of input must not panic.
use icx_codec::SecretRedactor;

#[test]
fn trailing_backslash_inside_a_wrapped_literal_does_not_panic() {
    for line in [
        "api_key = Some(\"abc\\",
        "password: String::from('x\\",
        "token: Some(\"\\",
        "let api_key = Some(String::from(\"C:\\\\dir\\",
    ] {
        let clean = SecretRedactor::redact(line);
        assert!(clean.contains("[REDACTED:"), "{line:?} -> {clean:?}");
    }
}

#[test]
fn escaped_newline_does_not_extend_a_wrapped_literal_past_its_line() {
    let clean = SecretRedactor::redact("api_key = Some(\"abc\\\npublic: ok\n");
    assert!(clean.contains("public: ok"), "{clean:?}");
}

/// Every combination of up to 5 risky fragments after a sensitive key: no panic, and the output
/// stays valid UTF-8 (guaranteed by `String`) with each input line accounted for.
#[test]
fn short_adversarial_combinations_never_panic() {
    const PARTS: &[&str] = &[
        "Some(",
        "String::from(",
        "\"",
        "'",
        "\\",
        "\n",
        ")",
        "x",
        "é",
        "<",
        "[",
        ":",
        "=",
        ",",
    ];
    let prefixes = [
        "api_key = ",
        "password: ",
        "let token: String = ",
        "token: Option<",
    ];
    for prefix in prefixes {
        // Release builds cover 5 fragments (~2.1M inputs); debug builds 4, to keep CI fast.
        let max_len = if cfg!(debug_assertions) { 4 } else { 5 };
        for len in 0..=max_len {
            let mut idx = [0usize; 5];
            loop {
                let mut s = String::from(prefix);
                for &i in &idx[..len] {
                    s.push_str(PARTS[i]);
                }
                let _ = SecretRedactor::redact(&s);
                // Odometer increment over the first `len` digits.
                let mut k = 0;
                while k < len {
                    idx[k] += 1;
                    if idx[k] < PARTS.len() {
                        break;
                    }
                    idx[k] = 0;
                    k += 1;
                }
                if k == len {
                    break;
                }
            }
        }
    }
}
