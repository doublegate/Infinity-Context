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
                let Some(close) = closing_quote(&body.as_bytes()[content_start..], q) else {
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
                && let Some(json) = Self::redact_json_line(body)
            {
                out.push_str(&json);
                out.push_str(ending);
                continue;
            }
            // Private-key normalization must also pass through pending scalar processing.
            // Otherwise a key banner after a scalar prefix hides that prefix only on the
            // second redaction pass. Keep block state so subsequent key lines stay hidden.
            let normalized = if let Some(inline) = Self::redact_inline_key_blocks(body) {
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
            if let Some(pending) = Self::sensitive_continuation(body) {
                pending_value = Some(pending);
            }
            out.push_str(&redacted);
            out.push_str(ending);
        }
        out
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
        if !suffix.is_empty() && !block && quote.is_none() {
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

    fn is_sensitive_key_name(raw_key: &str) -> bool {
        let trimmed = raw_key
            .trim()
            .trim_start_matches("export ")
            .trim_start_matches('-')
            .trim_matches(|c: char| c == '"' || c == '\'');
        let key_upper = trimmed.to_ascii_uppercase().replace('-', "_");
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
        matches(&key_upper)
            || key_upper
                .strip_suffix('S')
                .is_some_and(|singular| !singular.ends_with("PASS") && matches(singular))
    }

    /// Upper-case `_`/`-`-separated parts of a key (`--max-tokens` -> `MAX`, `TOKENS`).
    fn key_parts(key: &str) -> Vec<String> {
        key.to_ascii_uppercase()
            .split(['_', '-'])
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

    fn redact_line(line: &str) -> String {
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
                // `disk-by...` matched in the field; v1.3.2): only when 3+ letters (no digits)
                // end in it, such as `autode|sk-`. Every distinctive prefix (`ghp_`, `AKIA`,
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
    ) -> Option<usize> {
        // A value with any quote is hidden from its first quote to the end of the line (v1.3.1,
        // #29, owner decision on PR #31).
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
            // An earlier pass already replaced this value.
            if out[val_start..].starts_with("[REDACTED:") {
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
            if !inner.trim().is_empty() && !inner.starts_with("[REDACTED:") {
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
        for (idx, ch) in rest.char_indices() {
            // A backtick or an existing marker ends the token too, so a marker the literal pass
            // wrote is never cut in half (idempotence, v1.3.1).
            if ch.is_whitespace()
                || matches!(ch, '"' | '\'' | '`' | ';' | '&' | '|' | ')' | ']' | '}')
                || (stop_at_comma && ch == ',')
                || (idx > 0 && rest[idx..].starts_with("[REDACTED:"))
            {
                val_len = idx;
                break;
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
        if val_len > 0 && !out[val_start..val_start + val_len].starts_with("[REDACTED:") {
            out.replace_range(val_start..val_start + val_len, replacement);
            return Some(val_start + replacement.len());
        }
        None
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
                if let Some(next_pos) =
                    Self::redact_value_span(&mut out, val_start, replacement, false)
                {
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
            if !key_name.is_empty() && Self::is_sensitive_key_name(key_name) {
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
                        )
                    {
                        search_pos = next_pos;
                        continue;
                    }
                    search_pos = after.max(colon_idx + 1);
                    continue;
                }
                if let Some(next_pos) =
                    Self::redact_value_span(&mut out, val_start, "[REDACTED:CONFIG_SECRET]", true)
                {
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
                    if let Some(next_pos) =
                        Self::redact_value_span(&mut out, val_start, "[REDACTED:CLI_SECRET]", false)
                    {
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

    /// Whether `before` ends in a run of 3+ ASCII letters that starts at a word boundary, with
    /// no digit in the run (`autode` in `autodesk-`, not `n` in a literal `\nAKIA`).
    fn ends_an_ordinary_word(before: &str) -> bool {
        let run = before
            .bytes()
            .rev()
            .take_while(u8::is_ascii_alphanumeric)
            .collect::<Vec<u8>>();
        run.len() >= 3 && run.iter().all(u8::is_ascii_alphabetic)
    }

    /// At least three `-`-separated parts, at least two (and a third) of them natural words.
    fn is_word_slug(token: &str) -> bool {
        let parts: Vec<&str> = token.split('-').filter(|p| !p.is_empty()).collect();
        let words = parts
            .iter()
            .filter(|p| {
                p.len() >= 3
                    && p.bytes().all(|b| b.is_ascii_alphabetic())
                    && p.bytes().any(|b| !b.is_ascii_hexdigit())
            })
            .count();
        parts.len() >= 3 && words >= 2 && words * 3 >= parts.len()
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

#[derive(Clone, Copy, Debug)]
struct PendingSecret {
    indent: usize,
    block: bool,
    scalar_indent: Option<usize>,
    /// Quote character of a quoted scalar that is still open at the end of the key's line.
    quote: Option<u8>,
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
            if sensitive && !check {
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
        // Since v1.3.1 (#29) the rest of the line after the quoted value is hidden with it.
        assert!(clean_multi.contains("FOO=1 AWS_SECRET_ACCESS_KEY=[REDACTED:ENV_SECRET]\n"));
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
        assert!(clean_edges.contains("curl --api-key=[REDACTED:CLI_SECRET]\n"));
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
}
