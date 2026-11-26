//! 4-Tier Pre-Ingestion Secret Redaction Gate, Shannon Character Entropy Filter, and AHTL
//! Delimiter Escaping (`SecretRedactor`).
//!
//! Reference: `docs/ahtl/05-structural-ast-skeletonization-and-redaction.md` §3,
//! `docs/security-and-threat-model.md` §2.

use std::collections::BTreeMap;

/// Sanitizes untrusted text before CAS chunking, HDC indexing, AHTL encoding, or `/dev/shm` broadcast.
#[derive(Clone, Debug, Default)]
pub struct SecretRedactor;

impl SecretRedactor {
    /// Model-facing sanitization: [`Self::redact`] followed by Tier-0 delimiter escaping, so
    /// untrusted text can neither leak secrets nor forge AHTL packets/headers.
    pub fn sanitize(input: &str) -> String {
        Self::escape_ahtl_delimiters(&Self::redact(input))
    }

    /// Storage-grade redaction (v1.1.0, ICX-AUD-021): removes credentials, private-key blocks and
    /// high-entropy secrets while preserving everything else byte-for-byte (line endings,
    /// trailing newline, and literal delimiters). For secret-free input the result equals the
    /// input, which is what makes CAS dereferencing lossless.
    pub fn redact(input: &str) -> String {
        // Unicode look-alikes (#104): a no-break or ideographic space, a zero-width character
        // or soft hyphen inside a key, a full-width `＝` or `：`. Redaction runs on an ASCII
        // normalisation of such text, and each line it changes is taken from that result;
        // every other line keeps its original bytes.
        if !input.chars().any(Self::is_lookalike) {
            return Self::redact_text(input);
        }
        // Two passes, the second over the first's result, so whatever either hides is hidden
        // (security reviews of v1.5.1, #106 and #110). Full-width quote-like characters (`＂`,
        // `＇`, `＼`, `｀`) are quotes to a reader of plain text, so outside JSON lines they are
        // made ASCII before the first pass: no pass can then consume an opening quote before
        // another reads it as one. On a JSON line they are string content and stay. The second
        // pass normalises every other look-alike. A line where neither pass hid anything keeps
        // its original bytes.
        let quoted: String = input
            .split_inclusive('\n')
            .map(|line| {
                if line.chars().any(Self::is_quote_like)
                    && !Self::is_json_line(line.trim_end())
                    && Self::embedded_json_start(line).is_none()
                {
                    line.chars().map(Self::ascii_quote).collect::<String>()
                } else {
                    line.to_owned()
                }
            })
            .collect();
        let plain = Self::redact_text(&quoted);
        let keep_quotes: String = plain
            .chars()
            .filter_map(Self::normalize_keep_quotes)
            .collect();
        // Lines of the first pass with no look-alike are already final for the second.
        let fixed: std::collections::HashSet<&str> = plain
            .split('\n')
            .map(|l| l.strip_suffix('\r').unwrap_or(l))
            .filter(|l| !l.chars().any(Self::is_lookalike))
            .collect();
        let second = Self::redact_text_skipping(&keep_quotes, &fixed);
        // `split`, not `split_inclusive`: a last line holding only removed characters
        // (`"\u{FEFF}"`) must still pair with its empty counterpart (review of #106).
        let pl: Vec<&str> = plain.split('\n').collect();
        let ql: Vec<&str> = second.split('\n').collect();
        let xl: Vec<&str> = input.split('\n').collect();
        let wl: Vec<&str> = quoted.split('\n').collect();
        if pl.len() != ql.len() {
            return second;
        }
        // An unchanged line pairs with its original at the same index, or at the same distance
        // from the end when a collapsed key block shortened the text, and only where the
        // quote-normalised texts are equal: a wrong pairing can only pick an identical line.
        let shift = wl.len().checked_sub(pl.len());
        let original = |k: usize| -> Option<&str> {
            [Some(k), shift.map(|d| k + d)]
                .into_iter()
                .flatten()
                .find(|&i| wl.get(i) == Some(&pl[k]))
                .and_then(|i| xl.get(i).copied())
        };
        let mut out = String::with_capacity(input.len());
        for k in 0..pl.len() {
            if k > 0 {
                out.push('\n');
            }
            let first_holds = pl[k]
                .chars()
                .filter_map(Self::normalize_keep_quotes)
                .eq(ql[k].chars());
            // Nothing hidden on this line by either pass: the original bytes.
            let line = if first_holds {
                original(k).unwrap_or(pl[k])
            } else {
                ql[k]
            };
            out.push_str(line);
        }
        out
    }

    /// A full-width quote-like character: `＂`, `＇`, `＼` or `｀`.
    fn is_quote_like(c: char) -> bool {
        matches!(c, '\u{FF02}' | '\u{FF07}' | '\u{FF3C}' | '\u{FF40}')
    }

    /// The ASCII form of a full-width quote-like character; every other character as it is.
    fn ascii_quote(c: char) -> char {
        if Self::is_quote_like(c) {
            char::from_u32(c as u32 - 0xFEE0).unwrap_or(c)
        } else {
            c
        }
    }

    /// [`Self::normalize_char`] without the quote-like characters (`＂`, `＇`, `＼`, `｀`).
    fn normalize_keep_quotes(c: char) -> Option<char> {
        match c {
            '\u{FF02}' | '\u{FF07}' | '\u{FF3C}' | '\u{FF40}' => Some(c),
            _ => Self::normalize_char(c),
        }
    }

    /// A character that redaction treats as its ASCII look-alike (or ignores).
    fn is_lookalike(c: char) -> bool {
        Self::normalize_char(c) != Some(c)
    }

    /// The ASCII form of a look-alike: no-break and other Unicode spaces become a space,
    /// zero-width characters and the soft hyphen disappear, full-width ASCII becomes ASCII.
    fn normalize_char(c: char) -> Option<char> {
        match c {
            '\u{00A0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}' => Some(' '),
            '\u{00AD}' | '\u{200B}'..='\u{200D}' | '\u{2060}' | '\u{FEFF}' => None,
            '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - 0xFEE0),
            _ => Some(c),
        }
    }

    fn redact_text(input: &str) -> String {
        crate::redact_shapes::redact_yaml_pairs(Self::redact_lines(input, None))
    }

    /// [`Self::redact_text`] that copies through, unchanged, any line in `fixed` met with no
    /// multi-line value or key block open: lines an earlier pass already redacted, which a
    /// second pass leaves as they are (idempotence), so the look-alike pass only does the line
    /// rules on the lines it normalised (#108).
    fn redact_text_skipping(input: &str, fixed: &std::collections::HashSet<&str>) -> String {
        crate::redact_shapes::redact_yaml_pairs(Self::redact_lines(input, Some(fixed)))
    }

    fn redact_lines(input: &str, fixed: Option<&std::collections::HashSet<&str>>) -> String {
        let mut out = String::with_capacity(input.len());
        let mut block: Option<KeyBlock> = None;
        let mut block_lines = 0usize;
        let mut pending_value: Option<PendingSecret> = None;
        for segment in input.split_inclusive('\n') {
            let (mut body, ending) = split_line_ending(segment);
            // YAML block contents are scalars even when they resemble mappings, comments or
            // private-key banners. Handle them before any of the line-oriented detectors.
            // Inside an open quoted scalar every line is secret up to the closing quote.
            if let Some(q) = pending_value.as_ref().and_then(|p| p.quote) {
                if body.trim().is_empty() {
                    out.push_str(body);
                    out.push_str(ending);
                    continue;
                }
                let content_start = body.len() - body.trim_start().len();
                let triple = pending_value.as_ref().is_some_and(|p| p.triple);
                let close = if triple {
                    let closer = [q; 3];
                    let closer = std::str::from_utf8(&closer).unwrap_or("\"\"\"");
                    find_unescaped(&body[content_start..], closer).map(|i| i + 2)
                } else {
                    closing_quote(&body.as_bytes()[content_start..], q)
                };
                let Some(close) = close else {
                    out.push_str(&Self::redact_bare_value_line(body));
                    out.push_str(ending);
                    continue;
                };
                // Emit the hidden value and its closing quote, then classify the rest of the
                // line like any other line: it can open a key block, a new block or quoted
                // secret, or hold further assignments.
                // The opening quote went with the first line's marker (first-quote rule), so the
                // closing quote goes with this one: no unmatched quote is left (#33).
                out.push_str(&body[..content_start]);
                out.push_str("[REDACTED:SECRET]");
                body = &body[content_start + close + 1..];
                pending_value = None;
                if body.is_empty() {
                    out.push_str(ending);
                    continue;
                }
            }
            // Backslash, bracket and plain-scalar continuations (red-team review of #103).
            if let Some(p) = pending_value.as_mut()
                && (p.backslash || p.brackets > 0 || p.plain)
            {
                if body.trim().is_empty() {
                    // A blank line ends a backslash continuation (review of #103).
                    if p.backslash {
                        pending_value = None;
                    }
                    out.push_str(body);
                    out.push_str(ending);
                    continue;
                }
                if p.plain {
                    let indent = body.len() - body.trim_start().len();
                    p.lines += 1;
                    if indent > p.indent && p.lines <= MAX_KEY_BLOCK_LINES {
                        out.push_str(&Self::redact_bare_value_line(body));
                        out.push_str(ending);
                        continue;
                    }
                    pending_value = None;
                } else {
                    p.lines += 1;
                    if p.brackets > 0 {
                        // Hide up to the bracket that closes the value and keep that bracket,
                        // so a second pass sees the value close again. The rest of the line is
                        // classified like any other line (security review: it was written
                        // verbatim, so a secret after the bracket survived).
                        let scan = scan_value(body, p.brackets, p.bracket_quote, p.bracket_triple);
                        let indent_of_key = p.indent;
                        p.sticky |= scan.ambiguous_quote;
                        if let Some(at) = scan.close.filter(|_| !p.sticky) {
                            let (inside, after) = body.split_at(at);
                            if inside.trim().is_empty() {
                                out.push_str(inside);
                            } else {
                                out.push_str(&Self::redact_bare_value_line(inside));
                            }
                            let close_len = after.chars().next().map_or(1, char::len_utf8);
                            out.push_str(&after[..close_len]);
                            body = &after[close_len..];
                            pending_value = None;
                            // What follows the bracket is still the value unless it ends the
                            // statement or is a comment (`) + "part2"`; review of #103).
                            let t = body.trim_start();
                            if !(t.is_empty()
                                || t.starts_with(['#', ';', ',', ')', ']', '}'])
                                || t.starts_with("//")
                                || Self::is_marker_run(t))
                            {
                                out.push_str(&body[..body.len() - t.len()]);
                                out.push_str("[REDACTED:SECRET]");
                                // The tail may open the next part of the value (`) + (`,
                                // `) + \`; review of #103).
                                let tail = scan_value(t, 0, None, false);
                                if tail.depth > 0 || tail.ambiguous_quote {
                                    // A possibly quoted tail stays pending to the line cap, as
                                    // on a first line (security review of #103).
                                    pending_value = Some(PendingSecret {
                                        indent: indent_of_key,
                                        brackets: tail.depth.max(1),
                                        bracket_quote: tail.quote,
                                        bracket_triple: tail.triple,
                                        sticky: tail.ambiguous_quote,
                                        ..PendingSecret::default()
                                    });
                                } else if let Some(q) =
                                    tail.quote.and_then(|q| u8::try_from(q).ok())
                                {
                                    // `) + """` or `` ) + ` ``: the tail opens a string.
                                    let rb = t.as_bytes();
                                    let at = tail.quote_at.unwrap_or(0);
                                    let start = if at >= 2 && rb[at - 1] == q && rb[at - 2] == q {
                                        at - 2
                                    } else {
                                        at
                                    };
                                    pending_value = Some(PendingSecret {
                                        indent: indent_of_key,
                                        quote: Some(q),
                                        triple: start != at,
                                        ..PendingSecret::default()
                                    });
                                } else if ends_with_continuation(t) {
                                    pending_value = Some(PendingSecret {
                                        indent: indent_of_key,
                                        backslash: true,
                                        ..PendingSecret::default()
                                    });
                                }
                                body = "";
                            }
                            if body.is_empty() {
                                out.push_str(ending);
                                continue;
                            }
                        } else {
                            p.brackets = scan.depth.max(1);
                            p.bracket_quote = scan.quote;
                            p.bracket_triple = scan.triple;
                            out.push_str(&Self::redact_bare_value_line(body));
                            if p.lines >= MAX_KEY_BLOCK_LINES {
                                pending_value = None;
                            }
                            out.push_str(ending);
                            continue;
                        }
                    } else {
                        if !ends_with_continuation(body) || p.lines >= MAX_KEY_BLOCK_LINES {
                            pending_value = None;
                        }
                        out.push_str(&Self::redact_bare_value_line(body));
                        out.push_str(ending);
                        continue;
                    }
                }
            }
            if let Some(pending) = pending_value.as_mut() {
                let indent = body.len() - body.trim_start().len();
                if body.trim().is_empty() {
                    out.push_str(body);
                    out.push_str(ending);
                    continue;
                }
                if pending.block || pending.scalar_indent.is_some() {
                    // Block scalars keep the indentation of their first content line. A plain
                    // multi-line scalar only needs its lines indented past the key.
                    let minimum = if pending.block {
                        pending.scalar_indent.unwrap_or(pending.indent + 1)
                    } else {
                        pending.indent + 1
                    };
                    if indent >= minimum {
                        if pending.block {
                            pending.scalar_indent = Some(minimum);
                        }
                        out.push_str(&Self::redact_bare_value_line(body));
                        out.push_str(ending);
                        continue;
                    }
                    pending_value = None;
                }
            }
            if let Some(kind) = block {
                block_lines += 1;
                if let Some(end) = kind.end_index_after(body, 0) {
                    block = None;
                    // Text after the END banner goes through full processing below (it may start
                    // another key block or hold an inline one).
                    body = &body[end..];
                    if body.is_empty() {
                        continue;
                    }
                } else if block_lines < MAX_KEY_BLOCK_LINES {
                    continue;
                } else {
                    // Runaway/unterminated block: stop swallowing and resume normal processing.
                    block = None;
                }
            }
            // A line that is a complete JSON object or array is redacted structurally (v1.3.1,
            // #29): the values of sensitive keys are hidden and everything else is kept, instead
            // of hiding the rest of the line from the first quote.
            // A line opening a private-key block keeps the key-block handling below.
            if pending_value.is_none()
                // Every key block on the line must end on it inside one JSON string (typed
                // PEM/PGP banners, no quote between BEGIN and END), so the structural path sees
                // the whole block and no block state is owed to the following lines.
                && key_blocks_within_strings(body)
                && let Some(json) =
                    Self::redact_json_line(body).or_else(|| Self::redact_json_fields(body))
            {
                let mut shaped = crate::redact_shapes::redact_json_shapes(&json);
                for _ in 0..4 {
                    let next = crate::redact_shapes::redact_json_shapes(&shaped);
                    if next == shaped {
                        break;
                    }
                    shaped = next;
                }
                out.push_str(&shaped);
                out.push_str(ending);
                continue;
            }
            // Private-key normalization must also pass through pending scalar processing.
            // Otherwise a key banner after a scalar prefix hides that prefix only on the
            // second redaction pass. Keep block state so subsequent key lines stay hidden.
            let normalized = if pending_value.is_none()
                && block.is_none()
                && fixed.is_some_and(|f| f.contains(body))
            {
                body.to_owned()
            } else if let Some(inline) = Self::redact_inline_key_blocks(body) {
                Self::redact_line(&inline)
            } else if let Some((kind, start)) = KeyBlock::begins(body) {
                block = Some(kind);
                block_lines = 0;
                // Text before the banner on the same line is not part of the key.
                let mut prefix = Self::redact_line(&body[..start]);
                prefix.push_str(kind.marker());
                prefix
            } else {
                Self::redact_line(body)
            };
            // Classification uses the same canonical line that will be emitted. Vendor-token
            // substitution must not change a structure into a scalar only on a second pass.
            let redacted = if let Some(pending) = pending_value.as_mut() {
                match ContinuationLine::classify(&normalized) {
                    // A comment between the key and its value keeps the value pending.
                    ContinuationLine::Comment => normalized,
                    // A nested mapping or structure: the key had no scalar value to hide.
                    ContinuationLine::Structure => {
                        pending_value = None;
                        normalized
                    }
                    ContinuationLine::Value => {
                        let indent = body.len() - body.trim_start().len();
                        if indent > pending.indent {
                            pending.scalar_indent = Some(indent);
                        } else {
                            pending_value = None;
                        }
                        Self::redact_bare_value_line(&normalized)
                    }
                }
            } else {
                normalized
            };
            // A definite multi-line `=` value (`api_key: str = """`) wins over the YAML reading
            // of the same line (review of #103).
            // An empty right-hand side after an annotation (`password: str =`) counts too: the
            // YAML reading of that line would only hide indented lines (#104).
            // Arm from the original line, or else from the emitted one: a second pass sees the
            // emitted line as its original, so a quote this pass removed must not change the
            // decision (idempotence, v1.5.1).
            let arm = |line: &str| {
                Self::assignment_continuation(line)
                    .filter(|p| {
                        p.quote.is_some()
                            || p.brackets > 0
                            || p.backslash
                            || line.trim_end().ends_with('=')
                    })
                    .or_else(|| Self::sensitive_continuation(line))
                    .or_else(|| Self::assignment_continuation(line))
            };
            if let Some(pending) =
                arm(body).or_else(|| (redacted != body).then(|| arm(&redacted)).flatten())
            {
                pending_value = Some(pending);
            }
            out.push_str(&redacted);
            out.push_str(ending);
        }
        out
    }

    /// Whether `line` is exactly one JSON object or array (surrounding whitespace allowed).
    fn is_json_line(line: &str) -> bool {
        let start = line.len() - line.trim_start().len();
        if !line[start..].starts_with(['{', '[']) {
            return false;
        }
        let mut scan = JsonScan {
            s: line.as_bytes(),
            edits: Vec::new(),
        };
        scan.value(start, false, 0, None)
            .is_some_and(|end| line[end..].trim().is_empty())
    }

    /// Structural redaction of a line that is exactly one JSON object or array (surrounding
    /// whitespace allowed), per RFC 8259 (v1.3.1, #29). The value of a sensitive key, of any type,
    /// becomes `"[REDACTED:CONFIG_SECRET]"`; every other string (keys included) is redacted on its
    /// own with [`Self::redact`]. Edits are made in place, so formatting and key order are kept.
    /// Returns `None` when the line is not such JSON, nests deeper than [`MAX_JSON_DEPTH`], or
    /// an edit would leave invalid JSON. The caller then applies the line rules (fail closed).
    fn redact_json_line(line: &str) -> Option<String> {
        let start = line.len() - line.trim_start().len();
        if !line[start..].starts_with(['{', '[']) {
            return None;
        }
        let mut scan = JsonScan {
            s: line.as_bytes(),
            edits: Vec::new(),
        };
        let end = scan.value(start, false, 0, None)?;
        if !line[end..].trim().is_empty() {
            return None;
        }
        if scan.edits.is_empty() {
            return Some(line.to_owned());
        }
        let mut out = line.to_owned();
        for (a, b, text) in scan.edits.into_iter().rev() {
            out.replace_range(a..b, &text);
        }
        // The edited line must still be one valid JSON value.
        let mut check = JsonScan {
            s: out.as_bytes(),
            edits: Vec::new(),
        };
        let check_start = out.len() - out.trim_start().len();
        let check_end = check.value(check_start, true, 0, None)?;
        out[check_end..].trim().is_empty().then_some(out)
    }

    /// A line that is a list of JSON object members without its braces (`"k": v, "k2": v2`, with
    /// an optional trailing comma), as in a pretty-printed object or a log fragment, redacted
    /// structurally like a whole object: only sensitive values are hidden (ADR-0011 rule 5).
    /// `None` unless the members parse strictly.
    /// A line that ends with a complete JSON object or array after other text: that JSON part
    /// redacted structurally, the prefix unchanged. `None` when there is no such part or the
    /// structural pass changes nothing.
    fn redact_embedded_json(line: &str) -> Option<String> {
        let t = line.trim_end();
        if !(t.ends_with('}') || t.ends_with(']')) {
            return None;
        }
        let at = Self::embedded_json_start(line)?;
        let json = Self::redact_json_line(&line[at..])?;
        let shaped = crate::redact_shapes::redact_json_shapes(&json);
        if shaped == line[at..] {
            return None;
        }
        Some(format!("{}{shaped}", &line[..at]))
    }

    /// Where the JSON object or array that ends `line` begins, when it begins after other text:
    /// one backward pass from the final `}` or `]` matches brackets, skipping JSON strings (a
    /// JSON string holds no unescaped quote), so one candidate is checked and the cost stays
    /// linear (review of #110).
    fn embedded_json_start(line: &str) -> Option<usize> {
        let t = line.trim_end();
        let b = t.as_bytes();
        let last = *b.last()?;
        if !matches!(last, b'}' | b']') {
            return None;
        }
        let escaped = |k: usize| {
            !b[..k]
                .iter()
                .rev()
                .take_while(|c| **c == b'\\')
                .count()
                .is_multiple_of(2)
        };
        let mut stack: Vec<u8> = Vec::new();
        let mut k = b.len();
        while k > 0 {
            k -= 1;
            match b[k] {
                b'"' if !escaped(k) => {
                    // Back to the string's opening quote.
                    loop {
                        if k == 0 {
                            return None;
                        }
                        k -= 1;
                        if b[k] == b'"' && !escaped(k) {
                            break;
                        }
                    }
                }
                c @ (b'}' | b']') => stack.push(c),
                c @ (b'{' | b'[') => {
                    let want = if c == b'{' { b'}' } else { b']' };
                    if stack.pop() != Some(want) {
                        return None;
                    }
                    if stack.is_empty() {
                        return (k > 0 && Self::is_json_line(&line[k..])).then_some(k);
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn redact_json_fields(line: &str) -> Option<String> {
        let start = line.len() - line.trim_start().len();
        let end = line.trim_end().len();
        if start >= end || !line[start..].starts_with('"') {
            return None;
        }
        let inner_end = if line[..end].ends_with(',') {
            end - 1
        } else {
            end
        };
        let wrapped = format!("{{{}}}", &line[start..inner_end]);
        let json = Self::redact_json_line(&wrapped)?;
        let inner = json.strip_prefix('{')?.strip_suffix('}')?;
        Some(format!("{}{inner}{}", &line[..start], &line[inner_end..]))
    }

    /// Redacts BEGIN..END private-key spans that start and end on the same line (e.g. a key
    /// embedded in a JSON string with `\n` escapes). Returns `None` if the line has none.
    fn redact_inline_key_blocks(line: &str) -> Option<String> {
        let mut out = line.to_owned();
        let mut changed = false;
        for kind in [KeyBlock::Pem, KeyBlock::Pgp] {
            while let Some(start) = kind.begin_index(&out) {
                let Some(end) = kind.end_index_after(&out, start) else {
                    break;
                };
                out.replace_range(start..end, kind.marker());
                changed = true;
            }
        }
        changed.then_some(out)
    }

    /// True if the line ends with a sensitive key whose value continues on the next line
    /// (`"password":` / `password:`).
    /// Only a line that *is* the key (optionally after `{`, `,` or a YAML `- `) arms the
    /// continuation, so prose such as `error: missing signing key:` never does (PR #20 review).
    /// A sensitive `KEY=value` / `key = value` assignment whose value goes on past its line
    /// (red-team review of #103): nothing after `=` (the value is on the next line), an
    /// unterminated quote, triple quote or backtick, a trailing backslash, or an unclosed
    /// bracket.
    fn assignment_continuation(line: &str) -> Option<PendingSecret> {
        // A stray apostrophe or label before the `=` (`don't log; password = """`,
        // `'outer: let api_key = (`) hides it from a quote-aware scan, so a second scan reads
        // apostrophes as ordinary characters; it can only arm more continuations (#104). Any
        // apostrophe gets it (#107: `don't log; password = '''` balances the apostrophes and
        // leaves none open). Double quotes and backticks stay quoted in both scans, so
        // `note = "password = (" ; don't` keeps its `=` inside the string, and Markdown code
        // spans stay quoted.
        Self::assignment_continuation_scan(line, true).or_else(|| {
            line.contains('\'')
                .then(|| Self::assignment_continuation_scan(line, false))?
        })
    }

    /// `track_apostrophes: false` reads `'` as an ordinary character (an apostrophe in text).
    fn assignment_continuation_scan(line: &str, track_apostrophes: bool) -> Option<PendingSecret> {
        let b = line.as_bytes();
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
                i += 1;
                continue;
            }
            match c {
                b'\'' if i > 0 && matches!(b[i - 1], b'&' | b'<') => {}
                b'"' | b'`' => quote = Some(c),
                b'\'' if track_apostrophes => quote = Some(c),
                b'=' => {
                    let prev = if i > 0 { b[i - 1] } else { 0 };
                    let next = b.get(i + 1).copied().unwrap_or(0);
                    if !matches!(prev, b'=' | b'!' | b'<' | b'>') && !matches!(next, b'=' | b'>') {
                        // `password := ...` (Go, Python walrus): the colon belongs to the operator.
                        let head = line[..i]
                            .trim_end_matches([' ', '\t'])
                            .trim_end_matches(':')
                            .trim_end_matches([' ', '\t']);
                        let key_start = head
                            .rfind(|c: char| {
                                !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
                            })
                            .map_or(0, |k| {
                                k + head[k..].chars().next().map_or(1, char::len_utf8)
                            });
                        let mut key = &head[key_start..];
                        // `api_key: str = ...`, `const password: string = ...`: the name is
                        // before the annotation's colon (review of #103).
                        // `api_key: str = ...`, `let a: u8, api_key: &str = ...`: a name before
                        // any annotation colon (not `::`) may be the key (review of #103).
                        if !Self::is_sensitive_key_name(key) {
                            let hb = head.as_bytes();
                            for c in (0..hb.len()).filter(|&k| {
                                hb[k] == b':'
                                    && hb.get(k + 1) != Some(&b':')
                                    && (k == 0 || hb[k - 1] != b':')
                            }) {
                                let name = head[..c].trim_end();
                                let start = name
                                    .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                                    .map_or(0, |k| {
                                        k + name[k..].chars().next().map_or(1, char::len_utf8)
                                    });
                                if Self::is_sensitive_key_name(&name[start..]) {
                                    key = &name[start..];
                                    break;
                                }
                            }
                        }
                        if !key.is_empty() && Self::is_sensitive_key_name(key) {
                            // Only this assignment's own value: up to a `;` that starts the
                            // next statement (`token = 1; password = \"\"\"`).
                            let rest = &line[i + 1..];
                            let stmt_end = scan_value(rest, 0, None, false)
                                .stmt_end
                                .unwrap_or(rest.len());
                            let rhs_full = rest[..stmt_end].trim();
                            // A value the value-span rule already hid to the end of its line ends
                            // with one of its markers, which may have taken a closing bracket or
                            // quote with it: such a value does not arm, so a second pass decides
                            // like the first (idempotence). A planted or other marker, or text
                            // after one, still arms (security review of v1.5.1).
                            if [
                                "[REDACTED:ENV_SECRET]",
                                "[REDACTED:CONFIG_SECRET]",
                                "[REDACTED:CLI_SECRET]",
                            ]
                            .iter()
                            .any(|m| rhs_full.ends_with(m))
                            {
                                i += 1;
                                continue;
                            }
                            // String prefixes (`r`, `b`, `f`, ...) and Rust raw-string hashes.
                            let rhs = rhs_full
                                .trim_start_matches(|c: char| {
                                    matches!(c, 'r' | 'R' | 'b' | 'B' | 'u' | 'U' | 'f' | 'F')
                                })
                                .trim_start_matches('#')
                                .trim_start();
                            let rhs = if rhs.starts_with(['"', '\'', '`']) {
                                rhs
                            } else {
                                rhs_full
                            };
                            let base = PendingSecret {
                                indent: line.len() - line.trim_start().len(),
                                ..PendingSecret::default()
                            };
                            if rhs.is_empty() {
                                return Some(base);
                            }
                            // Quotes and brackets first: a backslash inside them does not end
                            // the value on the next line (review of #103).
                            for t in ["\"\"\"", "'''"] {
                                if let Some(r) = rhs.strip_prefix(t)
                                    && find_unescaped(r, t).is_none()
                                {
                                    return Some(PendingSecret {
                                        quote: Some(t.as_bytes()[0]),
                                        triple: true,
                                        ..base
                                    });
                                }
                            }
                            if rhs.starts_with('`') && !rhs[1..].contains('`') {
                                return Some(PendingSecret {
                                    quote: Some(b'`'),
                                    ..base
                                });
                            }
                            // An open bracket, with any quote opened inside it carried along
                            // (`f("` then lines then `")`).
                            let scan = scan_value(rhs_full, 0, None, false);
                            if scan.depth > 0 || scan.ambiguous_quote {
                                return Some(PendingSecret {
                                    brackets: scan.depth.max(1),
                                    bracket_quote: scan.quote,
                                    bracket_triple: scan.triple,
                                    sticky: scan.ambiguous_quote,
                                    ..base
                                });
                            }
                            if let Some(q) = unterminated_quote(rhs) {
                                return Some(PendingSecret {
                                    quote: Some(q),
                                    ..base
                                });
                            }
                            // A quote opened later in the value (`"a" + "`, `prefix + """`),
                            // after an operator or space so an apostrophe does not count.
                            if let (Some(q), Some(at)) = (scan.quote, scan.quote_at)
                                && let Ok(q) = u8::try_from(q)
                                && let rb = rhs_full.as_bytes()
                                // The scanner reads `"""` as `""` then `"`: step back to the
                                // triple's first quote.
                                && let start = if at >= 2 && rb[at - 1] == q && rb[at - 2] == q {
                                    at - 2
                                } else {
                                    at
                                }
                                // Before the quote: the start of the value, or an operator or space,
                                // allowing a string prefix (`r"""`, `f"`, `b'`). A backtick may
                                // follow an identifier (a tagged template, `String.raw\``): it has
                                // no apostrophe ambiguity (#104).
                                && let pre = rhs_full[..start].trim_end_matches(|c: char| {
                                    matches!(c, 'r' | 'R' | 'b' | 'B' | 'u' | 'U' | 'f' | 'F')
                                })
                                && (pre.is_empty()
                                    || q == b'`'
                                    || pre.chars().next_back().is_some_and(|b| {
                                        b.is_whitespace() || "+(=,[{".contains(b)
                                    }))
                            {
                                let triple = start != at || scan.triple;
                                return Some(PendingSecret {
                                    quote: Some(q),
                                    triple,
                                    ..base
                                });
                            }
                            if ends_with_continuation(rhs) {
                                return Some(PendingSecret {
                                    backslash: true,
                                    ..base
                                });
                            }
                            // This value ends on its line; a later sensitive assignment on the
                            // same line may still continue (review of #103).
                        }
                    }
                }
                _ => {}
            }
            i += 1;
        }
        None
    }

    fn sensitive_continuation(line: &str) -> Option<PendingSecret> {
        let declaration = Self::declaration(line);
        let colon = Self::scan_unquoted(declaration).1?;
        let (head, suffix) = (&declaration[..colon], &declaration[colon + 1..]);
        let suffix = suffix.trim();
        let mut properties = suffix.split_whitespace().peekable();
        while properties.peek().is_some_and(|s| s.starts_with(['&', '!'])) {
            properties.next();
        }
        let header = properties.next().unwrap_or("");
        let block = yaml_block_header(header) && properties.next().is_none();
        // A quoted scalar that does not close on this line continues on the next ones.
        let quote = unterminated_quote(suffix);
        // A plain scalar continues on lines indented past the key (red-team review of #103:
        // `password: true` then an indented secret line).
        let plain = !suffix.is_empty()
            && !block
            && quote.is_none()
            && !suffix.starts_with(['{', '[', '"', '\'', '`', '|', '>', '#']);
        if !suffix.is_empty() && !block && quote.is_none() && !plain {
            return None;
        }
        let head = head.trim();
        let split = head
            .rfind(|c: char| c.is_whitespace() || c == '{' || c == ',')
            .map_or(0, |i| {
                i + head[i..].chars().next().map_or(1, char::len_utf8)
            });
        let (prefix, key) = head.split_at(split);
        let prefix = prefix.trim();
        let prefix_ok =
            prefix.is_empty() || prefix == "-" || prefix.ends_with('{') || prefix.ends_with(',');
        let key = key.trim_matches(|c: char| c == '"' || c == '\'');
        // This must be a standalone key, not prose or an inline value that happens to end
        // in `_TOKEN:`. Substitution can remove whitespace inside that value; accepting its
        // punctuation as part of a key would arm a new continuation only on the next pass.
        if !key
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-'))
        {
            return None;
        }
        let indent = line.len() - line.trim_start().len()
            + usize::from(line.trim_start().starts_with("- ")) * 2;
        let scalar_indent = block
            .then(|| {
                header
                    .chars()
                    .find_map(|c| c.to_digit(10))
                    .map(|n| indent + n as usize)
            })
            .flatten();
        (prefix_ok && !key.is_empty() && Self::is_sensitive_key_name(key)).then_some(
            PendingSecret {
                indent,
                block,
                scalar_indent,
                quote,
                plain,
                ..PendingSecret::default()
            },
        )
    }

    /// The line without a trailing ` # comment`, right-trimmed. A `#` inside a quoted value
    /// is not a comment.
    fn declaration(line: &str) -> &str {
        let (comment, _) = Self::scan_unquoted(line);
        comment.map_or(line, |i| &line[..i]).trim_end()
    }

    /// Byte index in `line` of the last `:` outside quotes, which introduces its last value or
    /// block header (a `:` inside a quoted value, even an unterminated one, is not a separator).
    fn declaration_colon(line: &str) -> Option<usize> {
        Self::scan_unquoted(Self::declaration(line)).1
    }

    /// One pass over `line` tracking YAML/JSON quoting: returns the byte index of the first
    /// unquoted ` #` comment marker and of the last unquoted `:`. A quote opens only at the
    /// start of a value (line start, or after whitespace, `:`, `,`, `[`, `{` or `=`), so an
    /// apostrophe inside prose does not; `\"` escapes inside double quotes and `''` inside
    /// single quotes.
    fn scan_unquoted(line: &str) -> (Option<usize>, Option<usize>) {
        let bytes = line.as_bytes();
        let mut quote: Option<u8> = None;
        let mut comment = None;
        let mut colon = None;
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            match quote {
                Some(b'"') if b == b'\\' => i += 1,
                Some(b'\'') if b == b'\'' && bytes.get(i + 1) == Some(&b'\'') => i += 1,
                Some(q) if b == q => quote = None,
                Some(_) => {}
                None => match b {
                    b'"' | b'\''
                        if i == 0
                            || matches!(
                                bytes[i - 1],
                                b' ' | b'\t' | b':' | b',' | b'[' | b'{' | b'='
                            ) =>
                    {
                        quote = Some(b);
                    }
                    b'#' if i > 0 && bytes[i - 1].is_ascii_whitespace() => {
                        comment = Some(i);
                        break;
                    }
                    b':' => colon = Some(i),
                    _ => {}
                },
            }
            i += 1;
        }
        (comment, colon)
    }

    /// Replaces the (possibly quoted) value on a continuation line, keeping indentation and any
    /// trailing comma.
    fn redact_bare_value_line(line: &str) -> String {
        let indent_len = line.len() - line.trim_start().len();
        let (indent, rest) = line.split_at(indent_len);
        let rest_trim = rest.trim_end();
        let (core, trailing_comma) = match rest_trim.strip_suffix(',') {
            Some(c) => (c, ","),
            None => (rest_trim, ""),
        };
        let quoted = core.len() >= 2
            && ((core.starts_with('"') && core.ends_with('"'))
                || (core.starts_with('\'') && core.ends_with('\'')));
        if quoted {
            let q = &core[..1];
            format!("{indent}{q}[REDACTED:SECRET]{q}{trailing_comma}")
        } else {
            format!("{indent}[REDACTED:SECRET]{trailing_comma}")
        }
    }

    /// Escapes literal `⟪`, `⟫`, `⟦`, `⟧` characters in untrusted inputs (`Tier 0`).
    pub fn escape_ahtl_delimiters(input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        for ch in input.chars() {
            match ch {
                '⟪' => out.push_str("<<"),
                '⟫' => out.push_str(">>"),
                '⟦' => out.push_str("[["),
                '⟧' => out.push_str("]]"),
                _ => out.push(ch),
            }
        }
        out
    }

    /// Computes the Shannon character entropy $H(S)$ in bits per character (`Tier 3`):
    /// $$H(S) = -\sum_{c \in \Sigma(S)} \frac{\operatorname{count}(c)}{|S|} \log_2 \left(\frac{\operatorname{count}(c)}{|S|}\right)$$
    pub fn shannon_entropy(s: &str) -> f64 {
        if s.is_empty() {
            return 0.0;
        }
        if s.is_ascii() {
            let mut counts = [0u32; 128];
            for &b in s.as_bytes() {
                counts[b as usize] += 1;
            }
            let len_f = s.len() as f64;
            let mut entropy = 0.0f64;
            for &count in &counts {
                if count > 0 {
                    let p = (count as f64) / len_f;
                    entropy -= p * p.log2();
                }
            }
            return entropy;
        }
        let mut counts: BTreeMap<char, usize> = BTreeMap::new();
        let mut total = 0usize;
        for ch in s.chars() {
            *counts.entry(ch).or_insert(0) += 1;
            total += 1;
        }
        let len_f = total as f64;
        let mut entropy = 0.0f64;
        for &count in counts.values() {
            let p = (count as f64) / len_f;
            entropy -= p * p.log2();
        }
        entropy
    }

    pub(crate) fn is_sensitive_key_name(raw_key: &str) -> bool {
        let trimmed = raw_key
            .trim()
            .trim_start_matches("export ")
            .trim_start_matches('-')
            .trim_matches(|c: char| c == '"' || c == '\'');
        // camelCase and PascalCase split into words (`accessToken`, `APIKey`; red-team review
        // of #103), so they match like their snake_case spellings.
        let key_upper = Self::split_camel(trimmed)
            .to_ascii_uppercase()
            .replace('-', "_");
        if Self::is_token_count_key(&key_upper) {
            return false;
        }
        let sensitive_suffixes = [
            "SECRET",
            "CLIENT_SECRET",
            "SECRET_KEY",
            "ACCESS_KEY",
            "SECRET_ACCESS_KEY",
            "SESSION_TOKEN",
            "PASSWORD",
            "PASSWD",
            "API_KEY",
            "APIKEY",
            "AUTH_TOKEN",
            "ACCESS_TOKEN",
            "REFRESH_TOKEN",
            "PRIVATE_KEY",
            "BEARER_TOKEN",
            "TOKEN",
            // v1.1.0 (ICX-AUD-015)
            "ENCRYPTION_KEY",
            "SIGNING_KEY",
            "MASTER_KEY",
            "CLIENT_KEY",
            "HMAC_KEY",
            "SSH_KEY",
            "PASS",
            "PASSPHRASE",
            "CREDENTIAL",
            "CREDENTIALS",
            "DSN",
            "COOKIE",
            "SESSION_ID",
            "SESSIONID",
        ];
        let matches = |key: &str| {
            sensitive_suffixes
                .iter()
                .any(|s| key == *s || key.ends_with(&format!("_{s}")))
        };
        // Plural collection keys (`api_keys`, `tokens`, `passwords`) hold the same secrets.
        // `PASS` is excluded: its plural `PASSES` is ordinary prose/config (`max_passes`).
        let suffix_match = |k: &str| {
            matches(k)
                || k.strip_suffix('S')
                    .is_some_and(|singular| !singular.ends_with("PASS") && matches(singular))
        };
        if suffix_match(&key_upper) {
            return true;
        }
        // A numbered key (`API_KEY_2`, `token1`) is the same key.
        let base = key_upper
            .trim_end_matches(|c: char| c.is_ascii_digit())
            .trim_end_matches('_');
        if base != key_upper && !base.is_empty() && suffix_match(base) {
            return true;
        }
        Self::is_qualified_secret_key(trimmed, base)
    }

    /// Credential words glued to a prefix (`PGPASSWORD`, `AUTHTOKEN`, `SECRETKEY`) or followed by
    /// a qualifier (`password_value`, `secret_old`, `api_key_confirmation`), unless what follows
    /// names something that is not a secret (`token_type`, `password_file`, `secret_name`); and
    /// `Authorization`, a bare `auth`, and a mixed- or lower-case `pwd` (connection strings), not
    /// the shell's `PWD` (red-team review of #103).
    fn is_qualified_secret_key(original: &str, upper: &str) -> bool {
        const WORDS: &[&str] = &[
            "PASSWORD",
            "PASSWD",
            "PASSPHRASE",
            "SECRET",
            "TOKEN",
            "APIKEY",
            "SECRETKEY",
            "PRIVATEKEY",
            "ACCESSKEY",
            "CREDENTIAL",
            "CREDENTIALS",
            "DBPASS",
            "PW",
            "PWD",
            // v1.5.1 (#104): `creds`, `{"creds": {...}}`.
            "CREDS",
        ];
        // What follows a credential word when the key names something about the credential
        // rather than the credential (`token_type`, `password_file`, `token_savings`). Anything
        // else still counts as the credential (`password_admin`, `api_key_prod`): the list fails
        // closed (security review of #103).
        const NOT_SECRET: &[&str] = &[
            "FILE",
            "FILES",
            "PATH",
            "DIR",
            "URL",
            "URI",
            "ENDPOINT",
            "HOST",
            "PORT",
            "LENGTH",
            "LEN",
            "MIN",
            "MAX",
            "COUNT",
            "NUM",
            "TYPE",
            "TYPES",
            "NAME",
            "NAMES",
            "POLICY",
            "ENABLED",
            "DISABLED",
            "REQUIRED",
            "HINT",
            "LABEL",
            "FIELD",
            "PROMPT",
            "TTL",
            "EXPIRY",
            "EXPIRES",
            "EXPIRATION",
            "LIFETIME",
            "HEADER",
            "PARAM",
            "PREFIX",
            "FORMAT",
            "ALGORITHM",
            "ALG",
            "SIZE",
            "LIMIT",
            "KIND",
            "MODE",
            "REGEX",
            "PATTERN",
            "PROVIDER",
            "ROTATION",
            "LOCATION",
            "ENV",
            "VAR",
            "VARIABLE",
            "STORE",
            "MANAGER",
            "SOURCE",
            "VERSION",
            "STATUS",
            "USAGE",
            "USED",
            "LEFT",
            "REMAINING",
            "BUDGET",
            "ESTIMATE",
            "STREAM",
            "STREAMS",
            "REDACTOR",
            "REDACTION",
            "SAVINGS",
            "SAVED",
            "CACHE",
            "LATTICE",
            "COUNTER",
            "BUCKET",
            "RATE",
            "WINDOW",
            "BUFFER",
            "POOL",
            "INDEX",
            "LIST",
            "MAP",
            "IDS",
        ];
        // Dotted keys (`headers.authorization`, `db.pwd`) split like snake_case ones.
        let parts: Vec<&str> = upper.split(['_', '.']).filter(|p| !p.is_empty()).collect();
        // A webhook URL is the credential itself, so `_URL`, `_URI` and `_ENDPOINT` do not make
        // it harmless (#104); `WEBHOOK_NAME` or `WEBHOOK_EVENT` stay (review of #106).
        if let Some(w) = parts.iter().position(|p| p.contains("WEBHOOK")) {
            // Fail closed: only known metadata after `WEBHOOK` is harmless (security review of
            // #106: an allowlist of credential suffixes let `WEBHOOK_SIGNING_KEY` and
            // `SLACK_WEBHOOK_PROD` through).
            let tail = &parts[w + 1..];
            let metadata = !tail.is_empty()
                && tail.iter().all(|p| {
                    matches!(
                        *p,
                        "NAME"
                            | "NAMES"
                            | "EVENT"
                            | "EVENTS"
                            | "ID"
                            | "IDS"
                            | "ENABLED"
                            | "DISABLED"
                            | "TYPE"
                            | "TYPES"
                            | "COUNT"
                            | "STATUS"
                            | "VERSION"
                            | "TIMEOUT"
                            | "RETRIES"
                    )
                });
            if !metadata {
                return true;
            }
        }
        if parts.contains(&"AUTHORIZATION")
            || (parts.last() == Some(&"AUTH") && (parts.len() == 1 || upper.contains('.')))
        {
            return true;
        }
        // The shell's `PWD` is a path; `MYSQL_PWD` and a lower-case `pwd` are passwords.
        if upper == "PWD" && !original.bytes().any(|b| b.is_ascii_lowercase()) {
            return false;
        }
        let word = |p: &str| {
            WORDS
                .iter()
                .any(|w| p == *w || (p.len() > w.len() && p.ends_with(w) && w.len() >= 5))
        };
        (0..parts.len()).any(|i| {
            // One part (`PGPASSWORD`), or two joined (`API`+`KEY`, `ACCESS`+`KEY`).
            let rest = if word(parts[i]) {
                &parts[i + 1..]
            } else if i + 1 < parts.len() && word(&format!("{}{}", parts[i], parts[i + 1])) {
                &parts[i + 2..]
            } else {
                return false;
            };
            rest.iter().all(|r| !NOT_SECRET.contains(r))
        })
    }

    /// `accessToken` -> `access_Token`, `APIKey` -> `API_Key`: word boundaries of camelCase and
    /// PascalCase as `_`.
    pub(crate) fn split_camel(key: &str) -> String {
        let chars: Vec<char> = key.chars().collect();
        let mut out = String::with_capacity(key.len() + 4);
        for (i, &c) in chars.iter().enumerate() {
            if i > 0 && c.is_ascii_uppercase() {
                let prev = chars[i - 1];
                let next_lower = chars.get(i + 1).is_some_and(char::is_ascii_lowercase);
                if prev.is_ascii_lowercase()
                    || prev.is_ascii_digit()
                    || (prev.is_ascii_uppercase() && next_lower)
                {
                    out.push('_');
                }
            }
            out.push(c);
        }
        out
    }

    /// Upper-case `_`/`-`-separated parts of a key (`--max-tokens` -> `MAX`, `TOKENS`).
    fn key_parts(key: &str) -> Vec<String> {
        key.to_ascii_uppercase()
            .split(['_', '-', '.'])
            .filter(|p| !p.is_empty())
            .map(str::to_owned)
            .collect()
    }

    /// Whether a key part names a credential kind, which no counter exemption may override.
    fn has_credential_part(parts: &[String]) -> bool {
        const CREDENTIAL_PARTS: &[&str] = &[
            "AUTH",
            "ACCESS",
            "SESSION",
            "REFRESH",
            "BEARER",
            "API",
            "ID",
            "CSRF",
            "XSRF",
            "OAUTH",
            "SECRET",
            "PRIVATE",
            "CLIENT",
            "GITHUB",
            "GH",
            "GITLAB",
            "NPM",
            "PYPI",
            "PAT",
            "SLACK",
            "BOT",
            "DEPLOY",
            "PUSH",
            "WRITE",
            "SERVICE",
            "PERSONAL",
            "JWT",
            "SIGNING",
            "RESET",
            "INVITE",
            "WEBHOOK",
            "HF",
            "OPENAI",
            "ANTHROPIC",
        ];
        parts.iter().any(|p| CREDENTIAL_PARTS.contains(&p.as_str()))
    }

    /// `MAX_TOKENS`, `INPUT_TOKENS`, `COMPLETION_TOKENS`, `cache_read_input_tokens`, ...:
    /// counters in LLM configs and usage blocks, not credentials (v1.2.1; v1.2.0 hid them via
    /// the plural-key rule). A key qualifies only when it ends in plural `TOKENS`, the part
    /// directly before names a count, and no part names a credential (`AUTH`, `ACCESS`, ...).
    /// Values under such keys are still checked by the vendor-prefix and entropy detectors.
    fn is_token_count_key(key_upper: &str) -> bool {
        const COUNT_PARTS: &[&str] = &[
            "MAX",
            "MIN",
            "NUM",
            "COUNT",
            "TOTAL",
            "INPUT",
            "OUTPUT",
            "PROMPT",
            "COMPLETION",
            "CACHE",
            "CACHED",
            "REASONING",
            "CONTEXT",
            "BUDGET",
            "LIMIT",
            "AUDIO",
            "TEXT",
            "IMAGE",
            "ACCEPTED",
            "REJECTED",
            "PREDICTION",
            "THINKING",
        ];
        let parts = Self::key_parts(key_upper);
        parts.len() >= 2
            && parts[parts.len() - 1] == "TOKENS"
            && COUNT_PARTS.contains(&parts[parts.len() - 2].as_str())
            && !Self::has_credential_part(&parts)
    }

    /// One line, redacted until it stops changing (at most five rounds). The line passes pair
    /// quotes by position, so a replacement can change what a second pass sees; running to a
    /// fixed point here makes `redact(redact(x)) == redact(x)` hold for every line shape
    /// (v1.5.1). A line without a secret costs one round.
    fn redact_line(line: &str) -> String {
        let mut current = Self::redact_line_once(line);
        if current == line {
            return current;
        }
        for _ in 0..4 {
            let next = Self::redact_line_once(&current);
            if next == current {
                break;
            }
            current = next;
        }
        current
    }

    fn redact_line_once(line: &str) -> String {
        // 0. v1.5.1 (#104): markup, name/value pairs, subscripts, `=>`, shell `set`,
        //    password-taking CLI tools, URL userinfo and parameters, numeric PINs.
        // JSON embedded after a prefix (`log {"cmd": "…"}`, #108) is redacted structurally
        // first, so its strings are decoded; the line rules below still run on the whole line.
        let embedded = Self::redact_embedded_json(line);
        let line = embedded.as_deref().unwrap_or(line);
        let line = &crate::redact_shapes::redact_line_shapes(line);
        // 1. Check space-separated CLI flags (`--api-key VALUE`, `--password "VALUE"`, etc.)
        let mut current = Self::redact_cli_flags(line);

        // 2. Check `.env` / shell / query / CLI `KEY=VALUE` and `--flag=VALUE` assignments for sensitive key names
        current = Self::redact_env_assignments(&current);

        // 3. Redact embedded credentials in database/broker URLs (`scheme://user:password@host`) before colon scan
        current = Self::redact_connection_urls(&current);

        // 4. Check JSON / YAML `"key": "value"` or `key: value` assignments for sensitive key names
        current = Self::redact_colon_assignments(&current);

        // 5. Redact 3-segment Base64URL JWT tokens (`eyJ... . eyJ... . ...`)
        current = Self::redact_jwt_tokens(&current);

        // 5b. Structured vendor formats (SendGrid, Slack webhooks, Twilio)
        current = Self::redact_structured_vendor_tokens(&current);

        // 6. Token-level prefix scan for 2025/2026 vendor credential formats (advancing cursor past short non-matches)
        let prefixes: &[(&str, &str, usize)] = &[
            ("sk-ant-", "[REDACTED:ANTHROPIC_KEY]", 14),
            ("sk-proj-", "[REDACTED:OPENAI_KEY]", 14),
            ("sk-svcacct-", "[REDACTED:OPENAI_KEY]", 16),
            ("sk_live_", "[REDACTED:STRIPE_KEY]", 14),
            ("rk_live_", "[REDACTED:STRIPE_KEY]", 14),
            ("sk_test_", "[REDACTED:STRIPE_KEY]", 14),
            ("rk_test_", "[REDACTED:STRIPE_KEY]", 14),
            ("whsec_", "[REDACTED:STRIPE_WEBHOOK]", 14),
            ("sk-", "[REDACTED:API_KEY]", 12),
            ("ghp_", "[REDACTED:GH_TOKEN]", 12),
            ("gho_", "[REDACTED:GH_TOKEN]", 12),
            ("ghu_", "[REDACTED:GH_TOKEN]", 12),
            ("ghs_", "[REDACTED:GH_TOKEN]", 12),
            ("ghr_", "[REDACTED:GH_TOKEN]", 12),
            ("github_pat_", "[REDACTED:GH_PAT]", 18),
            ("aim_", "[REDACTED:AIM_TOKEN]", 12),
            ("AKIA", "[REDACTED:AWS_KEY]", 16),
            ("ASIA", "[REDACTED:AWS_KEY]", 16),
            ("AIza", "[REDACTED:GCP_KEY]", 20),
            ("hf_", "[REDACTED:HF_TOKEN]", 16),
            ("npm_", "[REDACTED:NPM_TOKEN]", 16),
            ("pypi-", "[REDACTED:PYPI_TOKEN]", 16),
            ("xoxb-", "[REDACTED:SLACK_TOKEN]", 14),
            ("xoxp-", "[REDACTED:SLACK_TOKEN]", 14),
            ("xoxa-", "[REDACTED:SLACK_TOKEN]", 14),
            ("xoxr-", "[REDACTED:SLACK_TOKEN]", 14),
            ("xoxs-", "[REDACTED:SLACK_TOKEN]", 14),
            ("xoxo-", "[REDACTED:SLACK_TOKEN]", 14),
            // v1.1.0 (ICX-AUD-015)
            ("xapp-", "[REDACTED:SLACK_TOKEN]", 20),
            ("glpat-", "[REDACTED:GITLAB_TOKEN]", 20),
            ("glptt-", "[REDACTED:GITLAB_TOKEN]", 20),
            ("dop_v1_", "[REDACTED:DIGITALOCEAN_TOKEN]", 40),
            ("shpat_", "[REDACTED:SHOPIFY_TOKEN]", 30),
            ("shpss_", "[REDACTED:SHOPIFY_TOKEN]", 30),
        ];

        for &(prefix, replacement, min_len) in prefixes {
            let mut search_pos = 0usize;
            while search_pos < current.len() {
                let Some(rel_idx) = current[search_pos..].find(prefix) else {
                    break;
                };
                let start = search_pos + rel_idx;
                // The generic `sk-` inside an ordinary word is not a token start (`autodesk-...`,
                // `disk-by...` matched in the field; v1.3.2): only when 2+ letters (no digits)
                // precede it, so the whole is a word such as `di|sk-` or `autode|sk-`. Every distinctive prefix (`ghp_`, `AKIA`,
                // `sk-ant-`, ...) is redacted wherever it appears, so `%3Dghp_...` or a literal
                // `\nAKIA...` is still caught (security review of #38).
                if prefix == "sk-" && Self::ends_an_ordinary_word(&current[..start]) {
                    search_pos = start + prefix.len();
                    continue;
                }
                let rest = &current.as_bytes()[start..];
                let token_len = rest
                    .iter()
                    .take_while(|&&b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                    .count();
                if token_len >= min_len {
                    current.replace_range(start..start + token_len, replacement);
                    search_pos = start + replacement.len();
                } else {
                    search_pos = start + prefix.len();
                }
            }
        }

        // 7. Redact `Bearer <tok>` and `Basic <tok>` authorization headers (ASCII byte-safe)
        current = Self::redact_auth_scheme(&current, "bearer ", "[REDACTED:BEARER]", false);
        current = Self::redact_auth_scheme(&current, "basic ", "[REDACTED:BASIC_AUTH]", true);

        // 8. Tier 3 — Shannon character entropy gate for unprefixed high-entropy secret tokens
        Self::redact_high_entropy_tokens(&current)
    }

    /// The length of the complete marker `s` starts with (`[REDACTED:` + `A-Z` and `_` + `]`),
    /// if any. An unclosed or malformed `[REDACTED:` is ordinary text and shields nothing
    /// (review of #106).
    pub(crate) fn marker_len(s: &str) -> Option<usize> {
        let rest = s.strip_prefix("[REDACTED:")?;
        let k = rest
            .bytes()
            .take_while(|b| b.is_ascii_uppercase() || *b == b'_')
            .count();
        (k > 0 && rest.as_bytes().get(k) == Some(&b']')).then_some("[REDACTED:".len() + k + 1)
    }

    /// The length of the run of complete markers `s` starts with, with spaces between them
    /// when `spaced` (config text).
    pub(crate) fn marker_run(s: &str, spaced: bool) -> usize {
        let mut at = 0;
        loop {
            let rest = &s[at..];
            let gap = if spaced {
                rest.len() - rest.trim_start_matches([' ', '\t']).len()
            } else {
                0
            };
            match Self::marker_len(&rest[gap..]) {
                Some(n) => at += gap + n,
                None => return at,
            }
        }
    }

    /// Whether `s` (trimmed) is nothing but complete markers, so there is nothing left to hide
    /// (review of #106: a span that only starts with one may still hold a secret).
    pub(crate) fn is_marker_run(s: &str) -> bool {
        let t = s.trim();
        !t.is_empty() && Self::marker_run(t, true) == t.len()
    }

    /// Whether `s` starts with a complete marker.
    pub(crate) fn starts_with_marker(s: &str) -> bool {
        Self::marker_len(s).is_some()
    }

    /// Whether `text` is exactly one marker this redactor emits (`[REDACTED:ENV_SECRET]`).
    fn is_redaction_marker(text: &str) -> bool {
        text.strip_prefix("[REDACTED:")
            .and_then(|r| r.strip_suffix(']'))
            .is_some_and(|kind| {
                !kind.is_empty() && kind.bytes().all(|b| b.is_ascii_uppercase() || b == b'_')
            })
    }

    /// Under a sensitive key, a value whose line holds any quote (`"`, `'`, a backtick, or an
    /// escaped `\"`) is replaced from its first quote to the end of the line by one marker
    /// (v1.3.1, #29). A call head directly before that quote stays visible
    /// (`env::var([REDACTED]`), since it is an identifier path ending in `(` or `[`. Any other head
    /// could itself be the secret (`hunter2 "note"`, `x or "..."`), so the value is replaced from
    /// its start. Owner decisions on PR #31: there is deliberately no quote pairing, no field
    /// boundary and no key detection. Each of those was bypassed in review: a stray quote shifted
    /// the pairing, and redacted key names hid later sensitive keys. Later text on the same line is
    /// hidden with the value. Returns the end of the replacement, or `None` when the value's line
    /// holds no quote, or holds nothing but a call head and an existing marker (idempotent).
    fn redact_from_first_quote(
        out: &mut String,
        val_start: usize,
        replacement: &str,
    ) -> Option<usize> {
        let rest = out.get(val_start..)?;
        let line_end = rest.find('\n').unwrap_or(rest.len());
        let line = rest[..line_end].trim_end_matches('\r');
        // Already done by an earlier pass: a call head (or nothing) followed by one marker.
        if let Some(m) = line.find("[REDACTED:")
            && (m == 0 || Self::is_call_head(&line[..m]))
            && Self::is_redaction_marker(&line[m..])
        {
            return Some(val_start + line.len());
        }
        let quote = line.find(['"', '\'', '`'])?;
        // An escaped quote (`\"`) starts at its backslash.
        let quote = if quote > 0 && line.as_bytes()[quote - 1] == b'\\' {
            quote - 1
        } else {
            quote
        };
        let head = &line[..quote];
        let from = if Self::is_call_head(head) { quote } else { 0 };
        let len = line.len();
        out.replace_range(val_start + from..val_start + len, replacement);
        Some(val_start + from + replacement.len())
    }

    /// `env::var(`, `os.getenv(`, `SecretStr(`, `cfg[`: an identifier path ending in `(` or `[`.
    fn is_call_head(head: &str) -> bool {
        head.ends_with(['(', '['])
            && head
                .as_bytes()
                .first()
                .is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_')
            && head[..head.len() - 1]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b':' | b'.'))
    }

    fn redact_value_span(
        out: &mut String,
        val_start: usize,
        replacement: &str,
        stop_at_comma: bool,
        key: &str,
        code_context: u8,
    ) -> Option<usize> {
        // A comma ends a value only inside brackets opened before the key (a call, a flow
        // mapping or sequence); in a YAML block scalar or an env file it is part of the value
        // (review of #103: `api_key: 1234,secret` hid only `1234`).
        let line_start = out[..val_start].rfind('\n').map_or(0, |i| i + 1);
        // The colon pass (YAML, JSON-ish, config text) is the one that asks to stop at commas;
        // in that text `;` is part of a value, not a shell separator (red-team review of #103).
        // A type-annotated code assignment (`let api_key: Option<String> = None;`) comes through
        // the same pass but is code, not config text (review of #103).
        let line_text = out[line_start..].split('\n').next().unwrap_or("").trim();
        // Only a binding keyword marks an annotated value as code: a trailing `;` also ends
        // connection strings and other config values (review of #103).
        let is_code_line = [
            "let ", "const ", "var ", "val ", "final ", "static ", "pub ", "def ", "fn ",
        ]
        .iter()
        .any(|k| line_text.starts_with(k));
        let config = stop_at_comma && !(code_context >= CODE_STRONG && is_code_line);
        let stop_at_comma = stop_at_comma && Self::open_brackets(&out[line_start..val_start]) > 0;
        // `Authorization: Bearer <t>` / `Basic <t>`: the scheme stays and the rest of the line is
        // hidden here with the scheme's own marker, whatever the token's length (security
        // review: the auth-scheme detector skips short tokens).
        if Self::normalized_key(key).contains("AUTHORIZATION") {
            let lead =
                out[val_start..].len() - out[val_start..].trim_start_matches([' ', '\t']).len();
            let v_start = val_start + lead;
            for (scheme, marker) in [
                ("bearer ", "[REDACTED:BEARER]"),
                ("basic ", "[REDACTED:BASIC_AUTH]"),
            ] {
                if out[v_start..]
                    .get(..scheme.len())
                    .is_some_and(|p| p.eq_ignore_ascii_case(scheme))
                {
                    let from = v_start + scheme.len();
                    let rest = &out[from..];
                    let line_end = rest.find('\n').unwrap_or(rest.len());
                    let line = rest[..line_end].trim_end_matches('\r');
                    let token_start = line.len() - line.trim_start().len();
                    let token = line[token_start..].trim_end();
                    if token.is_empty() || Self::is_redaction_marker(token) {
                        return Some(from + line.len());
                    }
                    let (a, b) = (from + token_start, from + token_start + token.len());
                    out.replace_range(a..b, marker);
                    return Some(a + marker.len());
                }
            }
        }
        // ADR-0011: a value that is never a secret is kept, and a value that opens with one
        // literal loses only that literal (and a quoted tail).
        if let Some(end) = Self::redact_value_by_shape(
            out,
            val_start,
            replacement,
            stop_at_comma,
            key,
            code_context,
            config,
        ) {
            return Some(end);
        }
        // Any other value with a quote is hidden from its first quote to the end of the line
        // (v1.3.1, #29, owner decision on PR #31).
        if let Some(end) = Self::redact_from_first_quote(out, val_start, replacement) {
            return Some(end);
        }
        // Past this point the rest of the line holds no quote of any kind, so only unquoted
        // shapes remain (#33: the quoted-literal branches that used to follow were unreachable).
        let bytes = out.as_bytes();
        if val_start >= bytes.len() {
            return None;
        }
        let quote = bytes[val_start];
        if quote == b'[' {
            // An earlier pass already replaced this value. Text glued to the marker, or after it
            // in config text, is part of the same value and is hidden too: a planted
            // `[REDACTED:X]secret` must not keep the secret (#104).
            if Self::starts_with_marker(&out[val_start..]) {
                let line_end = out[val_start..]
                    .find('\n')
                    .map_or(out.len(), |k| val_start + k);
                let line = out[val_start..line_end].trim_end_matches('\r');
                // Complete markers only, and all of a leading run of them (`[REDACTED:X]
                // [REDACTED:Y]secret`; review of #106); anything else is an ordinary value.
                Self::marker_len(line)?;
                let run = Self::marker_run(line, config);
                let tail = &line[run..];
                let t = tail.trim_start();
                let glued = !tail.is_empty() && !tail.starts_with([' ', '\t']);
                // `#`, `//` and `\` start a comment or escape only after a space: glued to the
                // marker they are part of the same word (#108: `[REDACTED:X]#secret`).
                let structural = t.is_empty()
                    || t.starts_with([';', ',', ')', ']', '}', '&', '|'])
                    || (!glued && (t.starts_with(['#', '\\']) || t.starts_with("//")));
                if !structural && (glued || config) {
                    let from = val_start + run + (tail.len() - t.len());
                    let to = val_start + line.len();
                    out.replace_range(from..to, replacement);
                    return Some(from + replacement.len());
                }
                return None;
            }
            // A flow sequence is the value of the sensitive key as a whole: hide its contents
            // up to the matching `]` on this line (an unterminated one hides the rest).
            let inner_start = val_start + 1;
            let mut depth = 1usize;
            let mut inner_end = bytes.len();
            // No quote can follow here (a quoted line took the first-quote rule), so brackets
            // are structure.
            for (offset, &b) in bytes[inner_start..].iter().enumerate() {
                match b {
                    b'[' => depth += 1,
                    b']' => {
                        depth -= 1;
                        if depth == 0 {
                            inner_end = inner_start + offset;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let inner = &out[inner_start..inner_end];
            if !inner.trim().is_empty() && !Self::is_marker_run(inner) {
                out.replace_range(inner_start..inner_end, replacement);
                return Some(inner_start + replacement.len());
            }
            return None;
        }
        if quote == b'{' {
            // A nested flow mapping: its own keys are classified independently.
            return None;
        }
        let rest = &out[val_start..];
        let mut val_len = rest.len();
        let cli = replacement == "[REDACTED:CLI_SECRET]";
        // A whitespace run is judged once and then skipped, so a long run costs linear time
        // (security review: re-scanning the run at every space was quadratic).
        let mut skip_to = 0usize;
        for (idx, ch) in rest.char_indices() {
            if idx < skip_to {
                continue;
            }
            // A lone carriage return inside a line is content (terminal overwrite), not its end.
            if ch == '\r' && rest[idx + 1..].chars().next().is_some_and(|n| n != '\n') {
                continue;
            }
            // A backtick or an existing marker ends the token too, so a marker the literal pass
            // wrote is never cut in half (idempotence, v1.3.1).
            // `;` ends the value only before whitespace, and closing brackets only inside
            // brackets: in an env file `null;secret` and `null)secret` are one value (review
            // of #103).
            let after = rest[idx + ch.len_utf8()..].chars().next();
            // An unquoted value runs to the end of the line, not to its first space: a YAML
            // plain scalar, an INI or env value and a passphrase hold spaces (red-team review of
            // #103: `password: correct horse staple` kept two words). It ends before a comment,
            // a shell `|`, `&&` or `;`, the next query parameter (`&name=`), and line ends. A
            // CLI flag value is one argument, so it still ends at whitespace.
            let ws_run_end =
                || idx + (rest[idx..].len() - rest[idx..].trim_start_matches([' ', '\t']).len());
            let ws_ends = || {
                let next = &rest[ws_run_end()..];
                let semicolon_ends = next
                    .strip_prefix(';')
                    .is_some_and(|r| r.chars().next().is_none_or(char::is_whitespace));
                next.is_empty()
                    || next.starts_with(['\n', '#'])
                    || next.starts_with("\r\n")
                    || next == "\r"
                    || (!config && (next.starts_with('|') || semicolon_ends))
                    || next.starts_with("//")
                    || (!config && next.starts_with("&&"))
            };
            let query_param = || {
                let n = &rest[idx + 1..];
                let name_len = n
                    .find(|c: char| {
                        !(c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-' | '[' | ']'))
                    })
                    .unwrap_or(n.len());
                !config && (n.starts_with('&') || (name_len > 0 && n[name_len..].starts_with('=')))
            };
            // A lone carriage return inside a line is content (terminal overwrite), not its end.
            if ch == '\n'
                || (ch == '\r' && after.is_none_or(|n| n == '\n'))
                || (ch.is_whitespace() && (cli || ws_ends()))
                || matches!(ch, '"' | '\'' | '`')
                || (ch == '&' && query_param())
                || (!config && ch == ';' && after.is_none_or(char::is_whitespace))
                || (stop_at_comma
                    && matches!(ch, ',' | ')' | ']' | '}')
                    && after.is_none_or(|n| {
                        n.is_whitespace() || matches!(n, ',' | ';' | ')' | ']' | '}')
                    }))
                || (idx > 0 && Self::starts_with_marker(&rest[idx..]))
            {
                val_len = idx;
                break;
            }
            if ch == ' ' || ch == '\t' {
                skip_to = ws_run_end();
                continue;
            }
            if ch == '\\' {
                let after_slash = &rest[idx + 1..];
                if after_slash.starts_with('n')
                    || after_slash.starts_with('r')
                    || after_slash.starts_with('t')
                    || after_slash.starts_with('"')
                    || after_slash.starts_with('\'')
                {
                    val_len = idx;
                    break;
                }
            }
        }
        let value = &out[val_start..val_start + val_len];
        if val_len > 0 && !Self::starts_with_marker(value) {
            out.replace_range(val_start..val_start + val_len, replacement);
            return Some(val_start + replacement.len());
        }
        // A value that starts with a marker and goes on (`[REDACTED:X]secret`, planted or
        // left by an earlier pass) loses the rest too (#104); markers only stay as they are.
        if Self::starts_with_marker(value) {
            let run = Self::marker_run(value, true);
            let tail = &value[run..];
            let t = tail.trim_start();
            if !t.is_empty() {
                let from = val_start + run + (tail.len() - t.len());
                out.replace_range(from..val_start + val_len, replacement);
                return Some(from + replacement.len());
            }
        }
        None
    }

    /// ADR-0011 rules 1 to 3 for the value at `val_start` (the rest of its line), returning where
    /// scanning continues. A value that is never a secret stays; a value that opens with one
    /// quoted literal has that literal's contents replaced. Either way the rest of the line stays
    /// when it holds no quote, and is hidden from its first quote to the end of the line when it
    /// does (a later literal may be a second secret, #29). `None`: neither shape, so the #29 rule
    /// applies.
    fn redact_value_by_shape(
        out: &mut String,
        val_start: usize,
        replacement: &str,
        stop_at_comma: bool,
        key: &str,
        code_context: u8,
        config: bool,
    ) -> Option<usize> {
        let rest = out.get(val_start..)?;
        let line_end = rest.find('\n').unwrap_or(rest.len());
        let line = rest[..line_end].trim_end_matches('\r');
        let line_len = line.len();
        if line.is_empty() || Self::is_marker_run(line) {
            return None;
        }
        // Brackets opened before the key (a call, a flow mapping or sequence): only inside them do
        // `,` and closing brackets separate values (review of #103: in a YAML block scalar,
        // `api_key: 1234,secret` is one value).
        let line_start = out[..val_start].rfind('\n').map_or(0, |i| i + 1);
        let open_before = Self::open_brackets(&out[line_start..val_start]);
        // The kept span ends at `cursor`; the rest of the line (`tail_len` bytes from there)
        // follows the #29 rule for any quote it holds: a later quote is always hidden.
        // Inside brackets a comma may end a harmless value whatever the caller's rule, since
        // `ends_value` then also requires a real boundary after it.
        let harmless_end =
            Self::value_extent(line, stop_at_comma || open_before > 0).and_then(|end| {
                let v = line[..end].trim_end();
                (Self::is_harmless_value(v, key, code_context, false)
                    && Self::ends_value(&line[v.len()..], open_before, config, true))
                .then_some(v.len())
            });
        let (cursor, tail_len) = if let Some(end) = harmless_end {
            (val_start + end, line_len - end)
        } else {
            let (open, close) = Self::leading_literal(line)?;
            // The literal must end the value token: `"abc"def`, `"a"#b` and `"a"/b` are one shell
            // word, so the tail is part of the secret and the #29 rule hides the whole value
            // (security review). Commas and closing brackets end it only in structured text
            // (`stop_at_comma`); a comment needs whitespace before it.
            if !Self::ends_value(&line[close + 1..], open_before, config, false) {
                return None;
            }
            let mut cursor = val_start + close + 1;
            if !Self::is_redaction_marker(&line[open + 1..close]) {
                out.replace_range(val_start + open + 1..val_start + close, replacement);
                cursor = val_start + open + 1 + replacement.len() + 1;
            }
            // The rest after a literal stays only when it is a comment or structure; a bare
            // word (`"a" b`, `'a' if x else b`) is part of the value to an INI or properties
            // parser (red-team review of #103), so it is hidden.
            let tail = &out[cursor..cursor + line_len - (close + 1)];
            let t = tail.trim_start();
            // The next assignment (`BAR=2`, `user: bob`) is not part of the value: it stays and
            // is scanned in its turn.
            let name_len = t
                .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')))
                .unwrap_or(t.len());
            // The next assignment of the same kind (`name:` in config text, `name=` elsewhere),
            // or the next flag after a CLI flag (review of #103: `foo:x` on an `=` line and
            // `--x` in config text are part of the value).
            let sep = if config { ':' } else { '=' };
            // A real assignment: `name=value` (not `==`, `=>` or a bare trailing `=`, as in
            // Base64 padding), or `name: value` with the space YAML requires (review of #103).
            let after_sep = t[name_len..].strip_prefix(sep);
            let next_assignment = (name_len > 0
                && t.as_bytes()[0].is_ascii_alphabetic()
                && after_sep.is_some_and(|r| {
                    if config {
                        r.starts_with([' ', '\t'])
                    } else {
                        r.chars()
                            .next()
                            .is_some_and(|c| !c.is_whitespace() && !matches!(c, '=' | '>'))
                    }
                }))
                || (key.trim_start().starts_with("--")
                    && t.strip_prefix("--")
                        .is_some_and(|r| r.starts_with(|c: char| c.is_ascii_alphabetic())));
            if !(t.is_empty()
                || t.starts_with('#')
                || t.starts_with("//")
                // Separators and closers count only inside brackets opened before the key.
                || (open_before > 0 && t.starts_with([',', ')', ']', '}']))
                // `;` ends a statement only before whitespace (`"abc" ;x` is one shell word with
                // the value; review of #103).
                || (!config
                    && t.strip_prefix(';')
                        .is_some_and(|r| r.chars().next().is_none_or(char::is_whitespace)))
                || next_assignment)
            {
                let from = cursor + (tail.len() - t.len());
                let to = cursor + tail.len();
                out.replace_range(from..to, replacement);
                return Some(cursor);
            }
            (cursor, line_len - (close + 1))
        };
        let tail = &out[cursor..cursor + tail_len];
        if let Some(q) = tail.find(['"', '\'', '`']) {
            let q = if q > 0 && tail.as_bytes()[q - 1] == b'\\' {
                q - 1
            } else {
                q
            };
            let from = cursor + q;
            if !Self::is_redaction_marker(&out[from..cursor + tail_len]) {
                out.replace_range(from..cursor + tail_len, replacement);
            }
        }
        // Scanning resumes at the kept tail, so a second assignment in it is still checked
        // (review of #103: `PASSWORD="a" TOKEN=b # "c"` left `TOKEN=b` visible).
        Some(cursor)
    }

    /// Whether `after` (the text right after a value) ends the value: nothing, whitespace, or a
    /// `;` followed by whitespace; inside brackets opened before the key, also `,` or a closing
    /// bracket followed by nothing, whitespace or more structure. `null;x`, `"a"#x`,
    /// `1234,x` and `null)x` are one shell word or scalar, so they do not.
    fn ends_value(after: &str, open_before: usize, config: bool, strict: bool) -> bool {
        let mut chars = after.chars();
        let Some(c) = chars.next() else {
            return true;
        };
        let then_ok = |next: Option<char>| {
            next.is_none_or(|n| n.is_whitespace() || matches!(n, ',' | ';' | ')' | ']' | '}'))
        };
        match c {
            // After whitespace, only a comment, the end, or (outside config text) a shell
            // separator ends the value; `null ;x` in YAML is one scalar (red-team review of #103).
            // (The literal path, `strict = false`, judges the rest itself with its tail rule.)
            c if c.is_whitespace() && !strict => true,
            c if c.is_whitespace() => {
                let t = after.trim_start();
                t.is_empty()
                    || t.starts_with('#')
                    || t.starts_with("//")
                    || (!config
                        && (t.starts_with('|')
                            || t.starts_with("&&")
                            || t.strip_prefix(';')
                                .is_some_and(|r| r.chars().next().is_none_or(char::is_whitespace))))
                    || (open_before > 0 && t.starts_with([',', ')', ']', '}']))
            }
            ';' if !config => chars.next().is_none_or(char::is_whitespace),
            ',' | ')' | ']' | '}' if open_before > 0 => then_ok(chars.next()),
            _ => false,
        }
    }

    /// Brackets left open in `prefix`, outside quotes (approximate: a stray quote ends the
    /// count, which only makes `ends_value` stricter).
    fn open_brackets(prefix: &str) -> usize {
        let mut depth = 0usize;
        let mut quote: Option<char> = None;
        let mut escaped = false;
        for c in prefix.chars() {
            if let Some(q) = quote {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == q {
                    quote = None;
                }
                continue;
            }
            match c {
                '"' | '\'' | '`' => quote = Some(c),
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        if quote.is_some() { 0 } else { depth }
    }

    /// Where a value starting at the beginning of `line` ends (ADR-0011 rule 1): at the first
    /// `;`, closing bracket, comment start (`#` or `//` after whitespace) or, with
    /// `stop_at_comma`, `,`, outside quotes and brackets; the end of the line otherwise. `None`
    /// for an unterminated quote or bracket.
    fn value_extent(line: &str, stop_at_comma: bool) -> Option<usize> {
        let b = line.as_bytes();
        let mut depth = 0usize;
        let mut i = 0usize;
        while i < b.len() {
            let c = b[i];
            let after_space = i == 0 || b[i - 1].is_ascii_whitespace();
            match c {
                b'"' | b'\'' | b'`' => {
                    i += 1 + closing_quote(&b[i + 1..], c)?;
                }
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => {
                    if depth == 0 {
                        return Some(i);
                    }
                    depth -= 1;
                }
                b';' if depth == 0 => return Some(i),
                b',' if depth == 0 && stop_at_comma => return Some(i),
                b'#' if depth == 0 && after_space => return Some(i),
                b'/' if depth == 0 && after_space && b.get(i + 1) == Some(&b'/') => {
                    return Some(i);
                }
                _ => {}
            }
            i += 1;
        }
        (depth == 0).then_some(b.len())
    }

    /// The opening and closing quote of a literal that starts `line` (`"..."`, `'...'`, with an
    /// optional `b`, `r`, `u` or `f` prefix of up to two letters). Backtick templates and
    /// unterminated literals are not literals here.
    fn leading_literal(line: &str) -> Option<(usize, usize)> {
        let b = line.as_bytes();
        let open = b.iter().take(3).position(|&c| c == b'"' || c == b'\'')?;
        if !b[..open]
            .iter()
            .all(|c| matches!(c.to_ascii_lowercase(), b'b' | b'r' | b'u' | b'f'))
        {
            return None;
        }
        let close = open + 1 + closing_quote(&b[open + 1..], b[open])?;
        Some((open, close))
    }

    /// ADR-0011 rule 2: a whole value that is never a secret. `code` is how sure the caller is
    /// that the line is code (`CODE_NONE`, `CODE_WEAK`: spaces around `=`, which INI and
    /// properties files have too, `CODE_STRONG`: a binding keyword, a type annotation or a
    /// trailing `;`). `json` is a strictly parsed JSON member, the only place an agent's session
    /// UUID is kept (red-team review of #103: `X-Session-Id: <uuid>` and prefixed session ids
    /// can be credentials).
    fn is_harmless_value(v: &str, key: &str, code: u8, json: bool) -> bool {
        if v.is_empty() {
            return false;
        }
        let lower = v.to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "null" | "none" | "nil" | "undefined" | "true" | "false"
        ) || v == "\"\""
            || v == "''"
        {
            return true;
        }
        // More than 10 digits may be a numeric token, as for the counter rule
        // (`is_counter_value`).
        if let Some(digits) = Self::number_digits(v) {
            return digits <= 10 && !Self::is_passcode_key(key);
        }
        (json && Self::is_session_id_key(key) && Self::is_uuid(Self::unquoted(v)))
            || Self::is_env_lookup(v)
            || Self::is_variable_reference(v)
            || (code >= CODE_STRONG && Self::is_code_reference(v) && Self::looks_like_code(v))
            || (code == CODE_WEAK && Self::is_constant_path(v))
    }

    /// `settings.SECRET_KEY`: a quote-free dotted path ending in an upper-snake constant, the one
    /// reference kept on a line that is only weakly code (spaces around `=`).
    fn is_constant_path(v: &str) -> bool {
        let segments: Vec<&str> = v.split('.').collect();
        segments.len() >= 2
            && segments.iter().all(|s| {
                s.as_bytes()
                    .first()
                    .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_')
                    && s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
            })
            && segments.last().is_some_and(|s| {
                s.contains('_')
                    && s.bytes().any(|c| c.is_ascii_uppercase())
                    && s.bytes()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
            })
    }

    /// A reference shape that INI, properties and env files do not produce, since spaces around
    /// `=` alone do not prove code (`password = my.dog.rex` is an INI value; security review): a
    /// call or subscript, a last segment that is an upper-snake constant
    /// (`settings.SECRET_KEY`), or a first segment that is a common code root (`self.token`,
    /// `config.api_key`).
    fn looks_like_code(v: &str) -> bool {
        const ROOTS: &[&str] = &[
            "self",
            "this",
            "cls",
            "settings",
            "config",
            "cfg",
            "conf",
            "app",
            "env",
            "os",
            "process",
            "options",
            "opts",
            "args",
            "params",
            "ctx",
            "context",
            "secrets",
            "credentials",
            "creds",
            "Settings",
            "Config",
            "std",
        ];
        let v = v.trim_start_matches(['&', '*']).trim_start_matches("mut ");
        let v = v.strip_prefix("await ").unwrap_or(v);
        // A call or subscript counts only when its head looks like code: a dotted or `::` path,
        // a snake_case name with an `_`, or a camelCase name (`get_token(user)`,
        // `os.getenv(K)`, `getToken(u)`); `abc(def)` could be an INI value (security review).
        if let Some(open) = v.find(['(', '[']) {
            let head = &v[..open];
            let camel = head.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
                && head.bytes().any(|c| c.is_ascii_uppercase());
            if head.contains(['.', ':', '_']) || camel {
                return true;
            }
        }
        let segments: Vec<&str> = v
            .split(['.', ':', '?', '-', '>'])
            .filter(|s| !s.is_empty())
            .collect();
        // An upper-snake constant with an `_` (`SECRET_KEY`): `my.PASS` could be an INI value
        // (review of #103).
        let last_is_const = segments.len() > 1
            && segments.last().is_some_and(|s| {
                s.contains('_')
                    && s.bytes().any(|c| c.is_ascii_uppercase())
                    && s.bytes()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
            });
        last_is_const || segments.first().is_some_and(|s| ROOTS.contains(s))
    }

    /// The number of digits of a plain decimal number (`-12`, `3.5e-2`, `16_384`), or `None`.
    fn number_digits(v: &str) -> Option<usize> {
        let body = v.strip_prefix(['-', '+']).unwrap_or(v);
        let (mantissa, exponent) = match body.find(['e', 'E']) {
            Some(i) => (&body[..i], Some(&body[i + 1..])),
            None => (body, None),
        };
        let mut digits = 0usize;
        let mut dot = false;
        let mut prev_digit = false;
        for c in mantissa.chars() {
            match c {
                '0'..='9' => {
                    digits += 1;
                    prev_digit = true;
                }
                '_' if prev_digit => prev_digit = false,
                '.' if !dot && prev_digit => {
                    dot = true;
                    prev_digit = false;
                }
                _ => return None,
            }
        }
        if digits == 0 || !prev_digit {
            return None;
        }
        // Exponent digits count too, and at most 3 are a number (review of #103:
        // `48291038e9182736` is not one).
        if let Some(e) = exponent {
            let e = e.strip_prefix(['-', '+']).unwrap_or(e);
            if e.is_empty() || e.len() > 3 || !e.bytes().all(|c| c.is_ascii_digit()) {
                return None;
            }
            digits += e.len();
        }
        Some(digits)
    }

    /// The key as the sensitivity rules see it, upper case with `-` as `_`.
    fn normalized_key(key: &str) -> String {
        Self::split_camel(
            key.trim()
                .trim_start_matches("export ")
                .trim_start_matches('-')
                .trim_matches(|c: char| c == '"' || c == '\''),
        )
        .to_ascii_uppercase()
        .replace('-', "_")
    }

    /// Password, passphrase, PIN, OTP, passcode and verification-code keys, whose numbers stay
    /// hidden.
    fn is_passcode_key(key: &str) -> bool {
        // Numbered keys (`pin2`, `otp_2`) are the same keys (review of #103).
        let full = Self::normalized_key(key);
        let k = full
            .trim_end_matches(|c: char| c.is_ascii_digit())
            .trim_end_matches('_')
            .to_owned();
        if Self::key_parts(&k)
            .iter()
            .any(|p| matches!(p.as_str(), "PW" | "PWD" | "DBPASS"))
            || k.ends_with("DBPASS")
        {
            return true;
        }
        [
            "PASSWORD",
            "PASSWD",
            "PASSCODE",
            "PASSPHRASE",
            "TOTP",
            "MFA",
            "2FA",
        ]
        .iter()
        .any(|p| k.contains(p))
            || Self::key_parts(&k)
                .iter()
                .any(|p| matches!(p.as_str(), "PASS" | "PIN" | "CODE") || p.ends_with("OTP"))
            || k.ends_with("PIN")
            || k.ends_with("OTP")
    }

    /// Exactly `session_id` / `sessionId`, as agent hook payloads name it (ADR-0011 rule 6;
    /// red-team review of #103: `sso_session_id`, `admin_session_id` and `X-Session-Id` can be
    /// credentials).
    fn is_session_id_key(key: &str) -> bool {
        matches!(
            Self::normalized_key(key).as_str(),
            "SESSION_ID" | "SESSIONID"
        )
    }

    fn unquoted(v: &str) -> &str {
        let b = v.as_bytes();
        if b.len() >= 2 && (b[0] == b'"' || b[0] == b'\'') && b[b.len() - 1] == b[0] {
            &v[1..v.len() - 1]
        } else {
            v
        }
    }

    /// `8-4-4-4-12` hex digits.
    fn is_uuid(s: &str) -> bool {
        s.len() == 36
            && s.bytes().enumerate().all(|(i, c)| {
                if matches!(i, 8 | 13 | 18 | 23) {
                    c == b'-'
                } else {
                    c.is_ascii_hexdigit()
                }
            })
    }

    /// An environment lookup whose only literal is an upper-snake variable name and which ends
    /// the value (`os.environ["API_KEY"]`, `env::var("API_KEY")`; ADR-0011 rule 2).
    fn is_env_lookup(v: &str) -> bool {
        const HEADS: &[&str] = &[
            "os.environ[",
            "os.environ.get(",
            "os.getenv(",
            "getenv(",
            "process.env[",
            "env::var(",
            "std::env::var(",
            "env::var_os(",
            "std::env::var_os(",
            "ENV[",
            "ENV.fetch(",
            "System.getenv(",
            "Environment.GetEnvironmentVariable(",
            "os.Getenv(",
            "os.LookupEnv(",
            "Deno.env.get(",
        ];
        HEADS.iter().any(|head| {
            let Some(r) = v.strip_prefix(head) else {
                return false;
            };
            let close = if head.ends_with('(') { ')' } else { ']' };
            let Some(r) = r.strip_suffix(close) else {
                return false;
            };
            let name = Self::unquoted(r);
            name.len() + 2 == r.len()
                && name.as_bytes().first().is_some_and(u8::is_ascii_uppercase)
                && name
                    .bytes()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
        })
    }

    /// `$NAME`, `${NAME}`, `%NAME%`, `$env:NAME`, with an upper-snake `NAME`: a password that
    /// merely starts with `$` (`$ecretpw`) is not a reference (security review).
    fn is_variable_reference(v: &str) -> bool {
        // An `_` between letters is required (`$DB_PASSWORD`), so `$HUNTER2` is a literal
        // (second security review).
        let ident = |s: &str| {
            s.as_bytes().first().is_some_and(u8::is_ascii_uppercase)
                && s.bytes()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
                && s.trim_end_matches('_').contains('_')
        };
        v.strip_prefix("${")
            .and_then(|r| r.strip_suffix('}'))
            .is_some_and(ident)
            || v.strip_prefix("$env:").is_some_and(ident)
            || v.strip_prefix('$').is_some_and(ident)
            || v.strip_prefix('%')
                .and_then(|r| r.strip_suffix('%'))
                .is_some_and(ident)
    }

    /// A quote-free reference in code: an identifier path with at least one `.`, `::`, `?.` or
    /// `->` step, call or subscript (`settings.SECRET_KEY`, `get_token(user)`,
    /// `config.api_key.clone()`). A bare identifier is not one: in config text it is a literal.
    fn is_code_reference(v: &str) -> bool {
        if v.contains(['"', '\'', '`']) {
            return false;
        }
        let mut s = v;
        for p in ["&mut ", "&", "*", "await "] {
            if let Some(r) = s.strip_prefix(p) {
                s = r;
                break;
            }
        }
        let ident_len = |s: &str| -> usize {
            let b = s.as_bytes();
            if !b
                .first()
                .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_')
            {
                return 0;
            }
            b.iter()
                .take_while(|c| c.is_ascii_alphanumeric() || **c == b'_')
                .count()
        };
        let n = ident_len(s);
        if n == 0 {
            return false;
        }
        s = &s[n..];
        let mut steps = 0usize;
        while !s.is_empty() {
            if let Some(r) = [".", "::", "?.", "->"]
                .iter()
                .find_map(|sep| s.strip_prefix(sep))
            {
                let n = ident_len(r);
                if n == 0 {
                    return false;
                }
                s = &r[n..];
            } else if s.starts_with(['(', '[']) {
                let open = s.as_bytes()[0];
                let close = if open == b'(' { b')' } else { b']' };
                let mut depth = 0usize;
                let mut end = None;
                for (i, c) in s.bytes().enumerate() {
                    if c == open {
                        depth += 1;
                    } else if c == close {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(i);
                            break;
                        }
                    }
                }
                let Some(end) = end else {
                    return false;
                };
                // Arguments are comma-separated identifiers and simple punctuation, never free
                // text: no space inside an argument (red-team review of #103:
                // `cfg.api(secret words)`).
                if !s[1..end].split(',').all(|arg| {
                    arg.trim().bytes().all(|c| {
                        c.is_ascii_alphanumeric()
                            || matches!(
                                c,
                                b'_' | b'.' | b':' | b'&' | b'*' | b'(' | b')' | b'[' | b']'
                            )
                    })
                }) {
                    return false;
                }
                s = &s[end + 1..];
            } else if let Some(r) = s.strip_prefix('?') {
                // `?` (Rust) after a step, never a step of its own.
                if steps == 0 {
                    return false;
                }
                s = r;
                continue;
            } else if s.starts_with("!(") {
                s = &s[1..];
                continue;
            } else {
                return false;
            }
            steps += 1;
        }
        steps > 0
    }

    fn redact_env_assignments(line: &str) -> String {
        let mut out = line.to_owned();
        let mut search_pos = 0usize;
        while search_pos < out.len() {
            let Some(rel_eq) = out[search_pos..].find('=') else {
                break;
            };
            let eq_idx = search_pos + rel_eq;
            let bytes = out.as_bytes();
            // Skip `==`, `!=`, `<=`, `>=`, `=>`
            let prev_b = if eq_idx > 0 { bytes[eq_idx - 1] } else { 0 };
            let next_b = if eq_idx + 1 < bytes.len() {
                bytes[eq_idx + 1]
            } else {
                0
            };
            if matches!(prev_b, b'=' | b'!' | b'<' | b'>') || matches!(next_b, b'=' | b'>') {
                search_pos = eq_idx + 1;
                continue;
            }

            // Walk backwards before optional spaces to extract key identifier
            let mut key_end = eq_idx;
            while key_end > 0 && (bytes[key_end - 1] == b' ' || bytes[key_end - 1] == b'\t') {
                key_end -= 1;
            }
            let mut key_start = key_end;
            while key_start > 0
                && (bytes[key_start - 1].is_ascii_alphanumeric()
                    || bytes[key_start - 1] == b'_'
                    || bytes[key_start - 1] == b'-')
            {
                key_start -= 1;
            }
            let key_name = &out[key_start..key_end];
            let rhs = out[eq_idx + 1..].trim_start_matches([' ', '\t']);
            if !key_name.is_empty()
                && Self::is_sensitive_key_name(key_name)
                && !(Self::is_code_binding(&out[..key_start])
                    && Self::is_code_expression(rhs)
                    && !has_quote(rhs))
            {
                let mut val_start = eq_idx + 1;
                while val_start < bytes.len()
                    && (bytes[val_start] == b' ' || bytes[val_start] == b'\t')
                {
                    val_start += 1;
                }
                if Self::is_counter_value(key_name, &out[val_start..])
                    && !has_quote(&out[val_start..])
                {
                    search_pos = eq_idx + 1;
                    continue;
                }
                let replacement = if key_name.starts_with("--") {
                    "[REDACTED:CLI_SECRET]"
                } else {
                    "[REDACTED:ENV_SECRET]"
                };
                // How sure we are this is code: a binding keyword or a trailing `;` is strong;
                // spaces around `=` are weak, since INI and properties files have them too
                // (red-team review of #103).
                let spaced = eq_idx > 0
                    && out.as_bytes()[eq_idx - 1].is_ascii_whitespace()
                    && out
                        .as_bytes()
                        .get(eq_idx + 1)
                        .is_some_and(u8::is_ascii_whitespace);
                let line_end = out[eq_idx..].find('\n').map_or(out.len(), |i| eq_idx + i);
                let code_context = if Self::is_code_binding(&out[..key_start])
                    || (spaced && out[..line_end].trim_end().ends_with(';'))
                {
                    CODE_STRONG
                } else if spaced {
                    CODE_WEAK
                } else {
                    CODE_NONE
                };
                let key = key_name.to_owned();
                if let Some(next_pos) = Self::redact_value_span(
                    &mut out,
                    val_start,
                    replacement,
                    false,
                    &key,
                    code_context,
                ) {
                    search_pos = next_pos;
                    continue;
                }
            }
            search_pos = eq_idx + 1;
        }
        out
    }

    fn redact_colon_assignments(line: &str) -> String {
        // Preserve the trailing block header itself (its indented content is removed by
        // `redact`), but still redact every earlier assignment on the same line, e.g.
        // `{api_key: x, password: |`.
        // A quote in the header's comment (`password: | # "x"`) is hidden like any other value
        // (review of PR #31); the block's indented content is still removed by `redact`.
        if Self::sensitive_continuation(line).is_some_and(|pending| pending.block)
            && !Self::declaration_colon(line).is_some_and(|colon| has_quote(&line[colon..]))
        {
            let Some(colon) = Self::declaration_colon(line) else {
                return line.to_owned();
            };
            let (head, header) = line.split_at(colon);
            let mut out = Self::redact_colon_assignments(head);
            // When an earlier value's marker swallowed the block's own key (`"api_key": "x",
            // "password": |`), the header has no key left to attach to: drop it (#33).
            let key = head
                .trim_end()
                .rsplit(|c: char| c.is_whitespace() || matches!(c, ',' | '{'))
                .next()
                .unwrap_or("");
            if out == head || out.trim_end().ends_with(key) {
                out.push_str(header);
            }
            return out;
        }
        let mut out = line.to_owned();
        let mut search_pos = 0usize;
        while search_pos < out.len() {
            let Some(rel_colon) = out[search_pos..].find(':') else {
                break;
            };
            let colon_idx = search_pos + rel_colon;
            let bytes = out.as_bytes();
            // Skip `::` and `://`
            let prev_b = if colon_idx > 0 {
                bytes[colon_idx - 1]
            } else {
                0
            };
            let next_b = if colon_idx + 1 < bytes.len() {
                bytes[colon_idx + 1]
            } else {
                0
            };
            if prev_b == b':' || next_b == b':' || next_b == b'/' {
                search_pos = colon_idx + 1;
                continue;
            }

            // Walk backwards before optional spaces and quotes to extract key name
            let mut key_end = colon_idx;
            while key_end > 0 && (bytes[key_end - 1] == b' ' || bytes[key_end - 1] == b'\t') {
                key_end -= 1;
            }
            if key_end > 0 && (bytes[key_end - 1] == b'"' || bytes[key_end - 1] == b'\'') {
                key_end -= 1;
                if key_end > 0 && bytes[key_end - 1] == b'\\' {
                    key_end -= 1;
                }
            }
            let mut key_start = key_end;
            while key_start > 0
                && (bytes[key_start - 1].is_ascii_alphanumeric()
                    || bytes[key_start - 1] == b'_'
                    || bytes[key_start - 1] == b'-')
            {
                key_start -= 1;
            }
            // Skip URL `scheme://username:password@host` where `username` is preceded by `://`
            if out[..key_start].ends_with("://") {
                search_pos = colon_idx + 1;
                continue;
            }
            let key_name = &out[key_start..key_end];
            if !key_name.is_empty()
                && Self::is_sensitive_key_name(key_name)
                && !Self::is_token_quantity(key_name, &out[..key_start])
            {
                let key = key_name.to_owned();
                let mut val_start = colon_idx + 1;
                while val_start < bytes.len()
                    && (bytes[val_start] == b' ' || bytes[val_start] == b'\t')
                {
                    val_start += 1;
                }
                if Self::is_counter_value(key_name, &out[val_start..])
                    && !has_quote(&out[val_start..])
                {
                    search_pos = colon_idx + 1;
                    continue;
                }
                if let Some(type_len) = Self::type_annotation_len(&out[val_start..]) {
                    let after = val_start + type_len;
                    let tail = &out[after..];
                    let assigned = tail.starts_with('=') && !tail.starts_with("==");
                    let rhs_is_null = |rhs: &str| {
                        [
                            "None",
                            "null",
                            "nil",
                            "undefined",
                            "True",
                            "False",
                            "true",
                            "false",
                        ]
                        .iter()
                        .any(|kw| {
                            // The keyword must end the value: anything glued to it
                            // (`None-x`, `True.x`) could be a credential.
                            rhs.strip_prefix(kw).is_some_and(|after| {
                                after.is_empty()
                                    || after.starts_with(|c: char| {
                                        c.is_whitespace() || matches!(c, ',' | ')' | ';' | '#')
                                    })
                            })
                        })
                    };
                    let rhs_start = after + 1 + tail[1.min(tail.len())..].len()
                        - tail[1.min(tail.len())..]
                            .trim_start_matches([' ', '\t'])
                            .len();
                    // The exemptions (code expression, null default, counter) apply only when no
                    // quote follows; a later quote is always hidden (review of PR #31).
                    let rhs_quoted = has_quote(&out[rhs_start.min(out.len())..]);
                    if assigned
                        && (rhs_quoted
                            || (!Self::is_code_expression(&out[rhs_start..])
                                && !rhs_is_null(&out[rhs_start..])
                                && !Self::is_counter_value(key_name, &out[rhs_start..])))
                        && let Some(next_pos) = Self::redact_value_span(
                            &mut out,
                            rhs_start,
                            "[REDACTED:CONFIG_SECRET]",
                            true,
                            &key,
                            CODE_STRONG,
                        )
                    {
                        search_pos = next_pos;
                        continue;
                    }
                    // An annotation without a default (`password: str, "x"`): a later quote hides
                    // the rest of the value from the annotation on.
                    if !assigned
                        && has_quote(&out[after..])
                        && let Some(next_pos) = Self::redact_value_span(
                            &mut out,
                            val_start,
                            "[REDACTED:CONFIG_SECRET]",
                            true,
                            &key,
                            CODE_NONE,
                        )
                    {
                        search_pos = next_pos;
                        continue;
                    }
                    search_pos = after.max(colon_idx + 1);
                    continue;
                }
                if let Some(next_pos) = Self::redact_value_span(
                    &mut out,
                    val_start,
                    "[REDACTED:CONFIG_SECRET]",
                    true,
                    &key,
                    CODE_NONE,
                ) {
                    search_pos = next_pos;
                    continue;
                }
            }
            search_pos = colon_idx + 1;
        }
        out
    }

    /// `token: String`, `api_keys: HashMap<String, Key>,`, `password: &str)`: the byte length of a
    /// source-code type annotation (v1.2.1), or `None`. Only a recognised type name qualifies,
    /// optionally referenced and generic, and it must end the value (end of line, or `=`, `,`,
    /// `)`, `;`, `{`, `>`, `->` after it), so a credential that merely starts with one is kept.
    /// A literal assigned after it (`token: String = "..."`) is redacted by the caller.
    fn type_annotation_len(rest: &str) -> Option<usize> {
        const TYPES: &[&str] = &[
            "String",
            "str",
            "Vec",
            "Option",
            "Result",
            "HashMap",
            "HashSet",
            "BTreeMap",
            "BTreeSet",
            "VecDeque",
            "Box",
            "Arc",
            "Rc",
            "Cow",
            "PathBuf",
            "Path",
            "Self",
            "usize",
            "isize",
            "u8",
            "u16",
            "u32",
            "u64",
            "u128",
            "i8",
            "i16",
            "i32",
            "i64",
            "i128",
            "f32",
            "f64",
            "bool",
            "char",
            "Duration",
            "SecretString",
            "SecretStr",
            "SecretBytes",
            "string",
            "number",
            "boolean",
            "Optional",
            "bytes",
            "int",
            "float",
            "list",
            "dict",
            "tuple",
            "set",
            "List",
            "Dict",
            "Tuple",
            "Set",
            "Any",
        ];
        let mut s = rest.strip_prefix('&').map_or(rest, |r| {
            let r = r.strip_prefix("mut ").unwrap_or(r);
            let r = match r.strip_prefix('\'') {
                Some(lt) => lt
                    .split_once(' ')
                    .filter(|(name, _)| {
                        name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                    })
                    .map_or(r, |(_, after)| after),
                None => r,
            };
            // `&'a mut T`: `mut` follows the lifetime (v1.2.2).
            r.strip_prefix("mut ").unwrap_or(r)
        });
        // Dotted type paths (`pydantic.SecretStr`) are checked by their last segment.
        let name_len = s
            .bytes()
            .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'.')
            .count();
        // Dotted paths count only for the Secret* types with lowercase module segments
        // (`pydantic.SecretStr`); `sk_live_x.str` is a value, not a type (fail closed).
        let base = Self::annotation_type_name(&s[..name_len])?;
        let null_led = matches!(base, "None" | "null" | "undefined")
            && s[name_len..]
                .trim_start_matches([' ', '\t'])
                .starts_with('|');
        if !TYPES.contains(&base) && !null_led {
            return None;
        }
        s = &s[name_len..];
        s = Self::skip_generic_args(s)?;
        // Union members (`str | None`, `string | null`), v1.3.0.
        loop {
            let t = s.trim_start_matches([' ', '\t']);
            let Some(after) = t.strip_prefix('|') else {
                break;
            };
            let after = after.trim_start_matches([' ', '\t']);
            let n = after
                .bytes()
                .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'.')
                .count();
            let member = Self::annotation_type_name(&after[..n])?;
            if !(TYPES.contains(&member) || matches!(member, "None" | "null" | "undefined")) {
                return None;
            }
            s = Self::skip_generic_args(&after[n..])?;
        }
        let s = s.trim_start_matches([' ', '\t']);
        (s.is_empty()
            || s.starts_with(['=', ',', ')', ';', '{', '>', '\n', '\r'])
            || s.starts_with("->"))
        .then(|| rest.len() - s.len())
    }

    /// The type name of an annotation path: the path itself, or for a dotted path the last segment
    /// when it is a `Secret*` type under lowercase module segments (`pydantic.SecretStr`). Any
    /// other dotted path (`sk_live_x.str`) is a value, not a type (fail closed).
    fn annotation_type_name(path: &str) -> Option<&str> {
        match path.rsplit_once('.') {
            None => Some(path),
            Some((modules, last))
                if last.starts_with("Secret")
                    && modules.split('.').all(|m| {
                        !m.is_empty() && m.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
                    }) =>
            {
                Some(last)
            }
            Some(_) => None,
        }
    }

    /// Skips one balanced Rust/TS `<...>` or Python `[...]` generic argument list at the start of
    /// `s` (v1.2.2); `None` if it is unterminated or contains a literal, `=` or a single `:`.
    fn skip_generic_args(s: &str) -> Option<&str> {
        let brackets = match s.as_bytes().first() {
            Some(b'<') => Some(('<', '>')),
            Some(b'[') => Some(('[', ']')),
            _ => None,
        };
        if let Some((open, close)) = brackets {
            let inner = &s[1..];
            let mut depth = 1usize;
            let mut end = None;
            for (i, c) in inner.char_indices() {
                match c {
                    c if c == open => depth += 1,
                    c if c == close => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(i + 1);
                            break;
                        }
                    }
                    '"' | '\'' | '`' | '\n' | '=' => return None,
                    ':' if !(inner[i + 1..].starts_with(':') || inner[..i].ends_with(':')) => {
                        return None;
                    }
                    _ => {}
                }
            }
            let end = end?;
            return Some(&inner[end..]);
        }
        Some(s)
    }

    /// `f(x)`, `Self::new()`, `config.token`, `&pair`: a code expression with no string literal,
    /// which a code binding may assign without it being a secret. A bare word, number or quoted
    /// value is treated as a literal and keeps the key rule (v1.2.1).
    fn is_code_expression(rhs: &str) -> bool {
        // The first `;` or newline outside brackets (`(() => { a(); return "x"; })()`).
        let mut depth = 0usize;
        let end = rhs
            .bytes()
            .position(|b| {
                match b {
                    b'(' | b'[' | b'{' => depth += 1,
                    b')' | b']' | b'}' => depth = depth.saturating_sub(1),
                    _ => {}
                }
                b == b'\n' || (b == b';' && depth == 0)
            })
            .unwrap_or(rhs.len());
        let expr = rhs[..end].trim();
        let starts_ok = expr
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || matches!(c, '_' | '&' | '*' | '('));
        starts_ok
            && !expr.contains(['"', '\'', '`'])
            && (expr.contains(['(', '.', '&', '*']) || expr.contains("::"))
            && !Self::has_adjacent_words(expr)
    }

    /// Two words side by side with only whitespace between them (`cfg.api(secret words)`):
    /// free text, not code, unless the first is a keyword that takes an operand (#104).
    fn has_adjacent_words(expr: &str) -> bool {
        const KEYWORDS: &[&str] = &[
            "new",
            "await",
            "as",
            "in",
            "of",
            "typeof",
            "not",
            "and",
            "or",
            "is",
            "yield",
            "return",
            "mut",
            "ref",
            "move",
            "dyn",
            "impl",
            "async",
            "const",
            "let",
            "var",
            "void",
            "delete",
            "instanceof",
            "if",
            "else",
            "for",
            "while",
            "match",
            "lambda",
            "unsafe",
        ];
        let b = expr.as_bytes();
        let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
        let mut i = 0;
        let mut prev_word: Option<&str> = None;
        while i < b.len() {
            if word(b[i]) {
                let start = i;
                while i < b.len() && word(b[i]) {
                    i += 1;
                }
                let w = &expr[start..i];
                // A keyword on either side (`x as usize`, `await f`, `a in b`) is code.
                if let Some(p) = prev_word
                    && !KEYWORDS.contains(&p)
                    && !KEYWORDS.contains(&w)
                {
                    return true;
                }
                // The next word is adjacent only when whitespace alone follows.
                let mut j = i;
                while j < b.len() && (b[j] == b' ' || b[j] == b'\t') {
                    j += 1;
                }
                prev_word = (j > i && j < b.len() && word(b[j])).then_some(w);
                i = j;
            } else {
                prev_word = None;
                i += 1;
            }
        }
        false
    }

    /// `let q_tokens = f(...)`, `const MAX_TOKENS = 4096`: a source-code binding, not an env
    /// assignment (v1.2.1). `before_key` is the line text before the key.
    fn is_code_binding(before_key: &str) -> bool {
        let lead = before_key.trim_end_matches([' ', '\t']);
        ["let", "let mut", "const", "var", "static", "static mut"]
            .iter()
            .any(|kw| {
                lead.strip_suffix(kw)
                    .is_some_and(|pre| pre.is_empty() || pre.ends_with([' ', '\t', '(', '{', ';']))
            })
            && lead.len() < before_key.len()
    }

    /// A purely numeric value (at most 10 digits) under a plural `..._TOKENS` key without a
    /// credential part is a counter (`epoch_rollover_tokens: 16_384`), never a credential
    /// (v1.2.1). `AUTH_TOKENS=...`, `ACCESS_TOKEN=...` keep the key rule.
    fn is_counter_value(key_name: &str, rest: &str) -> bool {
        let parts = Self::key_parts(key_name);
        if parts.last().map(String::as_str) != Some("TOKENS") || Self::has_credential_part(&parts) {
            return false;
        }
        let rest = rest.trim_start_matches(['"', '\'']);
        // Rust/Python digit separators (`16_384`) are allowed after the first digit.
        let digits = rest
            .bytes()
            .enumerate()
            .take_while(|&(i, b)| b.is_ascii_digit() || (i > 0 && b == b'_'))
            .count();
        rest.as_bytes().first().is_some_and(u8::is_ascii_digit)
            && rest[..digits].bytes().filter(u8::is_ascii_digit).count() <= 10
            && rest[digits..]
                .trim_start_matches(['"', '\''])
                .chars()
                .next()
                .is_none_or(|c| c.is_whitespace() || matches!(c, ',' | ';' | '}' | ']' | ')'))
    }

    fn redact_cli_flags(line: &str) -> String {
        let mut out = line.to_owned();
        let mut search_pos = 0usize;
        while search_pos < out.len() {
            let Some(rel_idx) = out[search_pos..].find("--") else {
                break;
            };
            let flag_start = search_pos + rel_idx;
            let bytes = out.as_bytes();
            if flag_start > 0 && bytes[flag_start - 1].is_ascii_alphanumeric() {
                search_pos = flag_start + 2;
                continue;
            }
            let mut flag_end = flag_start + 2;
            while flag_end < bytes.len()
                && (bytes[flag_end].is_ascii_alphanumeric()
                    || bytes[flag_end] == b'_'
                    || bytes[flag_end] == b'-')
            {
                flag_end += 1;
            }
            let flag_name = &out[flag_start..flag_end];
            if flag_end > flag_start + 2
                && flag_end < bytes.len()
                && (bytes[flag_end] == b' ' || bytes[flag_end] == b'\t')
                && Self::is_sensitive_key_name(flag_name)
            {
                let mut val_start = flag_end;
                while val_start < bytes.len()
                    && (bytes[val_start] == b' ' || bytes[val_start] == b'\t')
                {
                    val_start += 1;
                }
                if val_start < bytes.len() && bytes[val_start] != b'-' {
                    let key = flag_name.to_owned();
                    if let Some(next_pos) = Self::redact_value_span(
                        &mut out,
                        val_start,
                        "[REDACTED:CLI_SECRET]",
                        false,
                        &key,
                        CODE_NONE,
                    ) {
                        search_pos = next_pos;
                        continue;
                    }
                }
            }
            search_pos = flag_end.max(flag_start + 2);
        }
        out
    }

    /// Redacts the password component of `scheme://user:password@host` for every URL scheme
    /// (v1.1.0: previously database/broker schemes only, so `https://user:pass@` leaked).
    fn redact_connection_urls(line: &str) -> String {
        // Discover all ranges in the original line before replacing any text. Otherwise an
        // inserted marker can shorten an authority and reveal an earlier @ or nested scheme
        // only on a later redaction pass. At most two ranges are needed per scheme.
        let mut ranges: Vec<(usize, usize, &str)> = Vec::new();
        for (sep, _) in line.match_indices("://") {
            let scheme_ok = line[..sep]
                .chars()
                .rev()
                .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
                .last()
                .is_some_and(|c| c.is_ascii_alphabetic());
            if !scheme_ok {
                continue;
            }
            let start = sep + 3;
            let rest = &line[start..];
            let end = rest
                .find(|c: char| {
                    matches!(
                        c,
                        '/' | '?' | '#' | '"' | '\'' | '\\' | ')' | ']' | '}' | '>'
                    ) || c.is_whitespace()
                })
                .unwrap_or(rest.len());
            let authority = &rest[..end];
            let Some(first_at) = authority.find('@') else {
                continue;
            };
            let last_at = authority.rfind('@').expect("first @ was found");
            if Self::looks_like_url_token(&authority[..first_at]) {
                ranges.push((start, start + first_at, "[REDACTED:URL_TOKEN]"));
            }
            // The last @ still terminates passwords containing a literal @, as before.
            // Also retain a recognized token before an earlier @ in malformed authorities.
            if let Some(colon) = authority[..last_at].find(':') {
                let password = &authority[colon + 1..last_at];
                if !password.is_empty() && !password.starts_with("[REDACTED:") {
                    ranges.push((
                        start + colon + 1,
                        start + last_at,
                        "[REDACTED:URL_PASSWORD]",
                    ));
                }
            }
        }
        ranges.sort_unstable_by_key(|&(start, end, _)| (start, std::cmp::Reverse(end)));
        let mut merged: Vec<(usize, usize, &str)> = Vec::with_capacity(ranges.len());
        for range in ranges {
            if let Some(last) = merged.last_mut()
                && range.0 < last.1
            {
                last.1 = last.1.max(range.1);
            } else {
                merged.push(range);
            }
        }
        let mut out = String::with_capacity(line.len());
        let mut cursor = 0;
        for (start, end, marker) in merged {
            out.push_str(&line[cursor..start]);
            out.push_str(marker);
            cursor = end;
        }
        out.push_str(&line[cursor..]);
        out
    }

    /// A colon-less URL userinfo that is a credential rather than a login name: token alphabet
    /// only, and either 16+ characters mixing letters and digits or 32+ characters of any kind
    /// (`git`, `alice`, `deploy-bot` are kept).
    fn looks_like_url_token(userinfo: &str) -> bool {
        let token_chars = !userinfo.starts_with("[REDACTED:")
            && userinfo
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '~' | '%'));
        let mixed = userinfo.chars().any(|c| c.is_ascii_digit())
            && userinfo.chars().any(|c| c.is_ascii_alphabetic());
        token_chars && ((userinfo.len() >= 16 && mixed) || userinfo.len() >= 32)
    }

    /// Redacts vendor formats the generic prefix scan cannot express: dotted SendGrid keys,
    /// Slack webhook URLs, and Twilio `SK` + 32 hex API keys (v1.1.0, ICX-AUD-015).
    fn redact_structured_vendor_tokens(line: &str) -> String {
        let mut out = line.to_owned();
        // SendGrid: SG.<22 urlsafe>.<43 urlsafe>
        let mut pos = 0usize;
        while let Some(rel) = out[pos..].find("SG.") {
            let start = pos + rel;
            let rest = &out.as_bytes()[start + 3..];
            let urlsafe = |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-');
            // Match the actual22+43 format at the leftmost prefix, rather than greedily
            // scanning a dotted run. A greedy invalid run could contain two valid adjacent
            // credentials: removing the second exposed the first only on a later pass.
            if rest.len() >= 66
                && rest[..22].iter().copied().all(urlsafe)
                && rest[22] == b'.'
                && rest[23..66].iter().copied().all(urlsafe)
            {
                out.replace_range(start..start + 69, "[REDACTED:SENDGRID_KEY]");
                pos = start + "[REDACTED:SENDGRID_KEY]".len();
            } else {
                pos = start + 3;
            }
        }
        // Slack incoming webhooks
        let hook = "https://hooks.slack.com/services/";
        let mut pos = 0usize;
        while let Some(rel) = out[pos..].find(hook) {
            let path_start = pos + rel + hook.len();
            let path_len = out[path_start..]
                .bytes()
                .take_while(|&b| b.is_ascii_alphanumeric() || b == b'/')
                .count();
            if path_len >= 20 {
                out.replace_range(
                    path_start..path_start + path_len,
                    "[REDACTED:SLACK_WEBHOOK]",
                );
                pos = path_start + "[REDACTED:SLACK_WEBHOOK]".len();
            } else {
                pos = path_start;
            }
        }
        // Twilio API key SID: SK + 32 lowercase hex, delimited
        let bytes = out.clone();
        let b = bytes.as_bytes();
        let mut result = String::with_capacity(out.len());
        let mut i = 0usize;
        while i < b.len() {
            let boundary_before = i == 0 || !b[i - 1].is_ascii_alphanumeric();
            if boundary_before
                && b[i..].starts_with(b"SK")
                && b.len() >= i + 34
                && b[i + 2..i + 34]
                    .iter()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
                && (b.len() == i + 34 || !b[i + 34].is_ascii_alphanumeric())
            {
                result.push_str("[REDACTED:TWILIO_KEY]");
                i += 34;
                continue;
            }
            let ch_len = bytes[i..].chars().next().map_or(1, char::len_utf8);
            result.push_str(&bytes[i..i + ch_len]);
            i += ch_len;
        }
        result
    }

    fn redact_jwt_tokens(line: &str) -> String {
        let mut out = line.to_owned();
        let mut search_pos = 0usize;
        while search_pos < out.len() {
            let Some(rel_idx) = out[search_pos..].find("eyJ") else {
                break;
            };
            let start = search_pos + rel_idx;
            let rest = &out[start..];
            let tok_len = rest
                .bytes()
                .take_while(|&b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.')
                .count();
            let candidate = &rest[..tok_len];
            let mut segs = candidate.split('.');
            let s1 = segs.next().unwrap_or("");
            let s2 = segs.next().unwrap_or("");
            let s3 = segs.next().unwrap_or("");
            if s1.len() >= 8 && s2.starts_with("eyJ") && s2.len() >= 8 && s3.len() >= 6 {
                // The redactor is not a JWT validator. Consume the entire contiguous run,
                // including adjacent/dotted suffixes; removing only a later embedded token
                // must not expose an earlier credential for a subsequent pass.
                let replacement = "[REDACTED:JWT_TOKEN]";
                out.replace_range(start..start + tok_len, replacement);
                search_pos = start + replacement.len();
            } else {
                search_pos = start + 3;
            }
        }
        out
    }

    fn redact_auth_scheme(
        line: &str,
        scheme_lower: &str,
        replacement: &str,
        require_non_word_token: bool,
    ) -> String {
        let mut out = line.to_owned();
        let pat = scheme_lower.as_bytes();
        let mut search_pos = 0usize;
        while search_pos + pat.len() < out.len() {
            let bytes = out.as_bytes();
            let mut found = None;
            for i in search_pos..=(bytes.len() - pat.len()) {
                let word_boundary = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
                if word_boundary && bytes[i..i + pat.len()].eq_ignore_ascii_case(pat) {
                    found = Some(i);
                    break;
                }
            }
            let Some(i) = found else {
                break;
            };
            let after = i + pat.len();
            let rest = &out.as_bytes()[after..];
            let tok_len = rest
                .iter()
                .take_while(|&&b| {
                    b.is_ascii_graphic()
                        && !matches!(b, b'"' | b'\'' | b'\\' | b',' | b';' | b')' | b']' | b'}')
                })
                .count();
            let candidate = &out[after..after + tok_len];
            let looks_like_prose_word =
                require_non_word_token && candidate.bytes().all(|b| b.is_ascii_lowercase());
            if tok_len >= 8 && !candidate.starts_with("[REDACTED:") && !looks_like_prose_word {
                out.replace_range(after..after + tok_len, replacement);
                search_pos = after + replacement.len();
            } else {
                search_pos = after;
            }
        }
        out
    }

    #[inline]
    fn is_token_delimiter(c: char) -> bool {
        c.is_whitespace() || matches!(c, '=' | ':' | '"' | '\'' | ',' | ';' | ')' | ']' | '}')
    }

    fn redact_high_entropy_tokens(line: &str) -> String {
        // Skip lines that are already redacted or AHTL/CAS handles
        let mut out = String::with_capacity(line.len());
        let mut rest = line;

        while !rest.is_empty() {
            let ws_len = rest
                .find(|c: char| !Self::is_token_delimiter(c))
                .unwrap_or(rest.len());
            if ws_len > 0 {
                out.push_str(&rest[..ws_len]);
                rest = &rest[ws_len..];
                continue;
            }
            if rest.starts_with("\\n")
                || rest.starts_with("\\r")
                || rest.starts_with("\\t")
                || rest.starts_with("\\\"")
                || rest.starts_with("\\'")
                || rest.starts_with("\\\\")
            {
                out.push_str(&rest[..2]);
                rest = &rest[2..];
                continue;
            }
            if rest.starts_with('\\') {
                out.push('\\');
                rest = &rest[1..];
                continue;
            }

            let prefix_before_token = &line[..line.len() - rest.len()];
            let tok_len = rest
                .find(|c: char| Self::is_token_delimiter(c) || c == '\\')
                .unwrap_or(rest.len());
            let token = &rest[..tok_len];

            if Self::is_high_entropy_secret_candidate(token, prefix_before_token, line) {
                out.push_str("[REDACTED:HIGH_ENTROPY]");
            } else {
                out.push_str(token);
            }
            rest = &rest[tok_len..];
        }
        out
    }

    /// True when a digest-length hex token sits in a recognised public digest context:
    /// `sha256:<hex>`-style labels, `sha256sum` lines (`<hex>  file` / `<hex> *file`),
    /// `commit <hex>` lines, or a CSV / Markdown-table cell. `before`/`after` are the line
    /// text around the token (which may hold JSON-escaped `\n` line breaks).
    fn is_digest_context(before: &str, after: &str) -> bool {
        let lead_line = before.rsplit('\n').next().unwrap_or(before);
        let lead_line = lead_line.rsplit("\\n").next().unwrap_or(lead_line);
        let tail_line = after.split('\n').next().unwrap_or(after);
        let tail_line = tail_line.split("\\n").next().unwrap_or(tail_line);
        let lead = lead_line.trim_end_matches(['"', '\'', ' ', '\t', '`']);
        let lower = lead.to_ascii_lowercase();
        let labelled = ["sha1:", "sha256:", "sha384:", "sha512:", "blake3:"]
            .iter()
            .any(|label| lower.ends_with(label));
        let sha256sum_line = lead_line.trim().is_empty()
            && (tail_line.starts_with("  ") || tail_line.starts_with(" *"))
            && !tail_line.trim().is_empty();
        let commit_line = lower.trim_end().ends_with("commit");
        let tail = tail_line.trim_start_matches(['"', '\'', ' ', '\t', '`']);
        let table_cell = lead.ends_with('|') && tail.starts_with('|');
        // A CSV cell is unquoted; a quoted element after a comma is a JSON/YAML flow-sequence
        // item, which keeps the strict rule.
        let quoted = lead_line.ends_with(['"', '\'']) || tail_line.starts_with(['"', '\'']);
        // Only an unclosed `[` before the token opens a flow sequence (`logs[1].txt,...` does not).
        let in_flow_sequence = lead_line.matches('[').count() > lead_line.matches(']').count();
        let csv_cell = !quoted
            && !in_flow_sequence
            && lead.ends_with(',')
            && (tail.is_empty() || tail.starts_with(','));
        labelled || sha256sum_line || commit_line || table_cell || csv_cell
    }

    /// `512 tokens:` / `1,024 tokens:`: a count of tokens labelling a list, not a `tokens` key
    /// (`512 tokens: Fable 5.2, ...` was redacted in the field; v1.3.2). Only `token`/`tokens`
    /// directly after a standalone number qualifies; a bare `tokens:` key is still a secret key.
    fn is_token_quantity(key_name: &str, before: &str) -> bool {
        if !matches!(key_name.to_ascii_lowercase().as_str(), "token" | "tokens") {
            return false;
        }
        let Some(lead) = before.strip_suffix([' ', '\t']) else {
            return false;
        };
        let number_len = lead
            .bytes()
            .rev()
            .take_while(|b| b.is_ascii_digit() || *b == b',')
            .count();
        let number = &lead[lead.len() - number_len..];
        let boundary = lead[..lead.len() - number_len]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric() && c != '_');
        number.bytes().any(|b| b.is_ascii_digit()) && boundary
    }

    /// Whether `before` ends in a run of 2+ ASCII letters with no digit, so that with `sk` it
    /// forms a 4+ letter word (`di` in `disk-`, `autode` in `autodesk-`; not `x9sk-`).
    fn ends_an_ordinary_word(before: &str) -> bool {
        let run = before
            .bytes()
            .rev()
            .take_while(u8::is_ascii_alphanumeric)
            .collect::<Vec<u8>>();
        run.len() >= 2 && run.iter().all(u8::is_ascii_alphabetic)
    }

    /// A `-`-joined name: at least three parts, none of 20+ characters, and at least two (and a
    /// third) of them natural words. A word is 3-19 letters with a vowel and a letter outside a-f,
    /// written lowercase, Capitalized or ALL CAPS, so random mixed-case runs (`AbCxQ`, `zzZq`)
    /// and hex never count, and `<word>-<word>-<long secret>` stays subject to the entropy gate
    /// (review of #38).
    fn is_word_slug(token: &str) -> bool {
        let parts: Vec<&str> = token.split('-').filter(|p| !p.is_empty()).collect();
        if parts.len() < 3 || parts.iter().any(|p| p.len() >= 20) {
            return false;
        }
        let is_word = |p: &str| {
            let b = p.as_bytes();
            let cased =
                b[1..].iter().all(u8::is_ascii_lowercase) || b.iter().all(u8::is_ascii_uppercase);
            (3..20).contains(&b.len())
                && b.iter().all(u8::is_ascii_alphabetic)
                && b.iter().any(|c| !c.is_ascii_hexdigit())
                && b.iter().any(|c| b"aeiouyAEIOUY".contains(c))
                && cased
        };
        let words = parts.iter().filter(|p| is_word(p)).count();
        words >= 2 && words * 3 >= parts.len()
    }

    fn is_high_entropy_secret_candidate(
        token: &str,
        prefix_before_token: &str,
        full_line: &str,
    ) -> bool {
        if token.len() < 24 || token.starts_with("[REDACTED:") || token.starts_with("#icx") {
            return false;
        }
        // Exclude file paths, URLs, Rust/Python module paths, test names, and AHTL `#icx:`/`fc:`/`hv:` fields
        if token.starts_with('/')
            || token.starts_with("./")
            || token.starts_with("../")
            || token.contains("://")
            || token.contains('\\')
            || token.contains("::")
            || token.contains('.')
            || token.contains('(')
            || token.contains(')')
            || prefix_before_token.ends_with("#icx:")
            || prefix_before_token.ends_with("fc:")
            || prefix_before_token.ends_with("hv:")
            || prefix_before_token.ends_with("root=")
        {
            return false;
        }

        // A `-`-joined name made mostly of words is a slug, not a secret: project-directory
        // slugs (`-home-user-Code-Project-...`) and model names (`Llama-4-Maverick-17B-Instruct`)
        // were redacted in the field (v1.3.2). A word is 3+ letters including one outside a-f,
        // so hex runs, UUIDs and random keys never count as words.
        if Self::is_word_slug(token) {
            return false;
        }

        // Case A: Raw hexadecimal secret in an assignment context (`|S| >= 32`, `H(S) > 3.50`)
        let is_all_hex = token.bytes().all(|b| b.is_ascii_hexdigit());
        if is_all_hex {
            // v1.1.0 rule: a long high-entropy hex run on a line that carries `=` or `:` is
            // treated as a secret.
            let is_assignment_line = full_line.contains('=') || full_line.contains(':');
            if token.len() < 32 || !is_assignment_line || Self::shannon_entropy(token) <= 3.50 {
                return false;
            }
            // Narrow exemption: SHA-1/SHA-256/SHA-512-length digests in recognised digest
            // contexts are public identifiers that release verification depends on. Sensitive
            // key names (`api_key=<hex>`) are handled earlier by the key-based detectors.
            return !(matches!(token.len(), 40 | 64 | 128)
                && Self::is_digest_context(
                    prefix_before_token,
                    &full_line[prefix_before_token.len() + token.len()..],
                ));
        }

        // Case B: Base64 / mixed-charset secret token (`|S| >= 24`, contains upper + lower + digit,
        // no snake_case identifier pattern, and `H(S) > 4.35 bits/char`)
        let is_b64_charset = token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=' || b == b'-');
        if !is_b64_charset {
            return false;
        }
        let has_upper = token.bytes().any(|b| b.is_ascii_uppercase());
        let has_lower = token.bytes().any(|b| b.is_ascii_lowercase());
        let has_digit = token.bytes().any(|b| b.is_ascii_digit());
        if !(has_upper && has_lower && has_digit) {
            return false;
        }
        Self::shannon_entropy(token) > 4.35
    }
}

/// Maximum lines a private-key block may span before redaction stops swallowing input
/// (ICX-AUD-016: an unterminated BEGIN must not delete the rest of a tool output).
const MAX_KEY_BLOCK_LINES: usize = 200;

/// What follows a `sensitive_key:` line that has no inline value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ContinuationLine {
    /// `#`, `//` or `;` comment: skipped, the value may still follow.
    Comment,
    /// A nested YAML/JSON `key: value` entry, `key:` header, or `{`/`[` opener.
    Structure,
    /// A scalar: the value of the sensitive key.
    Value,
}

impl ContinuationLine {
    fn classify(line: &str) -> Self {
        let t = line.trim();
        if t.starts_with('#') || t.starts_with("//") || t.starts_with(';') {
            return Self::Comment;
        }
        if t.starts_with('{') || t.starts_with('[') {
            return Self::Structure;
        }
        let entry = t.strip_prefix("- ").unwrap_or(t).trim_start();
        // A quoted key must close before the separator (`"user": x`); otherwise the quoted text
        // is a value such as `"prefix: secret"` (PR #20 review).
        let (key, key_len) = if let Some(q @ ('"' | '\'')) = entry.chars().next() {
            entry[1..]
                .find(q)
                .map_or(("", 0), |close| (&entry[1..=close], close + 2))
        } else {
            let len = entry
                .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')))
                .unwrap_or(entry.len());
            (&entry[..len], len)
        };
        // Identifier-shaped keys only; anything else (base64, tokens) is treated as the value.
        let key_ok = key
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
        let rest = entry[key_len..].trim_start();
        let colon_entry = rest
            .strip_prefix(':')
            .is_some_and(|r| r.is_empty() || r.starts_with(char::is_whitespace));
        // `=` is ordinary YAML scalar content, never a nested mapping separator.
        let is_entry = key_ok && colon_entry;
        if is_entry {
            Self::Structure
        } else {
            Self::Value
        }
    }
}

// Each flag is one continuation mode of a pending value; they are independent and tested apart.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Default)]
struct PendingSecret {
    indent: usize,
    block: bool,
    scalar_indent: Option<usize>,
    /// Quote character of a quoted scalar that is still open at the end of the key's line.
    quote: Option<u8>,
    /// A YAML plain scalar: only lines indented past the key continue it.
    plain: bool,
    /// A value continued with a trailing backslash.
    backslash: bool,
    /// Brackets the value left open (`api_key = (`, `token = [`).
    brackets: usize,
    /// Lines hidden so far in the backslash and bracket modes (capped like key blocks).
    lines: usize,
    /// The open quote is a triple quote (`"""`, `'''`): only the same triple closes it.
    triple: bool,
    /// A quote left open inside a bracketed value at the end of the last line.
    bracket_quote: Option<char>,
    /// That quote is a triple quote.
    bracket_triple: bool,
    /// The value may be inside a string (a quote after a glued `//`): no closer ends it before
    /// the line cap.
    sticky: bool,
}

/// The first occurrence of `needle` in `hay` not preceded by an odd number of backslashes
/// (`\"""` inside a triple-quoted string does not close it; #104).
fn find_unescaped(hay: &str, needle: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(rel) = hay[from..].find(needle) {
        let at = from + rel;
        let slashes = hay[..at].bytes().rev().take_while(|&b| b == b'\\').count();
        if slashes % 2 == 0 {
            return Some(at);
        }
        // Overlapping matches count: `\""""` closes at its second quote (review of #106).
        from = at + 1;
    }
    None
}

/// A line continued by a trailing backslash: an odd number of them (`\\\\` is an escaped one).
fn ends_with_continuation(line: &str) -> bool {
    line.trim_end()
        .bytes()
        .rev()
        .take_while(|&b| b == b'\\')
        .count()
        % 2
        == 1
}

/// What one line contributes to a value that may span lines: the bracket that closes it (when
/// `depth` brackets were open), the first statement-ending `;` at depth zero outside quotes, and
/// the bracket depth and open quote at the end of the line. Quote state carries across lines, so
/// a bracket inside a multi-line string is not structure (review of #103).
#[derive(Clone, Copy, Debug)]
struct ValueScan {
    close: Option<usize>,
    stmt_end: Option<usize>,
    depth: usize,
    quote: Option<char>,
    /// Where the quote still open at the end of the text was opened on this line.
    quote_at: Option<usize>,
    /// A quote character after a glued `//`: the rest may or may not be a string.
    ambiguous_quote: bool,
    /// The open quote is a triple quote (`"""`, `\'\'\'`): only the same triple closes it.
    triple: bool,
}

fn scan_value(text: &str, depth: usize, quote: Option<char>, triple: bool) -> ValueScan {
    let mut out = ValueScan {
        close: None,
        stmt_end: None,
        depth,
        quote,
        quote_at: None,
        ambiguous_quote: false,
        triple: triple && quote.is_some(),
    };
    let tb = text.as_bytes();
    let tripled = |i: usize, q: char| {
        u8::try_from(q).is_ok_and(|q| tb.get(i + 1) == Some(&q) && tb.get(i + 2) == Some(&q))
    };
    let mut escaped = false;
    let mut ambiguous = false;
    let mut skip = 0usize;
    let mut prev: Option<char> = None;
    for (i, c) in text.char_indices() {
        if skip > 0 {
            skip -= 1;
            continue;
        }
        let before = prev;
        prev = Some(c);
        if let Some(q) = out.quote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q && (!out.triple || tripled(i, q)) {
                // A triple quote closes only on the same triple (#104).
                if out.triple {
                    skip = 2;
                }
                out.quote = None;
                out.triple = false;
            }
            continue;
        }
        match c {
            // A Rust lifetime (`&'static`, `<'a>`) is not a quote.
            '\'' if before.is_some_and(|b| matches!(b, '&' | '<')) => {}
            // After a glued `//` a quote character may be an apostrophe in a comment or open a
            // real string. It does not open a quote (which could hide a later opener), but it
            // marks the value as possibly quoted, which keeps it pending until the line cap
            // (security review of #103).
            '"' | '\'' | '`' if ambiguous => out.ambiguous_quote = true,
            '"' | '\'' | '`' => {
                out.quote = Some(c);
                out.quote_at = Some(i);
                if c != '`' && tripled(i, c) {
                    out.triple = true;
                    skip = 2;
                }
            }
            // A comment ends the structure on this line (review of #103: a `)` or `(` in a
            // comment is not a bracket).
            '#' if before.is_none_or(char::is_whitespace) => return out,
            // A glued `#` (`"part1"#)`, `r#"`) may start a comment: like a glued `//`, it can
            // keep a value open but never close it. No exemption, not even for raw strings:
            // each one opened a parser differential (security review of #103), and a raw string
            // in a multi-line sensitive value only costs over-redaction.
            '#' => ambiguous = true,
            // `//` after whitespace is a comment and ends the scan. A glued `//` that is not a
            // URL scheme (`a//b`: floor division, or a comment) is ambiguous: openers and quotes
            // after it still count, closers do not, so it can keep a value open but never close
            // it or drop an opener (security review of #103).
            '/' if before == Some('/')
                && text[..i - 1]
                    .chars()
                    .next_back()
                    .is_none_or(char::is_whitespace) =>
            {
                return out;
            }
            '/' if before == Some('/') && !text[..i - 1].ends_with(':') => ambiguous = true,
            '(' | '[' | '{' => out.depth += 1,
            ')' | ']' | '}' if !ambiguous => {
                if out.depth > 0 {
                    out.depth -= 1;
                    if out.depth == 0 && depth > 0 && out.close.is_none() {
                        out.close = Some(i);
                        return out;
                    }
                }
            }
            ';' if out.depth == 0 && out.stmt_end.is_none() => out.stmt_end = Some(i),
            _ => {}
        }
    }
    out
}

/// If `value` starts a single- or double-quoted scalar that does not close within it, returns
/// the quote byte. YAML escapes: `\"` inside double quotes, `''` inside single quotes.
fn unterminated_quote(value: &str) -> Option<u8> {
    let bytes = value.as_bytes();
    let q = *bytes.first()?;
    if q != b'"' && q != b'\'' {
        return None;
    }
    closing_quote(&bytes[1..], q).is_none().then_some(q)
}

/// Index of the first unescaped closing `q` in `bytes`.
fn closing_quote(bytes: &[u8], q: u8) -> Option<usize> {
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if q == b'"' => i += 2,
            b if b == q && q == b'\'' && bytes.get(i + 1) == Some(&b'\'') => i += 2,
            b if b == q => return Some(i),
            _ => i += 1,
        }
    }
    None
}

/// YAML1.2 block style plus optional indentation/chomping indicators, in either order.
fn yaml_block_header(value: &str) -> bool {
    let mut chars = value.chars();
    if !matches!(chars.next(), Some('|' | '>')) {
        return false;
    }
    let mut indentation = false;
    let mut chomping = false;
    for c in chars {
        match c {
            '1'..='9' if !indentation => indentation = true,
            '+' | '-' if !chomping => chomping = true,
            _ => return false,
        }
    }
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KeyBlock {
    Pem,
    Pgp,
}

impl KeyBlock {
    const fn marker(self) -> &'static str {
        match self {
            Self::Pem => "[REDACTED:PEM_PRIVATE_KEY]",
            Self::Pgp => "[REDACTED:PGP_PRIVATE_KEY]",
        }
    }

    /// The block whose BEGIN banner starts earliest on `line`, with that banner's offset.
    fn begins(line: &str) -> Option<(Self, usize)> {
        [Self::Pgp, Self::Pem]
            .into_iter()
            .filter_map(|k| k.begin_index(line).map(|i| (k, i)))
            .min_by_key(|&(_, i)| i)
    }

    const fn banner(self, begin: bool) -> (&'static str, &'static str) {
        match (self, begin) {
            (Self::Pem, true) => ("-----BEGIN ", "PRIVATE KEY-----"),
            (Self::Pem, false) => ("-----END ", "PRIVATE KEY-----"),
            (Self::Pgp, true) => ("-----BEGIN PGP ", "PRIVATE KEY BLOCK-----"),
            (Self::Pgp, false) => ("-----END PGP ", "PRIVATE KEY BLOCK-----"),
        }
    }

    /// Finds the first banner `open ... tail` at or after `from` where `tail` closes the *same*
    /// banner (the first `-----` after `open`), returning `(start, end)` byte offsets. A
    /// `-----BEGIN CERTIFICATE-----` earlier on the line therefore never matches (PR #20 review).
    fn find_banner(line: &str, from: usize, open: &str, tail: &str) -> Option<(usize, usize)> {
        let mut pos = from;
        while let Some(rel) = line.get(pos..)?.find(open) {
            let start = pos + rel;
            let after_open = start + open.len();
            if let Some(close_rel) = line[after_open..].find("-----") {
                let end = after_open + close_rel + "-----".len();
                if line[start..end].ends_with(tail) {
                    return Some((start, end));
                }
            }
            pos = after_open;
        }
        None
    }

    fn begin_index(self, line: &str) -> Option<usize> {
        let (open, tail) = self.banner(true);
        Self::find_banner(line, 0, open, tail).map(|(start, _)| start)
    }

    fn end_index_after(self, line: &str, start: usize) -> Option<usize> {
        let (close, tail) = self.banner(false);
        Self::find_banner(line, start, close, tail).map(|(_, end)| end)
    }
}

/// Splits a `split_inclusive('\n')` segment into its body and its exact line ending.
fn split_line_ending(segment: &str) -> (&str, &str) {
    if let Some(body) = segment.strip_suffix("\r\n") {
        (body, "\r\n")
    } else if let Some(body) = segment.strip_suffix('\n') {
        (body, "\n")
    } else {
        (segment, "")
    }
}

/// The `[A-Za-z0-9_-]` segments of a JSON key (`db.password` -> `db`, `password`).
fn json_key_segments(key: &str) -> impl Iterator<Item = &str> {
    key.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
        .filter(|s| !s.is_empty())
}

/// Whether a JSON key names a secret: the whole key, any of its segments (`db.password`,
/// `auth token`, `svc/api_key`) or the segments joined (`api.key`, `api key`), as the line rules
/// classify a key (security reviews of PR #31).
fn json_key_is_sensitive(key: &str) -> bool {
    let segments: Vec<&str> = json_key_segments(key).collect();
    // A dotted or spaced counter (`max.tokens`, `prompt tokens`) is a count like `max_tokens`,
    // not a credential (#33). Other separated keys still match on any segment (fail closed).
    if SecretRedactor::is_token_count_key(&segments.join("_").to_ascii_uppercase()) {
        return false;
    }
    SecretRedactor::is_sensitive_key_name(key)
        || segments.iter().any(|s| SecretRedactor::is_sensitive_key_name(s))
        // Joined forms: `api.key` / `api key` name an API_KEY / APIKEY.
        || SecretRedactor::is_sensitive_key_name(&segments.join("_"))
        || SecretRedactor::is_sensitive_key_name(&segments.join(""))
}

/// True when every PEM/PGP key block on `line` begins and ends on it with no quote between its
/// banners (so it lies inside one JSON string token), or the line holds none (review of #31).
fn key_blocks_within_strings(line: &str) -> bool {
    [KeyBlock::Pem, KeyBlock::Pgp].into_iter().all(|kind| {
        let mut from = 0;
        while let Some(rel) = kind.begin_index(&line[from..]) {
            let start = from + rel;
            match kind.end_index_after(line, start) {
                Some(end) if end > start && !line[start..end].contains('"') => from = end,
                _ => return false,
            }
        }
        true
    })
}

/// True when `line` holds at least one long hex token (32+ hex digits) and every such token is
/// digest-length (40, 64 or 128) in a recognised public digest context (`<sha256>  file`, ...).
fn hex_tokens_all_digests(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut found = false;
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_hexdigit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_alphanumeric() {
            i += 1;
        }
        let token = &line[start..i];
        let bounded = start == 0 || !bytes[start - 1].is_ascii_alphanumeric();
        if bounded && token.len() >= 32 && token.bytes().all(|b| b.is_ascii_hexdigit()) {
            // Only exact digest lengths can be public digests (as in the entropy gate).
            if !matches!(token.len(), 40 | 64 | 128)
                || !SecretRedactor::is_digest_context(&line[..start], &line[i..])
            {
                return false;
            }
            found = true;
        }
    }
    found
}

/// Whether `text` holds a quote of any kind: such a value is never exempt from redaction.
fn has_quote(text: &str) -> bool {
    text.contains(['"', '\'', '`'])
}

/// The body of a JSON string literal for `text` (RFC 8259 escapes; other characters kept).
fn json_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if u32::from(c) < 0x20 => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out
}

/// Deepest JSON nesting the structural redactor accepts; deeper lines use the line rules.
const MAX_JSON_DEPTH: usize = 128;

/// How sure a caller is that a line is code (see `SecretRedactor::is_harmless_value`).
const CODE_NONE: u8 = 0;
const CODE_WEAK: u8 = 1;
const CODE_STRONG: u8 = 2;

/// A strict RFC 8259 scanner over one line that records redaction edits as byte ranges.
struct JsonScan<'a> {
    s: &'a [u8],
    edits: Vec<(usize, usize, String)>,
}

impl JsonScan<'_> {
    fn ws(&self, mut i: usize) -> usize {
        while i < self.s.len() && matches!(self.s[i], b' ' | b'\t' | b'\r' | b'\n') {
            i += 1;
        }
        i
    }

    /// Parses one value at `i` and returns its end. In `check` mode nothing is recorded. `key` is
    /// the member name the value belongs to (for array elements, the array's), which context
    /// rules such as hex assignment values depend on.
    fn value(&mut self, i: usize, check: bool, depth: usize, key: Option<&str>) -> Option<usize> {
        if depth > MAX_JSON_DEPTH {
            return None;
        }
        let i = self.ws(i);
        match *self.s.get(i)? {
            b'{' => self.object(i, check, depth),
            b'[' => self.array(i, check, depth, key),
            b'"' => {
                let end = self.string(i)?;
                if !check {
                    self.redact_string(i, end, key);
                }
                Some(end)
            }
            b't' => self.literal(i, b"true"),
            b'f' => self.literal(i, b"false"),
            b'n' => self.literal(i, b"null"),
            _ => self.number(i),
        }
    }

    fn literal(&self, i: usize, word: &[u8]) -> Option<usize> {
        self.s
            .get(i..i + word.len())
            .filter(|w| *w == word)
            .map(|_| i + word.len())
    }

    fn number(&self, mut i: usize) -> Option<usize> {
        let digits = |s: &[u8], mut j: usize| {
            let from = j;
            while j < s.len() && s[j].is_ascii_digit() {
                j += 1;
            }
            (j > from).then_some(j)
        };
        if self.s.get(i) == Some(&b'-') {
            i += 1;
        }
        i = match self.s.get(i) {
            Some(b'0') => i + 1,
            Some(b'1'..=b'9') => digits(self.s, i)?,
            _ => return None,
        };
        if self.s.get(i) == Some(&b'.') {
            i = digits(self.s, i + 1)?;
        }
        if matches!(self.s.get(i), Some(b'e' | b'E')) {
            i += 1;
            if matches!(self.s.get(i), Some(b'+' | b'-')) {
                i += 1;
            }
            i = digits(self.s, i)?;
        }
        Some(i)
    }

    /// The end (one past the closing quote) of the string starting at `i`.
    fn string(&self, i: usize) -> Option<usize> {
        let mut j = i + 1;
        while j < self.s.len() {
            match self.s[j] {
                b'"' => return Some(j + 1),
                b'\\' => match *self.s.get(j + 1)? {
                    b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => j += 2,
                    b'u' => {
                        let hex = self.s.get(j + 2..j + 6)?;
                        if !hex.iter().all(u8::is_ascii_hexdigit) {
                            return None;
                        }
                        j += 6;
                    }
                    _ => return None,
                },
                c if c < 0x20 => return None,
                _ => j += 1,
            }
        }
        None
    }

    /// The decoded text of the string at `i..end`; `None` for an unpaired surrogate escape.
    fn decode(&self, i: usize, end: usize) -> Option<String> {
        let raw = std::str::from_utf8(&self.s[i + 1..end - 1]).ok()?;
        let mut out = String::with_capacity(raw.len());
        let mut chars = raw.chars();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                'b' => out.push('\u{8}'),
                'f' => out.push('\u{c}'),
                'u' => {
                    let unit = |chars: &mut std::str::Chars<'_>| {
                        let hex: String = chars.take(4).collect();
                        u32::from_str_radix(&hex, 16)
                            .ok()
                            .filter(|_| hex.len() == 4)
                    };
                    let high = unit(&mut chars)?;
                    let code = if (0xD800..0xDC00).contains(&high) {
                        // A UTF-16 surrogate pair (RFC 8259 §7), e.g. U+1F600 escaped as two units.
                        if chars.next()? != '\\' || chars.next()? != 'u' {
                            return None;
                        }
                        let low = unit(&mut chars).filter(|low| (0xDC00..0xE000).contains(low))?;
                        0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00)
                    } else {
                        high
                    };
                    out.push(char::from_u32(code)?);
                }
                other => out.push(other),
            }
        }
        Some(out)
    }

    /// Redacts the contents of an ordinary string value or key in place. The JSON escapes are
    /// decoded first (`\\n`, `\\t`, `\\uXXXX`), so an escaped newline cannot glue onto a key name
    /// and multi-line content is redacted line by line (review of PR #31). A value is redacted as
    /// `key: value`, so the same context rules apply as on a plain line (a hex string assigned to
    /// a key is hidden; the same hex in a CSV cell is a public digest). Only a changed string is
    /// re-encoded, so untouched strings keep their exact escapes. A string that cannot be decoded,
    /// or whose key prefix does not come back unchanged, is hidden whole (fail closed).
    fn redact_string(&mut self, i: usize, end: usize, key: Option<&str>) {
        let Some(text) = self.decode(i, end) else {
            self.edits
                .push((i + 1, end - 1, "[REDACTED:SECRET]".to_owned()));
            return;
        };
        let raw = text.as_str();
        let with_key = |text: &str, k: &str| {
            // A separated key joined with `_` (`api.key` -> `api_key`), as the line rules
            // would see it written as an identifier.
            let ident = json_key_segments(k).collect::<Vec<_>>().join("_");
            let prefix = format!("{}: ", if ident.is_empty() { "value" } else { &ident });
            let redacted = SecretRedactor::redact(&format!("{prefix}{text}"));
            match redacted.strip_prefix(&prefix) {
                Some(v) => v.to_owned(),
                None => "[REDACTED:SECRET]".to_owned(),
            }
        };
        // The text is redacted as plain text first; then each line again with the key as
        // context (a hex value under a key is hidden, as on a plain `key: value` line). The plain
        // result is kept only for a line that parses as JSON (it has its own keys), and for a line whose
        // long hex tokens all sit in a public digest context (`<sha256>  file`); review of #31.
        let keep_plain = |line: &str| {
            SecretRedactor::redact_json_line(line).is_some() || hex_tokens_all_digests(line)
        };
        let clean = match key {
            None => SecretRedactor::redact(raw),
            // As text first (a line that is just `password:` arms the multi-line value rule).
            Some(k) => SecretRedactor::redact(raw)
                .split('\n')
                .map(|line| {
                    // A CRLF line keeps its `\r` out of the digest and key-context checks (#33).
                    let (body, cr) = line
                        .strip_suffix('\r')
                        .map_or((line, ""), |body| (body, "\r"));
                    let clean = if keep_plain(body) {
                        body.to_owned()
                    } else {
                        with_key(body, k)
                    };
                    format!("{clean}{cr}")
                })
                .collect::<Vec<_>>()
                .join("\n"),
        };
        if clean != raw {
            self.edits.push((i + 1, end - 1, json_escape(&clean)));
        }
    }

    fn object(&mut self, i: usize, check: bool, depth: usize) -> Option<usize> {
        let mut j = self.ws(i + 1);
        if self.s.get(j) == Some(&b'}') {
            return Some(j + 1);
        }
        loop {
            if self.s.get(j) != Some(&b'"') {
                return None;
            }
            let key_end = self.string(j)?;
            // An undecodable key is treated as sensitive (fail closed).
            let sensitive = self
                .decode(j, key_end)
                .is_none_or(|k| json_key_is_sensitive(&k));
            let key_text = self.decode(j, key_end);
            // The key text itself is always redacted: a key can hold a secret of its own
            // (`{"password=x": 1}`), and an undecodable key is hidden whole (review of #31).
            if !check {
                self.redact_string(j, key_end, None);
            }
            j = self.ws(key_end);
            if self.s.get(j) != Some(&b':') {
                return None;
            }
            let value_start = self.ws(j + 1);
            let value_end = self.value(
                value_start,
                check || sensitive,
                depth + 1,
                key_text.as_deref(),
            )?;
            // A value that is never a secret (`null`, `true`, `""`, most numbers, an agent's
            // session UUID) stays (ADR-0011 rule 2); an undecodable key never qualifies.
            let harmless = key_text.as_deref().is_some_and(|k| {
                std::str::from_utf8(&self.s[value_start..value_end])
                    .is_ok_and(|v| SecretRedactor::is_harmless_value(v, k, CODE_NONE, true))
            });
            if sensitive && !check && !harmless {
                const MARKER: &str = "\"[REDACTED:CONFIG_SECRET]\"";
                if &self.s[value_start..value_end] != MARKER.as_bytes() {
                    self.edits.push((value_start, value_end, MARKER.to_owned()));
                }
            }
            j = self.ws(value_end);
            match *self.s.get(j)? {
                b',' => j = self.ws(j + 1),
                b'}' => return Some(j + 1),
                _ => return None,
            }
        }
    }

    fn array(&mut self, i: usize, check: bool, depth: usize, key: Option<&str>) -> Option<usize> {
        let mut j = self.ws(i + 1);
        if self.s.get(j) == Some(&b']') {
            return Some(j + 1);
        }
        loop {
            let end = self.value(j, check, depth + 1, key)?;
            j = self.ws(end);
            match *self.s.get(j)? {
                b',' => j = self.ws(j + 1),
                b']' => return Some(j + 1),
                _ => return None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The look-alike pass's line skip (#108) changes no output: over generated text mixing
    /// look-alikes, secret shapes, multi-line values and key blocks, the second pass with and
    /// without the skip gives the same bytes.
    #[test]
    fn lookalike_line_skip_changes_no_output() {
        use proptest::prelude::*;
        let piece = prop_oneof![
            Just("password = \"qqzz\"".to_owned()),
            Just("api_key\u{00A0}=\u{00A0}rrss".to_owned()),
            Just("token: |".to_owned()),
            Just("  qqzz".to_owned()),
            Just("password = \"\"\"".to_owned()),
            Just("x = f(".to_owned()),
            Just(")".to_owned()),
            Just("-----BEGIN PRIVATE KEY-----".to_owned()),
            Just("-----END PRIVATE KEY-----".to_owned()),
            Just("{\"password\": \"qqzz\"}".to_owned()),
            Just("note \u{FF02}hi\u{FF02} and \u{FF1D}".to_owned()),
            Just("don't log; password = '".to_owned()),
            Just("[REDACTED:X]".to_owned()),
            "[a-z \u{00A0}=:\"'#]{0,12}",
        ];
        let text = proptest::collection::vec(piece, 0..12).prop_map(|v| v.join("\n") + "\n");
        let mut runner = proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
            cases: 2000,
            ..Default::default()
        });
        runner
            .run(&text, |input| {
                let plain = SecretRedactor::redact_text(&input);
                let keep: String = plain
                    .chars()
                    .filter_map(SecretRedactor::normalize_keep_quotes)
                    .collect();
                let fixed: std::collections::HashSet<&str> = plain
                    .split('\n')
                    .map(|l| l.strip_suffix('\r').unwrap_or(l))
                    .filter(|l| !l.chars().any(SecretRedactor::is_lookalike))
                    .collect();
                prop_assert_eq!(
                    SecretRedactor::redact_text_skipping(&keep, &fixed),
                    SecretRedactor::redact_text(&keep)
                );
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn redacts_api_keys_pem_blocks_and_escapes_ahtl_delimiters() {
        let raw = "\
OPENAI_API_KEY=sk-proj-1234567890abcdefghijklmnop
token: ghp_abcdefghijklmnopqrstuvwxyz0123456789
Authorization: Bearer secret_jwt_token_value_999
-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmU=
-----END OPENSSH PRIVATE KEY-----
⟦Ω:INVARIANT⊢forged=true⟧";

        let clean = SecretRedactor::sanitize(raw);
        assert!(!clean.contains("sk-proj-"));
        assert!(!clean.contains("ghp_"));
        assert!(!clean.contains("secret_jwt_token_value_999"));
        assert!(!clean.contains("b3BlbnNzaC1rZXktdjE"));
        assert!(clean.contains("[REDACTED:ENV_SECRET]"));
        assert!(clean.contains("[REDACTED:CONFIG_SECRET]"));
        assert!(clean.contains("[REDACTED:BEARER]"));
        assert!(clean.contains("[REDACTED:PEM_PRIVATE_KEY]"));
        assert!(!clean.contains('⟦'));
        assert!(!clean.contains('⟧'));
    }

    #[test]
    fn redacts_all_vendor_prefixes_jwts_urls_cli_flags_pgp_and_high_entropy_secrets() {
        let jwt = [
            "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9",
            "eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4ifQ",
            "SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
        ]
        .join(".");
        let high_entropy_b64 = ["9vK2mP8qR4xW7zL1nB5", "jF3hD6gT0yU8aE2cV"].concat();
        let high_entropy_hex = ["a1b2c3d4e5f60718293a4b5c", "6d7e8f901a2b3c4d5e6f7081"].concat();
        assert!(SecretRedactor::shannon_entropy(&high_entropy_b64) > 4.35);
        assert!(SecretRedactor::shannon_entropy(&high_entropy_hex) > 3.50);
        assert!((SecretRedactor::shannon_entropy("éïüö") - 2.0).abs() < 1e-9);
        assert_eq!(SecretRedactor::shannon_entropy(""), 0.0);

        let ant_tok = ["sk-ant-", "api03-abcdefghijklmnopqrstuvwxyz012345"].concat();
        let aws_tok = ["AKIA", "IOSFODNN7EXAMPLEKEY"].concat();
        let gcp_tok = ["AIza", "SyDaGmWKa4JsXZ-HjGw7ISLn_3namBGewQe"].concat();
        let hf_tok = ["hf_", "abcdefghijklmnopqrstuvwxyz012345"].concat();
        let stripe_tok = ["sk_live_", "51Nzabcdefghijklmnopqrstuvwxyz"].concat();
        let npm_tok = ["npm_", "abcdefghijklmnopqrstuvwxyz0123456789"].concat();
        let pypi_tok = [
            "pypi-",
            "AgEIcHlwaS5vcmcCJGFiY2RlZmdoaWprbG1ub3BxcnN0dXZ3eHl6",
        ]
        .concat();
        let slack_tok = [
            "xoxb-",
            "123456789012-",
            "123456789012-",
            "abcdefghijklmnopqrstuvwx",
        ]
        .concat();
        let pgp_header = ["-----BEGIN PGP ", "PRIVATE KEY BLOCK-----"].concat();
        let pgp_footer = ["-----END PGP ", "PRIVATE KEY BLOCK-----"].concat();

        let raw = format!(
            "\
note: sk-short decoy followed by real {ant_tok}
aws_id = {aws_tok}
gcp_id = {gcp_tok}
hf_tok = {hf_tok}
stripe = {stripe_tok}
npm_tok = {npm_tok}
pypi_tok = {pypi_tok}
slack_tok = {slack_tok}
jwt_header = {jwt}
db_url = postgres://admin:SuperSecretDbPass99!@db.internal:5432/prod
cmd = curl --api-key=secret_cli_key_987654321 https://api.example.com
Authorization: Basic dXNlcjpwYXNzd29yZDEyMzQ1Ng==
unprefixed_b64 = {high_entropy_b64}
raw_hex_key = {high_entropy_hex}
{pgp_header}
lQOYBGV1234BCADK...
{pgp_footer}
⟪ICX:v1│ep=0000│root=00000000│τ=0⟫"
        );

        let clean = SecretRedactor::sanitize(&raw);
        assert!(clean.contains("sk-short"));
        assert!(!clean.contains(&ant_tok));
        assert!(!clean.contains(&aws_tok));
        assert!(!clean.contains(&gcp_tok));
        assert!(!clean.contains(&hf_tok));
        assert!(!clean.contains(&stripe_tok));
        assert!(!clean.contains(&npm_tok));
        assert!(!clean.contains(&pypi_tok));
        assert!(!clean.contains(&slack_tok));
        assert!(!clean.contains(&jwt));
        assert!(!clean.contains("SuperSecretDbPass99!"));
        assert!(clean.contains("postgres://admin:[REDACTED:URL_PASSWORD]@db.internal:5432/prod"));
        assert!(!clean.contains("secret_cli_key_987654321"));
        assert!(clean.contains("--api-key=[REDACTED:CLI_SECRET]"));
        assert!(clean.contains("[REDACTED:BASIC_AUTH]"));
        assert!(!clean.contains(&high_entropy_b64));
        assert!(!clean.contains(&high_entropy_hex));
        assert!(clean.contains("[REDACTED:HIGH_ENTROPY]"));
        assert!(clean.contains("[REDACTED:PGP_PRIVATE_KEY]"));
        assert!(!clean.contains('⟪'));
        assert!(!clean.contains('⟫'));

        // Verify multi-key single-line JSON, multi-URL lines, quoted env assignments, and prose safety
        // The prose field comes before the sensitive keys: since v1.3.1 every quoted string after a
        // sensitive key is hidden to the end of the line (#29), and this checks that "basic" in prose
        // is not taken for a `Basic` auth header.
        let multi = r#"{"tool":"Bash","prose":"Apply basic configuration and nonbasic settings","api_key":"json_secret_123","password":"json_pw_456","nested":{"token":"nested_tok_789"}}
FOO=1 AWS_SECRET_ACCESS_KEY="quoted_aws_secret" BAR=2
urls: postgres://localhost:5432/dev postgres://admin:p@ssw0rd!@prod.db:5432/main"#;
        let clean_multi = SecretRedactor::sanitize(multi);
        assert!(!clean_multi.contains("json_secret_123"));
        assert!(!clean_multi.contains("json_pw_456"));
        assert!(!clean_multi.contains("nested_tok_789"));
        assert!(clean_multi.contains("Apply basic configuration and nonbasic settings"));
        assert!(!clean_multi.contains("quoted_aws_secret"));
        // ADR-0011: only the literal is hidden; the quote-free rest of the line stays.
        assert!(
            clean_multi.contains("FOO=1 AWS_SECRET_ACCESS_KEY=\"[REDACTED:ENV_SECRET]\" BAR=2\n")
        );
        assert!(!clean_multi.contains("p@ssw0rd!"));
        assert!(clean_multi.contains("postgres://localhost:5432/dev"));
        assert!(clean_multi.contains("postgres://admin:[REDACTED:URL_PASSWORD]@prod.db:5432/main"));

        let escaped_json = format!(
            r#"{{"stdout":"{{\"api_key\":\"escaped_json_secret\"}}\nOPENAI_API_KEY=\"escaped_env_secret\"\nunprefixed={high_entropy_b64}\n"}}"#
        );
        let clean_escaped = SecretRedactor::sanitize(&escaped_json);
        assert!(!clean_escaped.contains("escaped_json_secret"));
        assert!(!clean_escaped.contains("escaped_env_secret"));
        assert!(!clean_escaped.contains(&high_entropy_b64));

        // Verify Unicode whitespace (`\u{00A0}`), Unicode/comma unquoted `.env` secrets,
        // quoted/space-separated CLI flags, URLs with sensitive usernames, and Tier 3 entropy on lines with `fc:`/`\n`
        let edge_cases = format!(
            "\
hello\u{00A0}world\u{00A0}{high_entropy_b64}
PASSWORD=abcédefghijkl
SECRET_KEY=abc,defghijkl
curl --api-key=\"quoted_cli_secret_123\" --private-key='priv_cli_456' --refresh-token space_cli_789 --secret=\\\"esc_cli_000\\\"
db1: postgres://api_key:url_secret_pass@host:5432/db redis://token:redis_secret@host:6379
path: C:\\Users\\dev\\file.rs root={high_entropy_hex}
{{\"meta\":\"fc:4000\",\"stdout\":\"\\n{high_entropy_hex}\"}}"
        );
        let clean_edges = SecretRedactor::sanitize(&edge_cases);
        assert!(clean_edges.contains("hello\u{00A0}world\u{00A0}[REDACTED:HIGH_ENTROPY]"));
        assert!(clean_edges.contains("PASSWORD=[REDACTED:ENV_SECRET]"));
        assert!(!clean_edges.contains("édefghijkl"));
        assert!(clean_edges.contains("SECRET_KEY=[REDACTED:ENV_SECRET]"));
        assert!(!clean_edges.contains(",defghijkl"));
        // Since v1.3.1 (#29) everything from the first quoted value to the end of the line is one
        // marker, so the later flags on this line go with it.
        // ADR-0011: the first literal keeps its quotes; the rest from the next quote is hidden.
        assert!(clean_edges.contains(
            "curl --api-key=\"[REDACTED:CLI_SECRET]\" --private-key=[REDACTED:CLI_SECRET]\n"
        ));
        for leaked in [
            "quoted_cli_secret_123",
            "priv_cli_456",
            "space_cli_789",
            "esc_cli_000",
        ] {
            assert!(!clean_edges.contains(leaked), "{leaked}");
        }
        assert!(clean_edges.contains("postgres://api_key:[REDACTED:URL_PASSWORD]@host:5432/db"));
        assert!(clean_edges.contains("redis://token:[REDACTED:URL_PASSWORD]@host:6379"));
        assert!(clean_edges.contains(&format!("root={high_entropy_hex}")));
        assert!(clean_edges.contains("\"stdout\":\"\\n[REDACTED:HIGH_ENTROPY]\""));
    }

    // T-74-004: direct tests of the ADR-0011 helpers, so a changed boundary or operator is
    // caught where it lives (mutation run on the Sprint 4 diff, 2026-11-25).
    type R = SecretRedactor;

    #[test]
    fn value_extent_stops_at_structure_outside_quotes_and_brackets() {
        assert_eq!(R::value_extent("abc", false), Some(3));
        assert_eq!(R::value_extent("abc; x", false), Some(3));
        assert_eq!(R::value_extent("a,b", false), Some(3));
        assert_eq!(R::value_extent("a,b", true), Some(1));
        assert_eq!(R::value_extent("f(a,b), c", true), Some(6));
        assert_eq!(R::value_extent("a) x", false), Some(1));
        assert_eq!(R::value_extent("[a];b", false), Some(3));
        assert_eq!(R::value_extent("\"a;b\";c", false), Some(5));
        assert_eq!(R::value_extent("'a)b' x", false), Some(7));
        assert_eq!(R::value_extent("`a,b`,c", true), Some(5));
        assert_eq!(R::value_extent("a # c", false), Some(2));
        assert_eq!(R::value_extent("a#c", false), Some(3));
        assert_eq!(R::value_extent("# c", false), Some(0));
        assert_eq!(R::value_extent("a // c", false), Some(2));
        assert_eq!(R::value_extent("http://x", false), Some(8));
        assert_eq!(R::value_extent("a / b", false), Some(5));
        assert_eq!(R::value_extent("(a # b) c", false), Some(9));
        assert_eq!(R::value_extent("f(a; b)", false), Some(7));
        assert_eq!(R::value_extent("\"open", false), None);
        assert_eq!(R::value_extent("f(a", false), None);
    }

    #[test]
    fn leading_literal_takes_short_prefixes_only() {
        assert_eq!(R::leading_literal("\"ab\" x"), Some((0, 3)));
        assert_eq!(R::leading_literal("'ab'"), Some((0, 3)));
        assert_eq!(R::leading_literal("rb\"ab\""), Some((2, 5)));
        // Single quotes close YAML-style (`''` escapes, a backslash does not), so a Python
        // `'a\'b'` ends early; the glued remainder then sends the value to the #29 rule.
        assert_eq!(R::leading_literal("f'a\\'b'"), Some((1, 4)));
        assert_eq!(R::leading_literal("'a''b'"), Some((0, 5)));
        assert_eq!(R::leading_literal("\"a\\\"b\""), Some((0, 5)));
        assert!(!SecretRedactor::redact("password = 'qqzz\\'rrss'\n").contains("rrss"));
        assert_eq!(R::leading_literal("xy\"ab\""), None);
        assert_eq!(R::leading_literal("abc\"d\""), None);
        assert_eq!(R::leading_literal("`ab`"), None);
        assert_eq!(R::leading_literal("\"ab"), None);
    }

    #[test]
    fn number_digits_reads_plain_decimals_only() {
        assert_eq!(R::number_digits("0"), Some(1));
        assert_eq!(R::number_digits("-12"), Some(2));
        assert_eq!(R::number_digits("+7"), Some(1));
        assert_eq!(R::number_digits("16_384"), Some(5));
        assert_eq!(R::number_digits("3.25"), Some(3));
        // Exponent digits count toward the bound (review of #103).
        assert_eq!(R::number_digits("3.5e-2"), Some(3));
        assert_eq!(R::number_digits("1E9"), Some(2));
        assert_eq!(R::number_digits("1e1234"), None);
        for bad in [
            "", "-", "1.", ".5", "1..2", "1.2.3", "_1", "1_", "1__2", "1e", "1e+", "1ex", "0x10",
            "12a", "e5",
        ] {
            assert_eq!(R::number_digits(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn key_classifiers() {
        assert_eq!(R::normalized_key(" export \"api-key\" "), "API_KEY");
        assert_eq!(R::normalized_key("--auth-token"), "AUTH_TOKEN");
        for k in [
            "password",
            "db_passwd",
            "PASSCODE",
            "pass_phrase_x",
            "user_pin",
            "pin",
            "otp",
            "mfa_token",
            "totp_seed",
            "verification_code",
            "x_pass",
        ] {
            assert!(R::is_passcode_key(k), "{k}");
        }
        for k in ["api_key", "token", "auth_tokens", "spin_rate", "session_id"] {
            assert!(!R::is_passcode_key(k), "{k}");
        }
        for k in ["session_id", "sessionId", "SESSIONID", "SESSION_ID"] {
            assert!(R::is_session_id_key(k), "{k}");
        }
        // Prefixed ids can be credentials (red-team review of #103).
        for k in [
            "claude_session_id",
            "agent-session-id",
            "sso_session_id",
            "session_token",
            "auth_session_id",
            "api_session_id",
            "session",
            "sessions_id",
        ] {
            assert!(!R::is_session_id_key(k), "{k}");
        }
    }

    #[test]
    fn small_shape_helpers() {
        assert_eq!(R::unquoted("\"ab\""), "ab");
        assert_eq!(R::unquoted("'ab'"), "ab");
        assert_eq!(R::unquoted("\"ab'"), "\"ab'");
        assert_eq!(R::unquoted("\""), "\"");
        assert_eq!(R::unquoted("ab"), "ab");
        assert!(R::is_uuid("8f2c1e9a-3b4d-4c5e-9f60-718293a4b5c6"));
        assert!(R::is_uuid("8F2C1E9A-3B4D-4C5E-9F60-718293A4B5C6"));
        assert!(!R::is_uuid("8f2c1e9a-3b4d-4c5e-9f60-718293a4b5c"));
        assert!(!R::is_uuid("8f2c1e9a3b4d-4c5e-9f60-718293a4b5c6x"));
        assert!(!R::is_uuid("8f2c1e9a-3b4d-4c5e-9f60-718293a4b5cg"));
        assert!(!R::is_uuid("8f2c1e9a-3b4d-4c5e-9f60_718293a4b5c6"));
        for v in [
            "$DB_PASSWORD",
            "${GITHUB_TOKEN}",
            "%APP_DATA%",
            "$env:MY_KEY",
            "$A_1",
        ] {
            assert!(R::is_variable_reference(v), "{v}");
        }
        for v in [
            "$HUNTER2",
            "$A_",
            "${A_B",
            "$db_password",
            "%A_B",
            "$",
            "A_B",
            "$_A",
        ] {
            assert!(!R::is_variable_reference(v), "{v}");
        }
        for v in [
            "os.environ[\"API_KEY\"]",
            "os.getenv('API_KEY')",
            "env::var(\"GITHUB_TOKEN\")",
            "ENV[\"X1\"]",
        ] {
            assert!(R::is_env_lookup(v), "{v}");
        }
        for v in [
            "os.environ[\"api_key\"]",
            "os.getenv(\"API_KEY\") or \"x\"",
            "os.environ[\"API_KEY\")",
            "foo(\"API_KEY\")",
            "os.getenv(\"\")",
            "os.getenv(\"A\"B\")",
        ] {
            assert!(!R::is_env_lookup(v), "{v}");
        }
    }

    #[test]
    fn code_references_and_code_shapes() {
        for v in [
            "a.b",
            "a::b",
            "a?.b",
            "a->b",
            "f(x)",
            "a.b(c)",
            "a[b]",
            "a.b?",
            "&a.b",
            "&mut a.b",
            "*a.b",
            "await a.b",
            "fmt!(x)",
            "a.b(c, d).e",
        ] {
            assert!(R::is_code_reference(v), "{v}");
        }
        for v in [
            "a", "a?", "a.", ".a", "a.b c", "f(\"x\")", "f(a!b)", "f(a", "1a.b", "a..b", "a.b)",
            "a.1b",
        ] {
            assert!(!R::is_code_reference(v), "{v}");
        }
        for v in [
            "self.token",
            "config.api_key",
            "settings.SECRET_KEY",
            "get_token(u)",
            "getToken(u)",
            "os.getenv(K)",
            "a::b(c)",
            "x.API_KEY",
        ] {
            assert!(R::looks_like_code(v), "{v}");
        }
        for v in ["my.dog.rex", "abc(def)", "Secret(x)", "a.B", "my.dog.Rex"] {
            assert!(!R::looks_like_code(v), "{v}");
        }
    }

    #[test]
    fn kept_tails_are_exact() {
        for (line, expected) in [
            (
                "password = \"abc\"  # c\n",
                "password = \"[REDACTED:ENV_SECRET]\"  # c\n",
            ),
            ("password = 'abc'\n", "password = '[REDACTED:ENV_SECRET]'\n"),
            // A bare tail after a literal is part of the value to an INI parser (red-team);
            // a following assignment is not.
            (
                "password = \"abc\" + x\n",
                "password = \"[REDACTED:ENV_SECRET]\" [REDACTED:ENV_SECRET]\n",
            ),
            (
                "password = \"abc\" next=1\n",
                "password = \"[REDACTED:ENV_SECRET]\" next=1\n",
            ),
            (
                "password = \"abc\" + \"d\"\n",
                "password = \"[REDACTED:ENV_SECRET]\" [REDACTED:ENV_SECRET]\n",
            ),
            (
                // Code holding a literal takes the #29 rule: hidden from its start.
                "token = None or \"abc\"\n",
                "token = [REDACTED:ENV_SECRET]\n",
            ),
            ("token = None\n", "token = None\n"),
            (
                "token = None # \"c\"\n",
                "token = None # [REDACTED:ENV_SECRET]\n",
            ),
            (
                "password = \"[REDACTED:ENV_SECRET]\" x\n",
                "password = \"[REDACTED:ENV_SECRET]\" [REDACTED:ENV_SECRET]\n",
            ),
            (
                "password = \"[REDACTED:ENV_SECRET]\" [REDACTED:ENV_SECRET]\n",
                "password = \"[REDACTED:ENV_SECRET]\" [REDACTED:ENV_SECRET]\n",
            ),
            // Outside brackets a comma is part of a YAML scalar, so the #29 rule applies.
            (
                "token: \"abc\", next: 1\n",
                "token: [REDACTED:CONFIG_SECRET]\n",
            ),
            ("f(token=None, x=1)\n", "f(token=None, x=1)\n"),
            ("{token: null, x: 1}\n", "{token: null, x: 1}\n"),
        ] {
            assert_eq!(SecretRedactor::redact(line), expected, "{line:?}");
        }
    }

    #[test]
    fn env_code_context_needs_spaces_or_a_binding() {
        assert_eq!(
            SecretRedactor::redact("password = settings.SECRET_KEY\n"),
            "password = settings.SECRET_KEY\n"
        );
        assert_eq!(
            SecretRedactor::redact("let password=settings.SECRET_KEY;\n"),
            "let password=settings.SECRET_KEY;\n"
        );
        for line in [
            "password=settings.SECRET_KEY\n",
            "password =settings.SECRET_KEY\n",
            "password= settings.SECRET_KEY\n",
        ] {
            assert!(
                !SecretRedactor::redact(line).contains("settings"),
                "{line:?}"
            );
        }
    }

    #[test]
    fn json_fields_need_a_leading_member() {
        assert_eq!(SecretRedactor::redact_json_fields("   "), None);
        assert_eq!(SecretRedactor::redact_json_fields(""), None);
        assert_eq!(SecretRedactor::redact_json_fields("a: \"x\""), None);
        assert_eq!(
            SecretRedactor::redact_json_fields("  \"token\": \"x\", \"n\": 1,  "),
            Some("  \"token\": \"[REDACTED:CONFIG_SECRET]\", \"n\": 1,  ".to_owned())
        );
        assert_eq!(
            SecretRedactor::redact_json_fields("\"n\": 1"),
            Some("\"n\": 1".to_owned())
        );
    }

    #[test]
    fn a_cli_flag_without_a_value_is_left_alone() {
        for line in ["run --token\n", "run --token\tother\n", "run --to x\n"] {
            let clean = SecretRedactor::redact(line);
            assert!(
                !clean.contains("CLI_SECRET") || line.contains("--token\t"),
                "{line:?} -> {clean:?}"
            );
        }
        assert!(!SecretRedactor::redact("run --token abcdefgh\n").contains("abcdefgh"));
        assert!(!SecretRedactor::redact("run --token\tabcdefgh\n").contains("abcdefgh"));
    }
}
