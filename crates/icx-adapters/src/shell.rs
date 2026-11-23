//! A small POSIX-style command-line reader, shared by `icx install` (recognising its own hooks)
//! and the Hermes adapter (recognising icx's own commands). It is not a shell: it only splits
//! words and finds the program, so a mistake can cost at most a missed or duplicated match.

/// Splits `command` into words: single quotes are literal, double quotes allow `\"`, `\\`, `\$`
/// and `` \` `` escapes, and a backslash outside quotes escapes the next character. So
/// `'/it'\''s/icx'` is the one word `/it's/icx`.
#[must_use]
pub fn shell_words(command: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    let mut chars = command.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some('\'') if c == '\'' => quote = None,
            Some('\'') => cur.push(c),
            Some(_) if c == '"' => quote = None,
            Some(_) if c == '\\' => match chars.peek() {
                Some(&n) if matches!(n, '"' | '\\' | '$' | '`') => {
                    cur.push(n);
                    chars.next();
                }
                _ => cur.push(c),
            },
            Some(_) => cur.push(c),
            None if c == '\\' => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
                in_word = true;
            }
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                in_word = true;
            }
            None if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut cur));
                    in_word = false;
                }
            }
            None => {
                cur.push(c);
                in_word = true;
            }
        }
    }
    if in_word {
        words.push(cur);
    }
    words
}

/// The index of the program word in `words`: past leading `KEY=value` assignments and an `env`
/// wrapper with its options (`env -u VAR -i ...`).
#[must_use]
pub fn program_index(words: &[String]) -> Option<usize> {
    let is_assignment = |w: &str| {
        w.split_once('=').is_some_and(|(k, _)| {
            !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
    };
    let mut i = 0;
    while i < words.len() {
        let w = words[i].as_str();
        if is_assignment(w) {
            i += 1;
        } else if w == "env" || w.ends_with("/env") {
            i += 1;
            while i < words.len() && words[i].starts_with('-') {
                // Options that take a value.
                let takes = matches!(
                    words[i].as_str(),
                    "-u" | "--unset" | "-C" | "--chdir" | "-S"
                );
                i += if takes { 2 } else { 1 };
            }
        } else {
            return Some(i);
        }
    }
    None
}

/// Whether `command` runs icx itself (`icx` or the `icx-hook` wrapper, at any path).
#[must_use]
pub fn runs_icx(command: &str) -> bool {
    let words = shell_words(command);
    program_index(&words)
        .and_then(|i| words[i].rsplit('/').next().map(str::to_owned))
        .is_some_and(|p| p == "icx" || p == "icx-hook")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_escapes_and_wrappers() {
        assert_eq!(shell_words(r"'/it'\''s/icx' hook"), ["/it's/icx", "hook"]);
        assert_eq!(shell_words(r#""a \"b\" c" d\ e"#), [r#"a "b" c"#, "d e"]);
        for c in [
            "icx deref x",
            "'/opt/my tools/icx' deref --raw x",
            "ICX_DIR=/w env -u ICX_NO_DAEMON -i /x/icx-hook --agent a",
            "env FOO=1 icx status",
        ] {
            assert!(runs_icx(c), "{c}");
        }
        for c in ["cat icx", "env -u icx true", "", "X=1"] {
            assert!(!runs_icx(c), "{c}");
        }
    }
}
