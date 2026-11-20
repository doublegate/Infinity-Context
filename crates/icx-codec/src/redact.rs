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
        let mut pending_value = false;
        for segment in input.split_inclusive('\n') {
            let (body, ending) = split_line_ending(segment);
            if let Some(kind) = block {
                block_lines += 1;
                if kind.is_end(body) {
                    block = None;
                    continue;
                }
                if block_lines < MAX_KEY_BLOCK_LINES {
                    continue;
                }
                // Runaway/unterminated block: stop swallowing and resume normal processing.
                block = None;
            }
            if let Some(inline) = Self::redact_inline_key_blocks(body) {
                out.push_str(&Self::redact_line(&inline));
                out.push_str(ending);
                continue;
            }
            if let Some(kind) = KeyBlock::begins(body) {
                block = Some(kind);
                block_lines = 0;
                out.push_str(kind.marker());
                out.push_str(ending);
                continue;
            }
            let redacted = if pending_value && !body.trim().is_empty() {
                match ContinuationLine::classify(body) {
                    // A comment between the key and its value keeps the value pending.
                    ContinuationLine::Comment => Self::redact_line(body),
                    // A nested mapping or structure: the key had no scalar value to hide.
                    ContinuationLine::Structure => {
                        pending_value = false;
                        Self::redact_line(body)
                    }
                    ContinuationLine::Value => {
                        pending_value = false;
                        Self::redact_bare_value_line(body)
                    }
                }
            } else {
                Self::redact_line(body)
            };
            if Self::ends_with_sensitive_key(body) {
                pending_value = true;
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
    fn ends_with_sensitive_key(line: &str) -> bool {
        let Some(head) = line.trim_end().strip_suffix(':') else {
            return false;
        };
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
        prefix_ok && !key.is_empty() && Self::is_sensitive_key_name(key)
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
        sensitive_suffixes
            .iter()
            .any(|s| key_upper == *s || key_upper.ends_with(&format!("_{s}")))
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
                return Some((inner_start + replacement.len() + 2).min(out.len()));
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
                return Some((inner_start + replacement.len() + 1).min(out.len()));
            }
            return None;
        }
        if matches!(quote, b'{' | b'[') {
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
        let mut out = line.to_owned();
        let mut search_pos = 0usize;
        while search_pos < out.len() {
            let Some(rel_idx) = out[search_pos..].find("://") else {
                break;
            };
            let sep = search_pos + rel_idx;
            let scheme_ok = out[..sep]
                .chars()
                .rev()
                .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
                .last()
                .is_some_and(|c| c.is_ascii_alphabetic());
            let after_scheme = sep + 3;
            if !scheme_ok {
                search_pos = after_scheme;
                continue;
            }
            let rest = &out[after_scheme..];
            let end_authority = rest
                .find(|c: char| {
                    matches!(
                        c,
                        '/' | '?' | '#' | '"' | '\'' | '\\' | ')' | ']' | '}' | '>'
                    ) || c.is_whitespace()
                })
                .unwrap_or(rest.len());
            let authority = &rest[..end_authority];
            if let Some(at_pos) = authority.rfind('@') {
                let userinfo = &authority[..at_pos];
                if let Some(colon_pos) = userinfo.find(':') {
                    let pass_start = after_scheme + colon_pos + 1;
                    let pass_end = after_scheme + at_pos;
                    if pass_end > pass_start && !out[pass_start..pass_end].starts_with("[REDACTED:")
                    {
                        let replacement = "[REDACTED:URL_PASSWORD]";
                        out.replace_range(pass_start..pass_end, replacement);
                        search_pos = pass_start + replacement.len();
                        continue;
                    }
                } else if Self::looks_like_url_token(userinfo) {
                    // `https://<token>@host` (no `user:` part): GitHub/GitLab-style token auth.
                    let replacement = "[REDACTED:URL_TOKEN]";
                    out.replace_range(after_scheme..after_scheme + at_pos, replacement);
                    search_pos = after_scheme + replacement.len();
                    continue;
                }
            }
            search_pos = after_scheme + end_authority;
        }
        out
    }

    /// A colon-less URL userinfo that is a credential rather than a login name: at least 16
    /// characters of token alphabet mixing letters and digits (`git`, `alice`, `deploy-bot` are
    /// kept).
    fn looks_like_url_token(userinfo: &str) -> bool {
        userinfo.len() >= 16
            && !userinfo.starts_with("[REDACTED:")
            && userinfo
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '~' | '%'))
            && userinfo.chars().any(|c| c.is_ascii_digit())
            && userinfo.chars().any(|c| c.is_ascii_alphabetic())
    }

    /// Redacts vendor formats the generic prefix scan cannot express: dotted SendGrid keys,
    /// Slack webhook URLs, and Twilio `SK` + 32 hex API keys (v1.1.0, ICX-AUD-015).
    fn redact_structured_vendor_tokens(line: &str) -> String {
        let mut out = line.to_owned();
        // SendGrid: SG.<22 urlsafe>.<43 urlsafe>
        let mut pos = 0usize;
        while let Some(rel) = out[pos..].find("SG.") {
            let start = pos + rel;
            let tok_len = out[start..]
                .bytes()
                .take_while(|&b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
                .count();
            let tok = &out[start..start + tok_len];
            if tok_len >= 60 && tok.matches('.').count() == 2 {
                out.replace_range(start..start + tok_len, "[REDACTED:SENDGRID_KEY]");
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
            }
            pos = path_start;
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
            if s1.len() >= 8
                && s2.starts_with("eyJ")
                && s2.len() >= 8
                && s3.len() >= 6
                && segs.next().is_none()
            {
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
            let is_assignment = full_line.contains('=') || full_line.contains(':');
            return token.len() >= 32 && is_assignment && Self::shannon_entropy(token) > 3.50;
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
    /// A nested `key: value` / `key = value` entry, `key:` header, or `{`/`[` opener.
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
        let key_len = entry
            .find(|c: char| {
                !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '"' | '\''))
            })
            .unwrap_or(entry.len());
        let key = entry[..key_len].trim_matches(|c: char| c == '"' || c == '\'');
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
        // `key = value` needs a value after a single `=`: `secret==` / `abc=` is base64 padding.
        let eq_entry = rest
            .strip_prefix('=')
            .is_some_and(|r| !r.starts_with('=') && !r.trim().is_empty());
        let is_entry = key_ok && (colon_entry || eq_entry);
        if is_entry {
            Self::Structure
        } else {
            Self::Value
        }
    }
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

    fn begins(line: &str) -> Option<Self> {
        if line.contains("-----BEGIN PGP ") && line.contains("PRIVATE KEY BLOCK-----") {
            Some(Self::Pgp)
        } else if line.contains("-----BEGIN ") && line.contains("PRIVATE KEY-----") {
            Some(Self::Pem)
        } else {
            None
        }
    }

    fn is_end(self, line: &str) -> bool {
        match self {
            Self::Pem => line.contains("-----END ") && line.contains("PRIVATE KEY-----"),
            Self::Pgp => line.contains("-----END PGP ") && line.contains("PRIVATE KEY BLOCK-----"),
        }
    }

    fn begin_index(self, line: &str) -> Option<usize> {
        let (open, tail) = match self {
            Self::Pem => ("-----BEGIN ", "PRIVATE KEY-----"),
            Self::Pgp => ("-----BEGIN PGP ", "PRIVATE KEY BLOCK-----"),
        };
        let start = line.find(open)?;
        line[start..].contains(tail).then_some(start)
    }

    fn end_index_after(self, line: &str, start: usize) -> Option<usize> {
        let (close, tail) = match self {
            Self::Pem => ("-----END ", "PRIVATE KEY-----"),
            Self::Pgp => ("-----END PGP ", "PRIVATE KEY BLOCK-----"),
        };
        let rel = line[start..].find(close)?;
        let after_close = start + rel;
        let tail_rel = line[after_close..].find(tail)?;
        Some(after_close + tail_rel + tail.len())
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
