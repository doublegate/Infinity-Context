//! A small POSIX-style command-line reader, shared by `icx install` (recognising its own hooks)
//! and the Hermes adapter (recognising icx's own commands). It is not a shell: it only splits
//! words and finds the program, so a mistake can cost at most a missed or duplicated match.

/// Splits `command` into words: single quotes are literal, double quotes allow `\"`, `\\`, `\$`
/// and `` \` `` escapes, and a backslash outside quotes escapes the next character. So
/// `'/it'\''s/icx'` is the one word `/it's/icx`. Separators (`;`, `|`, `&`) are ordinary
/// characters here; [`command_segments`] splits at them. Both use one lexer, so they cannot
/// disagree on quoting (review of #92).
#[must_use]
pub fn shell_words(command: &str) -> Vec<String> {
    lex(command, false).into_iter().flatten().collect()
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

/// The simple commands of a command line, split at `&&`, `||`, `;`, `|` and `&` outside quotes
/// (spaced or not), each as its words. A separator inside quotes (`grep "|" x`) is an argument.
/// A shell wrapper's `-c` script (`sh -c "..."`) is read as commands too, at most 8 levels deep.
#[must_use]
pub fn command_segments(command: &str) -> Vec<Vec<String>> {
    segments_at(command, 0)
}

/// One pass over `command`: words as [`shell_words`] reads them, with each separator outside
/// quotes ending the current simple command. Separators are recognised by the scanner itself,
/// never marked in the text, so no input character can fake one (review of #92).
fn split_simple_commands(command: &str) -> Vec<Vec<String>> {
    lex(command, true)
}

/// The shared lexer. With `split`, `&&`, `||`, `;`, `|`, `&` and an unquoted newline end a
/// simple command (a multi-line script is several commands, review of #92); without it they are
/// word characters, and a newline is whitespace. A backslash before a newline (or CRLF), outside
/// quotes or inside double quotes, is a line continuation and joins the lines.
fn lex(command: &str, split: bool) -> Vec<Vec<String>> {
    let mut segments = vec![Vec::new()];
    let mut cur = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    let mut chars = command.chars().peekable();
    let end_word = |cur: &mut String, in_word: &mut bool, segments: &mut Vec<Vec<String>>| {
        if *in_word {
            if let Some(seg) = segments.last_mut() {
                seg.push(std::mem::take(cur));
            }
            *in_word = false;
        }
    };
    while let Some(c) = chars.next() {
        match quote {
            Some('\'') if c == '\'' => quote = None,
            Some('\'') => cur.push(c),
            Some(_) if c == '"' => quote = None,
            Some(_) if c == '\\' => match chars.peek().copied() {
                Some(n) if matches!(n, '"' | '\\' | '$' | '`') => {
                    cur.push(n);
                    chars.next();
                }
                Some('\n') => {
                    chars.next();
                }
                Some('\r') if chars.clone().nth(1) == Some('\n') => {
                    chars.next();
                    chars.next();
                }
                _ => cur.push(c),
            },
            Some(_) => cur.push(c),
            None if c == '\\' => match chars.next() {
                Some('\n') => {}
                Some('\r') if chars.peek() == Some(&'\n') => {
                    chars.next();
                }
                Some(n) => {
                    cur.push(n);
                    in_word = true;
                }
                None => in_word = true,
            },
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                in_word = true;
            }
            None if split && matches!(c, ';' | '|' | '&' | '\n') => {
                end_word(&mut cur, &mut in_word, &mut segments);
                if matches!(c, '|' | '&') && chars.peek() == Some(&c) {
                    chars.next();
                }
                segments.push(Vec::new());
            }
            None if c.is_whitespace() => end_word(&mut cur, &mut in_word, &mut segments),
            None => {
                cur.push(c);
                in_word = true;
            }
        }
    }
    end_word(&mut cur, &mut in_word, &mut segments);
    segments.retain(|s| !s.is_empty());
    segments
}

fn segments_at(command: &str, depth: usize) -> Vec<Vec<String>> {
    let mut expanded = Vec::new();
    for seg in split_simple_commands(command) {
        let shell = program_index(&seg).filter(|&i| {
            seg[i]
                .rsplit('/')
                .next()
                .is_some_and(|p| matches!(p, "sh" | "bash" | "zsh" | "dash" | "fish"))
        });
        let script = shell.filter(|_| depth < 8).and_then(|i| {
            seg[i + 1..]
                .iter()
                .position(|w| w.starts_with('-') && !w.starts_with("--") && w.contains('c'))
                .and_then(|j| seg.get(i + 2 + j))
                .cloned()
        });
        match script {
            Some(inner) => expanded.extend(segments_at(&inner, depth + 1)),
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

    /// Single-quotes `s` for the shell (test helper).
    fn crate_quote(s: &str) -> String {
        format!("'{}'", s.replace('\'', "'\\''"))
    }

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
        // Nested wrapper scripts are read at most 8 levels deep.
        let mut nested = "icx deref x".to_owned();
        for _ in 0..12 {
            nested = format!("sh -c {}", crate_quote(&nested));
        }
        assert!(!runs_icx(&nested), "deeper than 8 levels is not unwrapped");
        let mut shallow = "icx deref x".to_owned();
        for _ in 0..3 {
            shallow = format!("sh -c {}", crate_quote(&shallow));
        }
        assert!(runs_icx(&shallow));
        assert!(runs_icx(r#"echo "a\"b\"c"&&icx deref x"#), "escaped quote");
        for c in [
            "cat icx",
            "env -u icx true",
            "",
            "X=1",
            "echo 'icx deref'",
            r#"grep "|" icx"#,
            "echo ';' icx deref",
            "echo \"\u{1}x\" icx deref",
            r#"FOO="a; icx deref x" ls"#,
            r#"grep --regexp="a|icx" file"#,
        ] {
            assert!(!runs_icx(c), "{c}");
        }
    }

    #[test]
    fn newlines_end_commands_and_continuations_join_lines() {
        assert!(runs_icx("cd /repo\nicx deref '#icx:1'"));
        assert!(runs_icx("sh -c \"cd /repo\nicx deref x\""));
        assert_eq!(
            command_segments("echo a\necho b"),
            [vec!["echo", "a"], vec!["echo", "b"]]
        );
        // A quoted newline stays inside its word; a continuation joins the lines.
        assert_eq!(command_segments("echo 'a\nb'"), [vec!["echo", "a\nb"]]);
        assert_eq!(command_segments("echo a \\\n b"), [vec!["echo", "a", "b"]]);
        assert!(!runs_icx("echo icx\nls"));
        assert_eq!(
            command_segments("echo a \\\r\n b"),
            [vec!["echo", "a", "b"]]
        );
        // Inside double quotes: a continuation (LF or CRLF) joins, another backslash stays.
        assert_eq!(shell_words("\"a\\\nb\""), ["ab"]);
        assert_eq!(shell_words("\"a\\\r\nb\""), ["ab"]);
        assert_eq!(shell_words("\"a\\qb\""), ["a\\qb"]);
        // A trailing backslash still ends a word.
        assert_eq!(shell_words("a \\"), ["a", ""]);
        // shell_words keeps separators as word characters and reads quotes the same way.
        assert_eq!(shell_words("a;b \"c\\\"d\"\ne"), ["a;b", "c\"d", "e"]);
    }
}
