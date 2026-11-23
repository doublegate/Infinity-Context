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

/// The simple commands of a command line, split at `&&`, `||`, `;` and `|` (spaced or not, outside
/// quotes), each as its words. A shell wrapper's `-c` script (`sh -c "..."`) is read as commands
/// too.
#[must_use]
pub fn command_segments(command: &str) -> Vec<Vec<String>> {
    // Separators outside quotes are spaced first, so they become words of their own.
    let mut spaced = String::with_capacity(command.len() + 8);
    let mut quote: Option<char> = None;
    let mut chars = command.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) if c == q => quote = None,
            None if c == '\'' || c == '"' => quote = Some(c),
            None if c == '\\' => {
                spaced.push(c);
                if let Some(n) = chars.next() {
                    spaced.push(n);
                }
                continue;
            }
            None if matches!(c, ';' | '|' | '&') => {
                spaced.push(' ');
                spaced.push(c);
                if matches!(chars.peek(), Some(&n) if n == c && c != ';') {
                    spaced.push(c);
                    chars.next();
                }
                spaced.push(' ');
                continue;
            }
            // Inside quotes, and ordinary characters outside them.
            _ => {}
        }
        spaced.push(c);
    }
    let mut out = Vec::new();
    let mut cur = Vec::new();
    for w in shell_words(&spaced) {
        if matches!(w.as_str(), "&&" | "||" | ";" | "|" | "&") {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(w);
        }
    }
    out.push(cur);
    let mut expanded = Vec::new();
    for seg in out.into_iter().filter(|s| !s.is_empty()) {
        let shell = program_index(&seg).filter(|&i| {
            seg[i]
                .rsplit('/')
                .next()
                .is_some_and(|p| matches!(p, "sh" | "bash" | "zsh" | "dash" | "fish"))
        });
        let script = shell.and_then(|i| {
            seg[i + 1..]
                .iter()
                .position(|w| w.starts_with('-') && !w.starts_with("--") && w.contains('c'))
                .and_then(|j| seg.get(i + 2 + j))
                .cloned()
        });
        match script {
            Some(inner) => expanded.extend(command_segments(&inner)),
            None => expanded.push(seg),
        }
    }
    expanded
}

/// Whether `command` runs icx itself (`icx` or the `icx-hook` wrapper, at any path) in any of
/// its simple commands (`cd /repo && icx deref ...`, `icx deref x | head`).
#[must_use]
pub fn runs_icx(command: &str) -> bool {
    command_segments(command).iter().any(|seg| {
        program_index(seg)
            .and_then(|i| seg[i].rsplit('/').next())
            .is_some_and(|p| p == "icx" || p == "icx-hook")
    })
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
        for c in [
            "cd /repo && icx deref --raw '#icx:1'",
            "icx deref x|head -n 5",
            "sh -c 'cd /p; icx hook --agent agy'",
            "bash -lc \"icx slice x\"",
        ] {
            assert!(runs_icx(c), "{c}");
        }
        assert_eq!(command_segments("a 'b && c' && d").len(), 2);
        for c in ["cat icx", "env -u icx true", "", "X=1", "echo 'icx deref'"] {
            assert!(!runs_icx(c), "{c}");
        }
    }
}
