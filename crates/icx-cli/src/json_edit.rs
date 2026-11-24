//! Formatting-preserving JSON edits for `icx install` (v1.4.1).
//!
//! The installer only ever adds: a member to an object, an element to an array, and both inside
//! new subtrees. So instead of re-serialising a user's config, which rewrites every line of a
//! file indented with tabs or aligned by hand, [`merge`] copies the original text byte for byte
//! and inserts only the new members and elements, indented like their siblings. A change that is
//! not a pure addition (a value replaced, a member removed or reordered) returns `None`, and the
//! caller falls back to [`reserialize`], which at least keeps the file's indent unit.

use crate::install::J;

/// A parsed JSON value with the byte range it occupies in the source text.
struct Node {
    start: usize,
    end: usize,
    kind: Kind,
}

enum Kind {
    Obj(Vec<Member>),
    Arr(Vec<Node>),
    Scalar,
}

struct Member {
    key: String,
    key_start: usize,
    key_end: usize,
    value: Node,
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn string(&mut self) -> Option<()> {
        if self.s.get(self.i) != Some(&b'"') {
            return None;
        }
        self.i += 1;
        while let Some(&c) = self.s.get(self.i) {
            self.i += 1;
            match c {
                b'\\' => self.i += 1,
                b'"' => return Some(()),
                _ => {}
            }
        }
        None
    }

    fn value(&mut self, depth: usize) -> Option<Node> {
        if depth > 128 {
            return None;
        }
        self.ws();
        let start = self.i;
        let kind = match *self.s.get(self.i)? {
            b'{' => {
                self.i += 1;
                let mut members = Vec::new();
                self.ws();
                if self.s.get(self.i) == Some(&b'}') {
                    self.i += 1;
                } else {
                    loop {
                        self.ws();
                        let key_start = self.i;
                        self.string()?;
                        let key_end = self.i;
                        let key: String =
                            serde_json::from_slice(&self.s[key_start..key_end]).ok()?;
                        self.ws();
                        if self.s.get(self.i) != Some(&b':') {
                            return None;
                        }
                        self.i += 1;
                        let value = self.value(depth + 1)?;
                        members.push(Member {
                            key,
                            key_start,
                            key_end,
                            value,
                        });
                        self.ws();
                        match *self.s.get(self.i)? {
                            b',' => self.i += 1,
                            b'}' => {
                                self.i += 1;
                                break;
                            }
                            _ => return None,
                        }
                    }
                }
                Kind::Obj(members)
            }
            b'[' => {
                self.i += 1;
                let mut items = Vec::new();
                self.ws();
                if self.s.get(self.i) == Some(&b']') {
                    self.i += 1;
                } else {
                    loop {
                        items.push(self.value(depth + 1)?);
                        self.ws();
                        match *self.s.get(self.i)? {
                            b',' => self.i += 1,
                            b']' => {
                                self.i += 1;
                                break;
                            }
                            _ => return None,
                        }
                    }
                }
                Kind::Arr(items)
            }
            b'"' => {
                self.string()?;
                Kind::Scalar
            }
            _ => {
                while self.i < self.s.len()
                    && !matches!(
                        self.s[self.i],
                        b',' | b'}' | b']' | b' ' | b'\t' | b'\n' | b'\r'
                    )
                {
                    self.i += 1;
                }
                if self.i == start {
                    return None;
                }
                Kind::Scalar
            }
        };
        Some(Node {
            start,
            end: self.i,
            kind,
        })
    }
}

/// The file's indent unit: a tab, or the smallest run of leading spaces; two spaces by default.
pub(crate) fn indent_unit(text: &str) -> String {
    let mut spaces = usize::MAX;
    for line in text.lines() {
        let lead: String = line
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();
        if lead.is_empty() || lead.len() == line.len() {
            continue;
        }
        if lead.starts_with('\t') {
            return "\t".to_owned();
        }
        spaces = spaces.min(lead.len());
    }
    if spaces == usize::MAX {
        "  ".to_owned()
    } else {
        " ".repeat(spaces)
    }
}

/// The leading whitespace of the line holding byte `at`, whatever follows it, or `None` for a
/// single-line document (kept compact). Used for an empty container's own indentation.
fn line_base(text: &str, at: usize) -> Option<String> {
    // A one-line file with a trailing newline is still one line (review of #94).
    if !text.trim_end_matches(['\r', '\n']).contains('\n') {
        return None;
    }
    let line_start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    Some(
        text[line_start..]
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect(),
    )
}

/// The whitespace that starts the line holding byte `at`, or `None` when something other than
/// whitespace comes before `at` on that line (a compact, single-line container).
fn line_indent(text: &str, at: usize) -> Option<String> {
    let line_start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    let lead = &text[line_start..at];
    lead.chars()
        .all(|c| c == ' ' || c == '\t')
        .then(|| lead.to_owned())
}

/// `v` as JSON with `unit` as the indent unit, every line after the first starting with `base`.
fn pretty(v: &J, unit: &str, base: &str) -> String {
    use serde::Serialize;
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(unit.as_bytes());
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    if v.serialize(&mut ser).is_err() {
        return "null".to_owned();
    }
    String::from_utf8_lossy(&buf).replace('\n', &format!("\n{base}"))
}

fn compact(v: &J) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "null".to_owned())
}

fn key_text(k: &str) -> String {
    serde_json::to_string(k).unwrap_or_else(|_| "\"\"".to_owned())
}

/// The whole document as `new`, re-serialised with the file's indent unit and its trailing-newline
/// convention; the fallback when [`merge`] cannot keep the original text.
pub(crate) fn reserialize(original: &str, new: &J) -> String {
    let unit = indent_unit(original);
    let mut out = pretty(new, &unit, "");
    if original.is_empty() || original.ends_with('\n') {
        out.push('\n');
    }
    with_line_endings(original, out)
}

/// `out` with CRLF line endings when every line of `original` ends in CRLF, so inserted lines
/// match the file (review of #94). Raw newlines cannot occur inside JSON strings, so converting
/// every `\n` changes only line breaks, and the original's are already CRLF.
fn with_line_endings(original: &str, out: String) -> String {
    let lf = original.matches('\n').count();
    if lf > 0 && lf == original.matches("\r\n").count() {
        out.replace("\r\n", "\n").replace('\n', "\r\n")
    } else {
        out
    }
}

/// `original` edited to hold `new`, when `new` only adds to `old` (the parse of `original`):
/// every existing byte is kept, and new members and elements are inserted after their last
/// sibling with the siblings' indentation. `None` when the change is not a pure addition or the
/// text does not parse.
pub(crate) fn merge(original: &str, old: &J, new: &J) -> Option<String> {
    let mut p = Parser {
        s: original.as_bytes(),
        i: 0,
    };
    let root = p.value(0)?;
    p.ws();
    if p.i != original.len() {
        return None;
    }
    // An empty root (`{}`, `[]`, as written by an editor or `echo '{}'`) has no style to keep:
    // its new content is laid out over several lines with the file's indent unit (#95).
    if original[root.start..root.end].len() == 2 {
        return Some(reserialize(original, new));
    }
    let unit = indent_unit(original);
    let mut out = String::with_capacity(original.len() + 256);
    out.push_str(&original[..root.start]);
    out.push_str(&emit(original, &unit, &root, old, new)?);
    out.push_str(&original[root.end..]);
    Some(with_line_endings(original, out))
}

fn emit(text: &str, unit: &str, node: &Node, old: &J, new: &J) -> Option<String> {
    if old == new {
        return Some(text[node.start..node.end].to_owned());
    }
    match (&node.kind, old, new) {
        (Kind::Obj(members), J::Obj(om), J::Obj(nm)) => {
            if members.len() != om.len() || nm.len() < om.len() {
                return None;
            }
            let mut out = String::new();
            let mut cursor = node.start;
            for (i, m) in members.iter().enumerate() {
                if m.key != om[i].0 || nm[i].0 != om[i].0 {
                    return None;
                }
                out.push_str(&text[cursor..m.value.start]);
                out.push_str(&emit(text, unit, &m.value, &om[i].1, &nm[i].1)?);
                cursor = m.value.end;
            }
            let added = &nm[om.len()..];
            if added.is_empty() {
                out.push_str(&text[cursor..node.end]);
                return Some(out);
            }
            let (indent, sep) = match members.last() {
                Some(last) => {
                    let sep = &text[last.key_end..last.value.start];
                    let sep = if sep.contains([' ', '\t']) { ": " } else { ":" };
                    (line_indent(text, last.key_start), sep)
                }
                // No sibling to copy: the colon style of the rest of the file.
                None => (
                    line_base(text, node.start).map(|base| format!("{base}{unit}")),
                    if text.contains("\":") && !text.contains("\": ") {
                        ":"
                    } else {
                        ": "
                    },
                ),
            };
            let entries: Vec<String> = added
                .iter()
                .map(|(k, v)| match &indent {
                    Some(ind) => format!("{}{sep}{}", key_text(k), pretty(v, unit, ind)),
                    None => format!("{}{sep}{}", key_text(k), compact(v)),
                })
                .collect();
            Some(insert_items(
                text,
                node,
                cursor,
                &out,
                !members.is_empty(),
                indent.as_deref(),
                unit,
                &entries,
            ))
        }
        (Kind::Arr(items), J::Arr(oa), J::Arr(na)) => {
            if items.len() != oa.len() || na.len() < oa.len() {
                return None;
            }
            let mut out = String::new();
            let mut cursor = node.start;
            for (i, item) in items.iter().enumerate() {
                out.push_str(&text[cursor..item.start]);
                out.push_str(&emit(text, unit, item, &oa[i], &na[i])?);
                cursor = item.end;
            }
            let added = &na[oa.len()..];
            if added.is_empty() {
                out.push_str(&text[cursor..node.end]);
                return Some(out);
            }
            let indent = match items.last() {
                Some(last) => line_indent(text, last.start),
                None => line_base(text, node.start).map(|base| format!("{base}{unit}")),
            };
            let entries: Vec<String> = added
                .iter()
                .map(|v| match &indent {
                    Some(ind) => pretty(v, unit, ind),
                    None => compact(v),
                })
                .collect();
            Some(insert_items(
                text,
                node,
                cursor,
                &out,
                !items.is_empty(),
                indent.as_deref(),
                unit,
                &entries,
            ))
        }
        _ => None,
    }
}

/// Appends `entries` to a container whose text up to `cursor` (just after its last existing item,
/// or its opening bracket when it had none) is already in `out`.
#[allow(clippy::too_many_arguments)]
fn insert_items(
    text: &str,
    node: &Node,
    cursor: usize,
    out: &str,
    had_items: bool,
    indent: Option<&str>,
    unit: &str,
    entries: &[String],
) -> String {
    let mut out = out.to_owned();
    if had_items {
        // After the last item: the new ones, then the original tail (whitespace and the bracket).
        for e in entries {
            match indent {
                Some(ind) => out.push_str(&format!(",\n{ind}{e}")),
                None => out.push_str(&format!(", {e}")),
            }
        }
        out.push_str(&text[cursor..node.end]);
    } else {
        // An empty container: its opening bracket, the items, then a closing bracket on its own
        // line at the container's own indentation.
        let open = &text[node.start..=node.start];
        let close = &text[node.end - 1..node.end];
        out.push_str(open);
        match indent {
            Some(ind) => {
                for (i, e) in entries.iter().enumerate() {
                    out.push_str(if i == 0 { "\n" } else { ",\n" });
                    out.push_str(ind);
                    out.push_str(e);
                }
                let base = ind.strip_suffix(unit).unwrap_or("");
                out.push('\n');
                out.push_str(base);
            }
            None => out.push_str(&entries.join(", ")),
        }
        out.push_str(close);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> J {
        serde_json::from_str(s).unwrap()
    }

    fn edit(original: &str, f: impl FnOnce(&mut J)) -> String {
        let old = parse(original);
        let mut new = old.clone();
        f(&mut new);
        let out = merge(original, &old, &new).expect("a pure addition merges");
        assert_eq!(
            parse(&out),
            new,
            "the merged text parses to the edited value"
        );
        out
    }

    #[test]
    fn tabs_and_aligned_values_are_kept_and_only_new_lines_appear() {
        let original = "{\n\t\"mcp\": {\n\t\t\"servers\": {\n\t\t\t\"a\": {\n\t\t\t\t\"pool\":     true,\n\t\t\t\t\"cwd\":  \"/x\"\n\t\t\t}\n\t\t}\n\t},\n\t\"model\":         \"m\"\n}\n";
        let out = edit(original, |v| {
            let servers = v.obj_mut("mcp").unwrap().obj_mut("servers").unwrap();
            servers.insert(
                "icx",
                parse(r#"{"type": "local", "command": ["icx", "mcp-serve"]}"#),
            );
        });
        let added: Vec<&str> = out.lines().filter(|l| !original.contains(*l)).collect();
        assert!(added.iter().all(|l| l.starts_with("\t\t\t")), "{out}");
        assert!(
            out.contains("\t\t\t\t\"pool\":     true,"),
            "alignment kept"
        );
        assert!(
            out.contains("\t\t\t},\n\t\t\t\"icx\": {\n\t\t\t\t\"type\": \"local\""),
            "{out}"
        );
        assert!(out.ends_with("}\n"));
        // Every original line is still there.
        for l in original.lines().filter(|l| *l != "\t\t\t}") {
            assert!(out.contains(l), "lost {l:?}");
        }
    }

    #[test]
    fn arrays_get_an_element_after_their_last_one() {
        let original = "{\n  \"permissions\": {\n    \"allow\": [\n      \"Shell(ls)\"\n    ],\n    \"deny\": []\n  }\n}";
        let out = edit(original, |v| {
            let allow = v.obj_mut("permissions").unwrap().arr_mut("allow").unwrap();
            allow.push(J::Str("Mcp(icx:icx)".into()));
        });
        assert_eq!(
            out,
            "{\n  \"permissions\": {\n    \"allow\": [\n      \"Shell(ls)\",\n      \"Mcp(icx:icx)\"\n    ],\n    \"deny\": []\n  }\n}"
        );
    }

    #[test]
    fn empty_containers_and_new_top_level_keys() {
        let original = "{\n    \"deny\": [],\n    \"hooks\": {}\n}\n";
        let out = edit(original, |v| {
            v.arr_mut("deny").unwrap().push(J::Str("x".into()));
            v.obj_mut("hooks").unwrap().insert("Stop", parse("[1]"));
            v.insert("version", parse("1"));
        });
        assert_eq!(
            out,
            "{\n    \"deny\": [\n        \"x\"\n    ],\n    \"hooks\": {\n        \"Stop\": [\n            1\n        ]\n    },\n    \"version\": 1\n}\n"
        );
    }

    #[test]
    fn compact_json_stays_compact() {
        let original = r#"{"mcpServers":{"a":{"command":"x"}}}"#;
        let out = edit(original, |v| {
            v.obj_mut("mcpServers")
                .unwrap()
                .insert("icx", parse(r#"{"command":"icx"}"#));
        });
        assert_eq!(
            out,
            r#"{"mcpServers":{"a":{"command":"x"}, "icx":{"command":"icx"}}}"#
        );
    }

    #[test]
    fn a_change_that_is_not_an_addition_is_refused() {
        let original = "{\"a\": 1, \"b\": 2}";
        let old = parse(original);
        for new in [r#"{"a": 2, "b": 2}"#, r#"{"b": 2, "a": 1}"#, r#"{"a": 1}"#] {
            assert!(merge(original, &old, &parse(new)).is_none(), "{new}");
        }
        assert!(merge("{\"a\": 1} trailing", &old, &old).is_none());
    }

    #[test]
    fn strings_with_escapes_and_brackets_parse() {
        let original = "{\n  \"cmd\": \"a \\\"]}\\\" b\",\n  \"list\": [\"x,y\"]\n}\n";
        let out = edit(original, |v| {
            v.arr_mut("list").unwrap().push(J::Str("z".into()));
        });
        assert!(out.contains("\"cmd\": \"a \\\"]}\\\" b\","), "{out}");
    }

    #[test]
    fn the_indent_unit_follows_the_file() {
        assert_eq!(indent_unit("{\n\t\"a\": 1\n}"), "\t");
        assert_eq!(
            indent_unit("{\n    \"a\": {\n        \"b\": 1\n    }\n}"),
            "    "
        );
        assert_eq!(indent_unit("{}"), "  ");
        assert_eq!(
            reserialize("{\n\t\"a\": 1\n}\n", &parse("{\"a\": [1]}")),
            "{\n\t\"a\": [\n\t\t1\n\t]\n}\n"
        );
    }

    #[test]
    fn one_line_files_crlf_files_and_tab_colons_keep_their_style() {
        // A compact file with a trailing newline stays compact (review of #94).
        let out = edit("{\"mcpServers\":{}}\n", |v| {
            v.obj_mut("mcpServers").unwrap().insert("icx", parse("1"));
        });
        assert_eq!(out, "{\"mcpServers\":{\"icx\":1}}\n");
        // CRLF files get CRLF lines.
        let crlf = "{\r\n  \"a\": [\r\n    1\r\n  ]\r\n}\r\n";
        let out = edit(crlf, |v| v.arr_mut("a").unwrap().push(parse("{\"b\": 2}")));
        assert_eq!(
            out.matches('\n').count(),
            out.matches("\r\n").count(),
            "{out:?}"
        );
        assert!(
            out.starts_with("{\r\n  \"a\": [\r\n    1,\r\n    {"),
            "{out:?}"
        );
        // A colon followed by a tab is spaced like its siblings.
        let out = edit("{\n  \"a\":\t1\n}\n", |v| v.insert("b", parse("2")));
        assert!(out.contains("\"b\": 2"), "{out:?}");
    }

    #[test]
    fn malformed_or_unsupported_text_is_refused() {
        let new = parse("{\"a\": 1}");
        for bad in [
            "",
            "{\"a\" 1}",
            "{\"a\": 1,}",
            "{\"a\": 1 \"b\": 2}",
            "[1 2]",
            "[1,",
            "{\"a\": \"unterminated}",
            "{1: 2}",
            "{\"a\": }",
        ] {
            assert!(merge(bad, &parse("{}"), &new).is_none(), "{bad:?}");
        }
        // Too deep to parse: refused, not a stack overflow.
        let deep = format!("{}{}", "[".repeat(200), "]".repeat(200));
        assert!(merge(&deep, &parse("[]"), &parse("[1]")).is_none());
    }

    #[test]
    fn nested_additions_inside_arrays_and_unchanged_containers() {
        let original = "[\n  {\"a\": [1]},\n  [2]\n]\n";
        let out = edit(original, |v| {
            if let J::Arr(items) = v {
                if let J::Obj(m) = &mut items[0] {
                    m[0].1 = parse("[1, 3]");
                }
                items.push(parse("4"));
            }
        });
        assert_eq!(out, "[\n  {\"a\": [1, 3]},\n  [2],\n  4\n]\n");
    }

    #[test]
    fn an_empty_root_gets_a_multi_line_layout() {
        for (original, end) in [("{}", ""), ("{}\n", "\n"), ("{}\r\n", "\r\n")] {
            let out = edit(original, |v| v.insert("a", parse("[1]")));
            let want =
                "{\n  \"a\": [\n    1\n  ]\n}".to_owned() + if end.is_empty() { "" } else { "\n" };
            let want = if end == "\r\n" {
                want.replace('\n', "\r\n")
            } else {
                want
            };
            assert_eq!(out, want, "{original:?}");
        }
    }
}
