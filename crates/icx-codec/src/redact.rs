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
            if let Some(pending) = pending_value.as_mut() {
                let indent = body.len() - body.trim_start().len();
                if body.trim().is_empty() {
                    out.push_str(body);
                    out.push_str(ending);
                    continue;
                }
                if let Some(q) = pending.quote {
                    // Inside an open quoted scalar every line is secret up to the closing quote.
                    let content_start = body.len() - body.trim_start().len();
                    let rest = &body[content_start..];
                    if let Some(close) = closing_quote(rest.as_bytes(), q) {
                        out.push_str(&body[..content_start]);
                        out.push_str("[REDACTED:SECRET]");
                        out.push_str(&Self::redact_line(&rest[close..]));
                        pending_value = None;
                    } else {
                        out.push_str(&Self::redact_bare_value_line(body));
                    }
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
        let (head, suffix) = declaration.rsplit_once(':')?;
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

    /// The line without a trailing ` # comment`, right-trimmed.
    fn declaration(line: &str) -> &str {
        let comment = line.match_indices('#').find_map(|(i, _)| {
            (i > 0 && line.as_bytes()[i - 1].is_ascii_whitespace()).then_some(i)
        });
        comment.map_or(line, |i| &line[..i]).trim_end()
    }

    /// Byte index in `line` of the `:` that introduces its last value or block header.
    fn declaration_colon(line: &str) -> Option<usize> {
        Self::declaration(line).rfind(':')
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

    fn redact_value_span(
        out: &mut String,
        val_start: usize,
        replacement: &str,
        stop_at_comma: bool,
    ) -> Option<usize> {
        let bytes = out.as_bytes();
        if val_start >= bytes.len() {
            return None;
        }
        let quote = bytes[val_start];
        let escaped_quote = if quote == b'\\'
            && val_start + 1 < bytes.len()
            && (bytes[val_start + 1] == b'"' || bytes[val_start + 1] == b'\'')
        {
            Some(bytes[val_start + 1])
        } else {
            None
        };
        if let Some(eq_q) = escaped_quote {
            let inner_start = val_start + 2;
            let mut inner_end = inner_start;
            while inner_end < bytes.len() {
                if bytes[inner_end] == b'\\'
                    && inner_end + 1 < bytes.len()
                    && bytes[inner_end + 1] == eq_q
                {
                    break;
                }
                if bytes[inner_end] == b'"' || bytes[inner_end] == b'\'' {
                    break;
                }
                inner_end += 1;
            }
            if inner_end > inner_start && !out[inner_start..inner_end].starts_with("[REDACTED:") {
                out.replace_range(inner_start..inner_end, replacement);
                // A malformed escaped opening can terminate at a bare one-byte quote.
                // Advancing by an assumed two-byte closer could enter the following UTF-8
                // codepoint. Resume at the known replacement boundary instead.
                return Some(inner_start + replacement.len());
            }
            return None;
        }
        if quote == b'"' || quote == b'\'' {
            let inner_start = val_start + 1;
            let mut inner_end = inner_start;
            while inner_end < bytes.len() && bytes[inner_end] != quote {
                if bytes[inner_end] == b'\\'
                    && inner_end + 1 < bytes.len()
                    && bytes[inner_end + 1].is_ascii()
                {
                    inner_end += 2;
                } else {
                    inner_end += 1;
                }
            }
            if inner_end > inner_start && !out[inner_start..inner_end].starts_with("[REDACTED:") {
                out.replace_range(inner_start..inner_end, replacement);
                return Some(inner_start + replacement.len());
            }
            return None;
        }
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
            if ch.is_whitespace()
                || matches!(ch, '"' | '\'' | ';' | '&' | '|' | ')' | ']' | '}')
                || (stop_at_comma && ch == ',')
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
            if !key_name.is_empty() && Self::is_sensitive_key_name(key_name) {
                let mut val_start = eq_idx + 1;
                while val_start < bytes.len()
                    && (bytes[val_start] == b' ' || bytes[val_start] == b'\t')
                {
                    val_start += 1;
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
        if Self::sensitive_continuation(line).is_some_and(|pending| pending.block) {
            let Some(colon) = Self::declaration_colon(line) else {
                return line.to_owned();
            };
            let (head, header) = line.split_at(colon);
            let mut out = Self::redact_colon_assignments(head);
            out.push_str(header);
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
        let csv_cell = lead.ends_with(',') && (tail.is_empty() || tail.starts_with(','));
        labelled || sha256sum_line || commit_line || table_cell || csv_cell
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
        let multi = r#"{"tool":"Bash","api_key":"json_secret_123","password":"json_pw_456","nested":{"token":"nested_tok_789"},"prose":"Apply basic configuration and nonbasic settings"}
FOO=1 AWS_SECRET_ACCESS_KEY="quoted_aws_secret" BAR=2
urls: postgres://localhost:5432/dev postgres://admin:p@ssw0rd!@prod.db:5432/main"#;
        let clean_multi = SecretRedactor::sanitize(multi);
        assert!(!clean_multi.contains("json_secret_123"));
        assert!(!clean_multi.contains("json_pw_456"));
        assert!(!clean_multi.contains("nested_tok_789"));
        assert!(clean_multi.contains("Apply basic configuration and nonbasic settings"));
        assert!(!clean_multi.contains("quoted_aws_secret"));
        assert!(
            clean_multi.contains("FOO=1 AWS_SECRET_ACCESS_KEY=\"[REDACTED:ENV_SECRET]\" BAR=2")
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
        assert!(clean_edges.contains("--api-key=\"[REDACTED:CLI_SECRET]\""));
        assert!(clean_edges.contains("--private-key='[REDACTED:CLI_SECRET]'"));
        assert!(clean_edges.contains("--refresh-token [REDACTED:CLI_SECRET]"));
        assert!(clean_edges.contains("--secret=\\\"[REDACTED:CLI_SECRET]\\\""));
        assert!(clean_edges.contains("postgres://api_key:[REDACTED:URL_PASSWORD]@host:5432/db"));
        assert!(clean_edges.contains("redis://token:[REDACTED:URL_PASSWORD]@host:6379"));
        assert!(clean_edges.contains(&format!("root={high_entropy_hex}")));
        assert!(clean_edges.contains("\"stdout\":\"\\n[REDACTED:HIGH_ENTROPY]\""));
    }
}
