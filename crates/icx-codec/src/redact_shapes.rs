// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later AND AGPL-3.0-or-later

//! Line shapes added in v1.5.1 (#104): credentials the key/value passes in `redact.rs` do not see
//! because the key and the value are not joined by `=` or `:`. They are markup elements and
//! attributes, name/value pairs, subscript and `=>` assignments, shell `set`, password-taking
//! CLI tools, URL userinfo and query parameters, and numeric PINs. Each pass runs before the
//! key/value passes, replaces only a value it can bound, and leaves existing markers alone, so
//! redaction stays idempotent. Every rule is scoped so that ambiguous input either stays as it
//! is (the v1.5.0 behaviour) or is hidden; none makes a v1.5.0 redaction visible.

use crate::redact::SecretRedactor as R;

const MARK: &str = "[REDACTED:SECRET]";
const MARK_CLI: &str = "[REDACTED:CLI_SECRET]";
const MARK_URL: &str = "[REDACTED:URL_SECRET]";

/// Runs every v1.5.1 line shape over `line`.
pub(crate) fn redact_line_shapes(line: &str) -> String {
    // Each pass needs a trigger character; most lines have none, and skip all of them.
    let has = |c: char| line.contains(c);
    let mut s = line.to_owned();
    if has('<') {
        s = xml_attribute_pairs(&xml_elements(&s));
    }
    if has('"') {
        s = json_name_value_pairs(&s);
    }
    if has('[') || line.contains("=>") {
        s = subscript_and_arrow(&s);
    }
    if line.contains("set") {
        s = shell_set(&s);
    }
    if has('-') || line.contains("<<<") {
        s = cli_shapes(&s);
    }
    if line.contains("://") {
        s = url_shapes(&s);
    }
    if may_hold_pin(line) {
        s = pin_values(&s);
    }
    s
}

/// A cheap pre-check for [`pin_values`]: a `pin` or `otp` spelling and a digit.
fn may_hold_pin(line: &str) -> bool {
    let b = line.as_bytes();
    b.iter().any(u8::is_ascii_digit)
        && b.windows(3).any(|w| {
            let w = [
                w[0].to_ascii_lowercase(),
                w[1].to_ascii_lowercase(),
                w[2].to_ascii_lowercase(),
            ];
            &w == b"pin" || &w == b"otp"
        })
}

/// The byte index just after the last character of `s` matching `pred` (0 when none), on a
/// character boundary.
fn after_last(s: &str, pred: impl Fn(char) -> bool) -> usize {
    s.char_indices()
        .rev()
        .find(|&(_, c)| pred(c))
        .map_or(0, |(k, c)| k + c.len_utf8())
}

/// Whether byte `at` falls inside an existing `[REDACTED:...]` marker.
fn inside_marker(s: &str, at: usize) -> bool {
    s[..at]
        .rfind("[REDACTED:")
        .is_some_and(|open| !s[open..at].contains(']'))
}

/// The v1.5.1 shapes that keep a JSON line valid JSON: they only replace string contents or
/// quote what they insert.
pub(crate) fn redact_json_shapes(line: &str) -> String {
    let mut s = json_name_value_pairs(line);
    if line.contains("://") {
        s = url_shapes(&s);
    }
    if may_hold_pin(line) {
        s = pin_values(&s);
    }
    s
}

/// Sorted ranges with overlapping or nested ones merged, latest start first, ready to replace
/// right to left (security review of v1.5.1: a nested range used to keep the outer one's edge).
fn merged_desc<T: Copy>(mut ranges: Vec<(usize, usize, T)>) -> Vec<(usize, usize, T)> {
    ranges.sort_by_key(|r| (r.0, std::cmp::Reverse(r.1)));
    let mut out: Vec<(usize, usize, T)> = Vec::with_capacity(ranges.len());
    for r in ranges {
        match out.last_mut() {
            Some(last) if r.0 < last.1 => last.1 = last.1.max(r.1),
            _ => out.push(r),
        }
    }
    out.reverse();
    out
}

/// Replaces `s[start..end]` with `marker` unless it is empty or already a marker.
fn hide(s: &mut String, start: usize, end: usize, marker: &str) -> usize {
    if start >= end
        || end > s.len()
        || !s.is_char_boundary(start)
        || !s.is_char_boundary(end)
        || s[start..end].contains("[REDACTED:")
        || inside_marker(s, start)
    {
        return end;
    }
    s.replace_range(start..end, marker);
    start + marker.len()
}

fn is_name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':')
}

/// The end of a quoted literal opened at `open` (index of the quote), honouring backslash
/// escapes; `None` when it does not close on this line.
#[allow(clippy::many_single_char_names)] // byte scanners: `s`, `b`, `i` as everywhere in redact.rs
fn literal_end(s: &str, open: usize) -> Option<usize> {
    let b = s.as_bytes();
    let q = *b.get(open)?;
    let mut i = open + 1;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            c if c == q => return Some(i),
            _ => i += 1,
        }
    }
    None
}

/// Hides the value that starts at `pos` (after optional spaces): a quoted literal loses its
/// contents, an unterminated one everything to the end of the line, an unquoted value the run
/// up to whitespace or `,;)]}`.
fn hide_value_at(s: &mut String, pos: usize, marker: &str) -> usize {
    let b = s.as_bytes();
    let mut i = pos;
    while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
        i += 1;
    }
    let Some(&c) = b.get(i) else {
        return i;
    };
    if matches!(c, b'"' | b'\'' | b'`') {
        return match literal_end(s, i) {
            Some(close) => hide(s, i + 1, close, marker) + 1,
            None => hide(s, i + 1, s.len(), marker),
        };
    }
    let end = s[i..]
        .find(|ch: char| {
            ch.is_whitespace() || matches!(ch, ',' | ';' | ')' | ']' | '}' | '"' | '\'' | '`')
        })
        .map_or(s.len(), |k| i + k);
    hide(s, i, end, marker)
}

/// `<password>secret</password>`, `<ApiKey attr="x">secret</ApiKey>`: the text of an element
/// whose name is a sensitive key.
fn xml_elements(line: &str) -> String {
    let mut s = line.to_owned();
    let mut pos = 0;
    while let Some(rel) = s[pos..].find('<') {
        let open = pos + rel;
        let b = s.as_bytes();
        let name_start = open + 1;
        let mut name_end = name_start;
        while name_end < b.len() && is_name_byte(b[name_end]) {
            name_end += 1;
        }
        // `Option<SecretString>` is a generic, not an element: an element follows the start of
        // the line, whitespace, another tag or a quote.
        let element_start = open == 0 || matches!(b[open - 1], b' ' | b'\t' | b'>' | b'"' | b'\'');
        if name_end == name_start || !b[name_start].is_ascii_alphabetic() || !element_start {
            pos = open + 1;
            continue;
        }
        let name = s[name_start..name_end].to_owned();
        let Some(gt_rel) = s[name_end..].find('>') else {
            break;
        };
        let gt = name_end + gt_rel;
        if s[..gt].ends_with('/') || !R::is_sensitive_key_name(&name) {
            pos = gt + 1;
            continue;
        }
        let close_tag = format!("</{name}");
        let Some(close_rel) = s[gt + 1..].find(&close_tag) else {
            // No closing tag on this line: the rest of the line is the value (fail closed).
            {
                let n = s.len();
                hide(&mut s, gt + 1, n, MARK)
            };
            break;
        };
        let close = gt + 1 + close_rel;
        pos = hide(&mut s, gt + 1, close, MARK) + close_tag.len();
    }
    s
}

/// Attributes of one tag on this line: `(name_lower, value_start, value_end)` for quoted values.
fn tag_attributes(tag: &str) -> Vec<(String, usize, usize)> {
    let b = tag.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_alphabetic() && (i == 0 || !is_name_byte(b[i - 1])) {
            let start = i;
            while i < b.len() && is_name_byte(b[i]) {
                i += 1;
            }
            let name = tag[start..i].to_ascii_lowercase();
            let mut j = i;
            while j < b.len() && b[j] == b' ' {
                j += 1;
            }
            if b.get(j) == Some(&b'=') {
                j += 1;
                while j < b.len() && b[j] == b' ' {
                    j += 1;
                }
                if matches!(b.get(j), Some(b'"' | b'\'')) {
                    if let Some(close) = literal_end(tag, j) {
                        out.push((name, j + 1, close));
                        i = close + 1;
                    }
                }
            }
        } else {
            i += 1;
        }
    }
    out
}

/// `<add key="Password" value="secret"/>`, `<param name="api_token" value="secret">`: the
/// `value` attribute of a tag whose `key` or `name` attribute is a sensitive key. A sensitive
/// attribute name (`password="secret"`) is left to the `=` pass.
fn xml_attribute_pairs(line: &str) -> String {
    let mut s = line.to_owned();
    let mut pos = 0;
    while let Some(rel) = s[pos..].find('<') {
        let open = pos + rel;
        let Some(gt_rel) = s[open..].find('>') else {
            break;
        };
        let gt = open + gt_rel;
        let tag = s[open..gt].to_owned();
        let attrs = tag_attributes(&tag);
        let named_sensitive = attrs.iter().any(|(n, a, z)| {
            matches!(n.as_str(), "key" | "name" | "id") && R::is_sensitive_key_name(&tag[*a..*z])
        });
        let mut next = gt + 1;
        if named_sensitive {
            // Right to left, so earlier offsets stay valid.
            for (n, a, z) in attrs.iter().rev() {
                if n == "value" {
                    let before = s.len();
                    hide(&mut s, open + a, open + z, MARK);
                    next = (next + s.len()).saturating_sub(before);
                }
            }
        }
        pos = next.min(s.len());
    }
    s
}

/// The quoted JSON strings on a line: `(content_start, content_end)`.
fn json_strings(s: &str) -> Vec<(usize, usize)> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'"' {
            if let Some(close) = literal_end(s, i) {
                out.push((i + 1, close));
                i = close + 1;
                continue;
            }
            break;
        }
        i += 1;
    }
    out
}

/// What follows the string ending at `end` (a quote), skipping spaces.
fn next_byte(s: &str, end: usize) -> Option<u8> {
    s.as_bytes()[end + 1..]
        .iter()
        .copied()
        .find(|c| *c != b' ' && *c != b'\t')
}

/// `{"name": "PASSWORD", "value": "secret"}` and `{"key": "API_KEY", "value": "secret"}`
/// (Kubernetes env lists, Terraform and CI variable exports): the `value` member of an object
/// whose `name` or `key` member names a sensitive key; and a JSON argv array element after a
/// sensitive long flag (`["--password", "secret"]`), or after `-p` when the array names a
/// password-taking tool.
fn json_name_value_pairs(line: &str) -> String {
    let mut s = line.to_owned();
    let strings = json_strings(&s);
    let mut targets: Vec<(usize, usize, &str)> = Vec::new();
    // Object members, delimited by the braces around each `"value"` member.
    for (idx, &(a, z)) in strings.iter().enumerate() {
        if !s[a..z].eq_ignore_ascii_case("value") || next_byte(&s, z) != Some(b':') {
            continue;
        }
        let Some(&(va, vz)) = strings.get(idx + 1) else {
            continue;
        };
        let obj_start = s[..a].rfind('{').unwrap_or(0);
        let obj_end = s[z..].find('}').map_or(s.len(), |k| z + k);
        let named = strings.windows(2).any(|w| {
            let (ka, kz) = w[0];
            let (xa, xz) = w[1];
            ka > obj_start
                && xz < obj_end
                && matches!(s[ka..kz].to_ascii_lowercase().as_str(), "name" | "key")
                && next_byte(&s, kz) == Some(b':')
                && R::is_sensitive_key_name(&s[xa..xz])
        });
        if named && va > z {
            targets.push((va, vz, MARK));
        }
    }
    // Argv arrays.
    let tool = strings
        .iter()
        .any(|&(a, z)| password_tool(&s[a..z]).is_some())
        || strings.iter().any(|&(a, z)| &s[a..z] == "login");
    for w in strings.windows(2) {
        let (fa, fz) = w[0];
        let (va, vz) = w[1];
        let flag = &s[fa..fz];
        let comma_between = s[fz + 1..va - 1].trim() == ",";
        let sensitive_flag =
            (flag.starts_with("--") && !flag.contains('=') && R::is_sensitive_key_name(flag))
                || (flag == "-p" && tool);
        if comma_between && sensitive_flag && !s[va..vz].starts_with('-') {
            targets.push((va, vz, MARK_CLI));
        }
    }
    for (a, z, m) in merged_desc(targets) {
        hide(&mut s, a, z, m);
    }
    s
}

/// `obj["password"] = "secret"`, `os.environ['API_KEY'] = "secret"`, and `=>` pairs
/// (`'password' => 'secret'`, `:token => "secret"`): a quoted literal value. Code on the right
/// (`= json!(...)`, `= all(...)`, a Rust match arm `"token" => {`) stays, as for other code.
#[allow(clippy::many_single_char_names)] // byte scanners: `s`, `b`, `i` as everywhere in redact.rs
fn subscript_and_arrow(line: &str) -> String {
    let mut s = line.to_owned();
    // Subscripts.
    let mut pos = 0;
    while let Some(rel) = s[pos..].find('[') {
        let open = pos + rel;
        let b = s.as_bytes();
        let Some(&q) = b.get(open + 1) else { break };
        if !matches!(q, b'"' | b'\'') {
            pos = open + 1;
            continue;
        }
        let Some(close) = literal_end(&s, open + 1) else {
            break;
        };
        let key = s[open + 2..close].to_owned();
        let mut j = close + 1;
        if s.as_bytes().get(j) != Some(&b']') {
            pos = open + 1;
            continue;
        }
        j += 1;
        let rest = &s[j..];
        let t = rest.trim_start();
        let at = j + (rest.len() - t.len());
        let assigns = t.starts_with('=') && !t.starts_with("==") && !t.starts_with("=>");
        let literal = t
            .get(1..)
            .is_some_and(|r| r.trim_start().starts_with(['"', '\'', '`']));
        if assigns && literal && R::is_sensitive_key_name(&key) {
            pos = hide_value_at(&mut s, at + 1, MARK);
        } else {
            pos = open + 1;
        }
    }
    // `=>`.
    let mut pos = 0;
    while let Some(rel) = s[pos..].find("=>") {
        let arrow = pos + rel;
        let head = s[..arrow].trim_end();
        let key = head
            .trim_end_matches(['"', '\''])
            .rsplit(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
            .next()
            .unwrap_or("");
        let literal = s[arrow + 2..].trim_start().starts_with(['"', '\'']);
        if literal && !key.is_empty() && R::is_sensitive_key_name(key) {
            pos = hide_value_at(&mut s, arrow + 2, MARK);
        } else {
            pos = arrow + 2;
        }
    }
    s
}

/// The shell segments of a line: byte ranges split at `;`, `&&`, `||` and `|` outside quotes.
fn segments(s: &str) -> Vec<(usize, usize)> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut start = 0;
    let mut quote: Option<u8> = None;
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if let Some(q) = quote {
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == q {
                quote = None;
            }
        } else if matches!(c, b'"' | b'\'') {
            quote = Some(c);
        } else if c == b';' || c == b'|' || (c == b'&' && b.get(i + 1) == Some(&b'&')) {
            out.push((start, i));
            let skip = usize::from(b.get(i + 1) == Some(&c));
            start = i + 1 + skip;
            i += skip;
        }
        i += 1;
    }
    out.push((start, b.len()));
    out
}

/// Whitespace-separated words of `s[a..z]`, without surrounding quotes: `(start, end)`.
#[allow(clippy::many_single_char_names)] // byte scanners: `s`, `b`, `i` as everywhere in redact.rs
fn words(s: &str, a: usize, z: usize) -> Vec<(usize, usize)> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = a;
    while i < z {
        while i < z && (b[i] == b' ' || b[i] == b'\t') {
            i += 1;
        }
        if i >= z {
            break;
        }
        let start = i;
        while i < z && b[i] != b' ' && b[i] != b'\t' {
            i += 1;
        }
        out.push((start, i));
    }
    out
}

/// `set PASSWORD secret`, `set -gx API_KEY secret` (fish), `setenv TOKEN secret` (csh): the rest
/// of the segment after a sensitive variable name.
#[allow(clippy::many_single_char_names)] // byte scanners: `s`, `b`, `i` as everywhere in redact.rs
fn shell_set(line: &str) -> String {
    let mut s = line.to_owned();
    for (a, z) in segments(line).into_iter().rev() {
        let w = words(&s, a, z);
        let Some(&(c0, c1)) = w.first() else { continue };
        let cmd = &s[c0..c1];
        if cmd != "set" && cmd != "setenv" {
            continue;
        }
        let mut k = 1;
        if cmd == "set" {
            while w.get(k).is_some_and(|&(x, _)| s.as_bytes()[x] == b'-') {
                k += 1;
            }
        }
        let (Some(&(n0, n1)), Some(&(v0, _))) = (w.get(k), w.get(k + 1)) else {
            continue;
        };
        let name = &s[n0..n1];
        // csh `set name = value` is left to the `=` pass; a command or variable substitution
        // (`set token (gh auth token)`, `set -x KEY $other`) is not the secret itself.
        let first = s.as_bytes()[v0];
        if matches!(first, b'=' | b'(' | b'$' | b'`') || !R::is_sensitive_key_name(name) {
            continue;
        }
        let end = s[..z].trim_end().len();
        hide(&mut s, v0, end, MARK);
    }
    s
}

/// The password-taking CLI tool a word names, by basename.
fn password_tool(word: &str) -> Option<&'static str> {
    let base = word
        .trim_matches(|c| c == '"' || c == '\'')
        .rsplit('/')
        .next()
        .unwrap_or("");
    [
        "mysql",
        "mysqldump",
        "mysqladmin",
        "mysqlimport",
        "mysqlcheck",
        "mysqlshow",
        "mariadb",
        "mariadb-dump",
        "mariadb-admin",
        "sshpass",
        "curl",
    ]
    .into_iter()
    .find(|t| *t == base)
}

/// Password-taking CLI shapes the generic `--flag value` pass cannot know:
/// - after a sensitive long flag, a value that starts with `-` (`--password -secret`);
/// - `-p` for `sshpass` and for `docker`/`podman`/`nerdctl` `login`, glued or spaced;
/// - glued `-psecret` for the MySQL and MariaDB clients (`-p secret` is a database name there);
/// - `curl -u user:secret` and `--user`/`--proxy-user user:secret`;
/// - the input of `--password-stdin`: a here-string (`<<< secret`) or `echo secret |`.
#[allow(clippy::many_single_char_names)] // byte scanners: `s`, `b`, `i` as everywhere in redact.rs
fn cli_shapes(line: &str) -> String {
    let mut s = line.to_owned();
    let segs = segments(line);
    let stdin_login = line.contains("--password-stdin");
    let mut targets: Vec<(usize, usize)> = Vec::new();
    for (idx, &(a, z)) in segs.iter().enumerate() {
        let w = words(&s, a, z);
        let word = |k: usize| {
            w.get(k)
                .map(|&(x, y)| s[x..y].trim_matches(|c| c == '"' || c == '\''))
        };
        let tool = w.iter().find_map(|&(x, y)| password_tool(&s[x..y]));
        let login = w.iter().any(|&(x, y)| &s[x..y] == "login")
            && w.iter().any(|&(x, y)| {
                matches!(
                    s[x..y].trim_start_matches('"').rsplit('/').next(),
                    Some("docker" | "podman" | "nerdctl" | "buildah" | "skopeo" | "helm")
                )
            });
        for k in 0..w.len() {
            let (x, y) = w[k];
            let tok = &s[x..y];
            let next = w.get(k + 1).copied();
            // `--password -secret`.
            if tok.starts_with("--")
                && !tok.contains('=')
                && R::is_sensitive_key_name(tok)
                && tok != "--password-stdin"
                && let Some((nx, ny)) = next
                && s.as_bytes()[nx] == b'-'
                && !s[nx..].starts_with("--")
            {
                targets.push((nx, ny));
            }
            // `-p` for sshpass and registry logins.
            if (tool == Some("sshpass") || login) && tok.starts_with("-p") {
                if tok == "-p" {
                    if let Some((nx, ny)) = next {
                        targets.push((nx, ny));
                    }
                } else if !tok.starts_with("-pa") || tool == Some("sshpass") {
                    targets.push((x + 2, y));
                }
            }
            // Glued `-psecret` for MySQL/MariaDB clients.
            if matches!(tool, Some(t) if t.starts_with("mysql") || t.starts_with("mariadb"))
                && tok.starts_with("-p")
                && tok.len() > 2
                && !tok.starts_with("--")
            {
                targets.push((x + 2, y));
            }
            // curl user:password.
            if tool == Some("curl") && matches!(tok, "-u" | "--user" | "-U" | "--proxy-user") {
                if let Some((nx, ny)) = next
                    && let Some(c) = s[nx..ny].find(':')
                {
                    targets.push((nx + c + 1, ny));
                }
            } else if tool == Some("curl") && tok.starts_with("-u") && tok.len() > 2 {
                if let Some(c) = tok.find(':') {
                    targets.push((x + c + 1, y));
                }
            }
            // `<<< secret` feeding --password-stdin.
            if stdin_login && tok == "<<<" {
                if let Some((nx, _)) = next {
                    targets.push((nx, z));
                }
            }
        }
        // `echo secret | ... --password-stdin`.
        if stdin_login
            && matches!(word(0), Some("echo" | "printf"))
            && segs
                .get(idx + 1)
                .is_some_and(|&(na, nz)| s[na..nz].contains("--password-stdin"))
            && w.len() > 1
        {
            let first = w.iter().skip(1).find(|&&(x, _)| s.as_bytes()[x] != b'-');
            if let Some(&(fx, _)) = first {
                targets.push((fx, s[..z].trim_end().len()));
            }
        }
    }
    let targets = merged_desc(targets.into_iter().map(|(x, y)| (x, y, ())).collect());
    for (x, y, ()) in targets {
        // Keep surrounding quotes of a quoted argument.
        let (mut x, mut y) = (x, y);
        let b = s.as_bytes();
        if y > x + 1 && matches!(b[x], b'"' | b'\'') && b[y - 1] == b[x] {
            x += 1;
            y -= 1;
        }
        hide(&mut s, x, y, MARK_CLI);
    }
    s
}

/// URL query and fragment parameters that carry a credential, besides sensitive key names.
fn is_url_secret_param(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    matches!(
        n.as_str(),
        "sig"
            | "signature"
            | "x-amz-signature"
            | "x-amz-security-token"
            | "x-amz-credential"
            | "x-goog-signature"
            | "x-goog-credential"
            | "code"
            | "access_token"
            | "refresh_token"
            | "id_token"
            | "client_secret"
            | "token"
            | "auth"
            | "apikey"
            | "api_key"
            | "key"
            | "password"
            | "passwd"
            | "pwd"
            | "secret"
    ) || R::is_sensitive_key_name(name)
}

/// Credential query or fragment parameters (`?sig=`, `&X-Amz-Signature=`, OAuth `?code=`, `#access_token=`).
fn url_shapes(line: &str) -> String {
    let mut s = line.to_owned();
    let mut pos = 0;
    while let Some(rel) = s[pos..].find("://") {
        let auth_start = pos + rel + 3;
        let url_end = s[auth_start..]
            .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | '`'))
            .map_or(s.len(), |k| auth_start + k);
        // Userinfo is the connection-URL pass's: it hides `user:password@` and token-shaped
        // userinfo, and keeps plain user names (`https://alice@host`) by design.
        if let Some(q) = s[auth_start..url_end]
            .find(['?', '#'])
            .map(|k| auth_start + k)
        {
            let mut params: Vec<(usize, usize, usize)> = Vec::new();
            let mut i = q + 1;
            while i < url_end {
                let end = s[i..url_end]
                    .find(['&', '#', ';'])
                    .map_or(url_end, |k| i + k);
                if let Some(eq) = s[i..end].find('=') {
                    params.push((i, i + eq, end));
                }
                i = end + 1;
            }
            // Right to left, so earlier offsets stay valid.
            for (a, eq, end) in params.into_iter().rev() {
                if is_url_secret_param(&s[a..eq]) {
                    hide(&mut s, eq + 1, end, MARK_URL);
                }
            }
        }
        match s[auth_start.min(s.len())..].find(char::is_whitespace) {
            Some(k) => pos = auth_start + k,
            None => break,
        }
    }
    s
}

/// A `pin` or `otp` key (`pin`, `pin2`, `user_pin`, `otp_code`, `smsOtp`; not `spin`).
fn is_pin_key(key: &str) -> bool {
    let k = key.trim_matches(|c| c == '"' || c == '\'');
    R::split_camel(k)
        .to_ascii_uppercase()
        .split(['_', '-', '.'])
        .map(|p| p.trim_end_matches(|c: char| c.is_ascii_digit()))
        .any(|p| matches!(p, "PIN" | "OTP" | "TOTP" | "HOTP"))
}

/// `pin: 1234`, `PIN=4821`, `"otp": "482913"`: a plain number of 4 to 10 digits under a PIN or
/// OTP key (owner decision for v1.5.1); `pin: 3.11` and words stay.
fn pin_values(line: &str) -> String {
    let mut s = line.to_owned();
    let b = line.as_bytes();
    let mut targets = Vec::new();
    for (i, &c) in b.iter().enumerate() {
        if c != b'=' && c != b':' {
            continue;
        }
        if b.get(i + 1) == Some(&b':') || (i > 0 && b[i - 1] == b':') || b.get(i + 1) == Some(&b'=')
        {
            continue;
        }
        let head = line[..i].trim_end().trim_end_matches(['"', '\'']);
        let key_start = after_last(head, |c: char| {
            !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
        });
        let key = &head[key_start..];
        if key.is_empty() || !is_pin_key(key) {
            continue;
        }
        let rest = &line[i + 1..];
        let t = rest.trim_start();
        let v0 = i + 1 + (rest.len() - t.len());
        let quoted = t.starts_with(['"', '\'']);
        let d0 = v0 + usize::from(quoted);
        let digits = line[d0..].bytes().take_while(u8::is_ascii_digit).count();
        let after = line[d0 + digits..].chars().next();
        let ends = match after {
            None => true,
            Some(ch) if quoted => ch == '"' || ch == '\'',
            Some(ch) => ch.is_whitespace() || matches!(ch, ',' | ';' | '}' | ']' | ')'),
        };
        if (4..=10).contains(&digits) && ends {
            let json_key = line[..i].trim_end().ends_with('"');
            targets.push((d0, d0 + digits, json_key && !quoted));
        }
    }
    for (a, z, quote_it) in targets.into_iter().rev() {
        // An unquoted JSON number becomes a quoted marker, so a JSON line stays valid.
        let marker = if quote_it {
            "\"[REDACTED:SECRET]\""
        } else {
            MARK
        };
        hide(&mut s, a, z, marker);
    }
    s
}

/// Indentation (spaces and tabs) of a line.
fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start_matches([' ', '\t']).len()
}

/// A YAML `key: value` line: `(column of the key, key, value start)`; a leading `- ` belongs
/// to the sequence, not the key.
fn yaml_member(line: &str) -> Option<(usize, &str, usize)> {
    let body = line.trim_end_matches(['\r', '\n']);
    let mut col = indent_of(body);
    if body[col..].starts_with("- ") {
        col += 2;
        col += indent_of(&body[col..]);
    }
    let rest = &body[col..];
    let colon = rest.find(':')?;
    let key = rest[..colon].trim().trim_matches(|c| c == '"' || c == '\'');
    if key.is_empty() || key.contains(char::is_whitespace) {
        return None;
    }
    let after = &rest[colon + 1..];
    if !(after.is_empty() || after.starts_with([' ', '\t'])) {
        return None;
    }
    let v = col + colon + 1 + (after.len() - after.trim_start().len());
    Some((col, key, v))
}

/// Hides the scalar value of a YAML line from byte `v` (keeping surrounding quotes).
fn hide_yaml_value(line: &str, v: usize) -> String {
    let mut s = line.to_owned();
    let body_end = s.trim_end_matches(['\r', '\n']).len();
    if v >= body_end {
        return s;
    }
    let value = &s[v..body_end];
    let (a, z) = match value.as_bytes()[0] {
        q @ (b'"' | b'\'') if value.len() >= 2 && value.as_bytes()[value.len() - 1] == q => {
            (v + 1, body_end - 1)
        }
        _ => (v, body_end),
    };
    hide(&mut s, a, z, "[REDACTED:CONFIG_SECRET]");
    s
}

/// Two YAML shapes that span lines (#104):
/// - a `name:`/`key:` member naming a sensitive key with a sibling `value:` (Kubernetes `env`,
///   CI variables): the sibling's value is hidden;
/// - a sensitive key with an empty value and more-indented children (`tokens:` followed by a
///   list): plain list items and `value:` members below it are hidden, up to the key-block
///   limit. Other mapping members keep their own per-key judgement.
pub(crate) fn redact_yaml_pairs(text: String) -> String {
    if !text.contains(':') || !text.contains('\n') {
        return text;
    }
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    // Only changed lines are copied; an unchanged text is returned as it is.
    let mut changed: std::collections::BTreeMap<usize, String> = std::collections::BTreeMap::new();
    for i in 0..lines.len() {
        // Only a `key:` line can open either shape.
        if !lines[i].contains(':') {
            continue;
        }
        let Some((col, key, v)) = yaml_member(lines[i]) else {
            continue;
        };
        let body = lines[i].trim_end_matches(['\r', '\n']);
        let value = body[v.min(body.len())..].trim();
        // Name/value siblings.
        if matches!(key.to_ascii_lowercase().as_str(), "name" | "key")
            && R::is_sensitive_key_name(value.trim_matches(|c| c == '"' || c == '\''))
        {
            for j in i + 1..lines.len().min(i + 12) {
                let l = lines[j];
                if l.trim().is_empty() {
                    continue;
                }
                let Some((c2, k2, v2)) = yaml_member(l) else {
                    break;
                };
                // A new sequence item or a shallower key ends the mapping.
                if c2 < col || (c2 == col && l.trim_start().starts_with("- ")) {
                    break;
                }
                if c2 == col && k2.eq_ignore_ascii_case("value") {
                    changed.insert(j, hide_yaml_value(lines[j], v2));
                    break;
                }
            }
        }
        // A sensitive block key: hide its children's values.
        if value.is_empty() && R::is_sensitive_key_name(key) {
            let base = indent_of(lines[i]);
            for (n, j) in (i + 1..lines.len()).enumerate() {
                let l = lines[j];
                if l.trim().is_empty() {
                    continue;
                }
                if indent_of(l) <= base || n >= 200 {
                    break;
                }
                // Mapping members keep their own per-key judgement (`username: alice` under
                // `credentials:` stays); a `value:` member and a plain list item are the secret.
                if let Some((_, k2, v2)) = yaml_member(l) {
                    if k2.eq_ignore_ascii_case("value") {
                        changed.insert(j, hide_yaml_value(lines[j], v2));
                    }
                } else if let Some(item) = l.trim_start().strip_prefix("- ") {
                    let v2 =
                        l.len() - l.trim_start().len() + 2 + (item.len() - item.trim_start().len());
                    changed.insert(j, hide_yaml_value(lines[j], v2));
                }
            }
        }
    }
    if changed.is_empty() {
        return text;
    }
    let mut out = String::with_capacity(text.len());
    for (k, line) in lines.iter().enumerate() {
        out.push_str(changed.get(&k).map_or(*line, String::as_str));
    }
    out
}
