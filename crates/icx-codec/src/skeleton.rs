//! Single-pass domain-specific Structural Skeletonizers (`StructuralSkeletonizer`) for
//! multi-language source code (Rust, Python, TypeScript/JS, Go, C/C++), unified `git diff`s,
//! and multi-ecosystem compiler/test logs (`cargo`, `pytest`, `tsc`/`vitest`, `go test`).
//!
//! Reference: `docs/ahtl/05-structural-ast-skeletonization-and-redaction.md`.

use crate::ahtl::{AhtlClause, AhtlPacket, ClauseOp, PacketKind};
use crate::redact::SecretRedactor;
use icx_core::ArtifactId;
use std::collections::BTreeSet;

/// Compresses raw source code, diffs, and build/test logs into structured `AhtlPacket` frames.
#[derive(Clone, Debug, Default)]
pub struct StructuralSkeletonizer;

impl StructuralSkeletonizer {
    /// Automatically detects payload domain (test/build log, unified diff, or source/text),
    /// sanitizes untrusted input via `SecretRedactor`, stamps `FLAG_IS_REDACTED` on `hologram_bytes[7]`
    /// if content was modified, and compiles a high-density `AhtlPacket`.
    pub fn compress_to_packet(
        subject: &str,
        raw_text: &str,
        deref_id: Option<ArtifactId>,
        mut hologram_bytes: [u8; 8],
    ) -> AhtlPacket {
        let clean = SecretRedactor::sanitize(raw_text);
        if raw_text.lines().ne(clean.lines()) {
            hologram_bytes[7] |= crate::braille::FLAG_IS_REDACTED;
        }
        Self::compress_sanitized_to_packet(subject, &clean, deref_id, hologram_bytes)
    }

    /// Compiles an `AhtlPacket` from a payload that has already been sanitized by `SecretRedactor::sanitize`.
    pub fn compress_sanitized_to_packet(
        subject: &str,
        clean: &str,
        deref_id: Option<ArtifactId>,
        hologram_bytes: [u8; 8],
    ) -> AhtlPacket {
        let total_lines = clean.lines().count().max(1) as u32;

        if Self::looks_like_test_or_build_log(clean) {
            Self::skeletonize_log(subject, clean, total_lines, deref_id, hologram_bytes)
        } else if Self::looks_like_diff(clean) {
            Self::skeletonize_diff(subject, clean, total_lines, deref_id, hologram_bytes)
        } else {
            Self::skeletonize_source_or_text(subject, clean, total_lines, deref_id, hologram_bytes)
        }
    }

    /// Line-anchored detection of compiler and test-runner logs so that source files containing
    /// diagnostic string literals (and test logs containing assertion diffs) are classified
    /// accurately.
    fn looks_like_test_or_build_log(text: &str) -> bool {
        text.lines().any(|line| {
            let t = line.trim();
            (t.starts_with("running ") && t.ends_with(" tests"))
                || t.starts_with("test result:")
                || t.starts_with("Compiling ")
                || t.starts_with("error[E")
                || (t.starts_with("thread '") && t.contains("panicked at"))
                || (t.starts_with("==") && t.contains("test session starts"))
                || (t.starts_with("==") && t.contains("FAILURES"))
                || t == "Traceback (most recent call last):"
                || t.starts_with("Test Files ")
                || t.starts_with("=== RUN ")
                || t.starts_with("--- PASS:")
                || t.starts_with("--- FAIL:")
                || t.contains(": error TS")
                || t.contains("): error TS")
        })
    }

    /// Line-anchored detection of unified `git diff` payloads.
    fn looks_like_diff(text: &str) -> bool {
        text.lines()
            .any(|line| line.starts_with("diff --git ") || line.starts_with("@@ -"))
    }

    fn skeletonize_log(
        subject: &str,
        text: &str,
        total_lines: u32,
        deref_id: Option<ArtifactId>,
        hologram_bytes: [u8; 8],
    ) -> AhtlPacket {
        let mut clauses = Vec::new();
        let mut pass_count = 0usize;
        let mut fail_count = 0usize;
        let mut diag_clauses = 0usize;
        let mut seen_failed_tests: BTreeSet<String> = BTreeSet::new();

        for (idx, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let line_no = idx + 1;

            // 1. Passing test counters across cargo test, pytest, vitest/jest, and go test
            if (trimmed.starts_with("test ") && trimmed.ends_with(" ... ok"))
                || (trimmed.contains("::")
                    && (trimmed.ends_with(" PASSED") || trimmed.contains(" PASSED [")))
                || trimmed.starts_with("PASSED ")
                || trimmed.starts_with("✓ ")
                || trimmed.starts_with("PASS ")
                || trimmed.starts_with("--- PASS:")
            {
                pass_count += 1;
                continue;
            }

            // 2. Failing test lines across cargo test, pytest, vitest/jest, and go test
            let extracted_fail = if trimmed.starts_with("test ") && trimmed.ends_with(" ... FAILED")
            {
                Some(
                    trimmed
                        .strip_prefix("test ")
                        .and_then(|s| s.strip_suffix(" ... FAILED"))
                        .unwrap_or(trimmed),
                )
            } else if let Some(rest) = trimmed.strip_prefix("FAILED ") {
                Some(rest.split(" - ").next().unwrap_or(rest))
            } else if trimmed.contains("::")
                && (trimmed.ends_with(" FAILED") || trimmed.contains(" FAILED ["))
            {
                Some(trimmed.split_whitespace().next().unwrap_or(trimmed))
            } else if let Some(rest) = trimmed.strip_prefix("--- FAIL:") {
                Some(rest.split_whitespace().next().unwrap_or(rest.trim()))
            } else {
                trimmed
                    .strip_prefix("FAIL ")
                    .or_else(|| trimmed.strip_prefix("❯ "))
                    .or_else(|| trimmed.strip_prefix("✕ "))
                    .map(|rest| rest.split(" (").next().unwrap_or(rest).trim())
            };

            if let Some(raw_test_name) = extracted_fail {
                let norm_name = Self::compact_token(raw_test_name);
                if !norm_name.is_empty() && seen_failed_tests.insert(norm_name.clone()) {
                    fail_count += 1;
                    clauses.push(AhtlClause::new(
                        ClauseOp::Anomaly,
                        format!("FAIL@L{line_no}"),
                        Some(norm_name),
                    ));
                }
            } else if (trimmed.contains("panicked at")
                || trimmed.starts_with("error[E")
                || trimmed.starts_with("error:")
                || trimmed.contains("error TS")
                || trimmed.starts_with("E   ")
                || trimmed.starts_with("AssertionError")
                || (trimmed.starts_with("File \"") && trimmed.contains(", line "))
                || (trimmed.contains(".go:")
                    && (trimmed.contains("expected")
                        || trimmed.contains("error")
                        || trimmed.contains("got ")
                        || trimmed.contains("panic"))))
                && diag_clauses < 16
            {
                diag_clauses += 1;
                clauses.push(AhtlClause::new(
                    ClauseOp::Anomaly,
                    format!("ERR@L{line_no}"),
                    Some(Self::compact_token(trimmed)),
                ));
            }
        }

        if pass_count > 0 {
            clauses.insert(
                0,
                AhtlClause::new(ClauseOp::Verified, "pass", Some(pass_count.to_string())),
            );
        }
        if fail_count > 0 {
            clauses.insert(
                0,
                AhtlClause::new(ClauseOp::Anomaly, "fail", Some(fail_count.to_string())),
            );
        }
        clauses.push(AhtlClause::new(
            ClauseOp::Neutral,
            "bytes",
            Some(text.len().to_string()),
        ));

        AhtlPacket::new(
            PacketKind::ToolStub,
            Self::compact_token(subject),
            Some((1, total_lines)),
            clauses,
            deref_id,
            hologram_bytes,
        )
    }

    fn skeletonize_diff(
        subject: &str,
        text: &str,
        total_lines: u32,
        deref_id: Option<ArtifactId>,
        hologram_bytes: [u8; 8],
    ) -> AhtlPacket {
        let lines: Vec<&str> = text.lines().collect();
        let mut clauses = Vec::new();
        let mut added = 0usize;
        let mut removed = 0usize;
        let mut seen_files: BTreeSet<String> = BTreeSet::new();
        let mut hunk_count = 0usize;

        for (idx, &line) in lines.iter().enumerate() {
            if line.starts_with("diff --git ") {
                continue;
            }
            // Match unified diff file header pairs (`--- <old>` immediately followed by `+++ <new>`)
            // so removed lines starting with `-- ` inside a hunk are never mistaken for headers.
            if let Some(after_minus) = line.strip_prefix("--- ") {
                if lines
                    .get(idx + 1)
                    .is_some_and(|next| next.starts_with("+++ "))
                {
                    let raw_path = after_minus.trim();
                    if raw_path != "/dev/null" {
                        let clean_path = raw_path.strip_prefix("a/").unwrap_or(raw_path);
                        let tok = Self::compact_token(clean_path);
                        if seen_files.len() < 4 && seen_files.insert(tok.clone()) {
                            clauses.push(AhtlClause::new(ClauseOp::Neutral, "file", Some(tok)));
                        }
                    }
                    continue;
                }
            }
            if let Some(after_plus) = line.strip_prefix("+++ ") {
                if idx > 0 && lines[idx - 1].starts_with("--- ") {
                    let raw_path = after_plus.trim();
                    if raw_path != "/dev/null" {
                        let clean_path = raw_path.strip_prefix("b/").unwrap_or(raw_path);
                        let tok = Self::compact_token(clean_path);
                        if seen_files.len() < 4 && seen_files.insert(tok.clone()) {
                            clauses.push(AhtlClause::new(ClauseOp::Neutral, "file", Some(tok)));
                        }
                    }
                    continue;
                }
            }
            if let Some(after_at) = line.strip_prefix("@@ ") {
                if let Some((coords, _)) = after_at.split_once(" @@") {
                    if hunk_count < 4 {
                        hunk_count += 1;
                        clauses.push(AhtlClause::new(
                            ClauseOp::Neutral,
                            "hunk",
                            Some(Self::compact_token(coords)),
                        ));
                    }
                }
                continue;
            }
            if let Some(rest) = line.strip_prefix('+') {
                added += 1;
                let trimmed = rest.trim();
                if let Some(sym) = Self::extract_symbol_signature(trimmed) {
                    if clauses.len() < 16 {
                        clauses.push(AhtlClause::new(ClauseOp::Added, "sym", Some(sym)));
                    }
                } else if (trimmed.contains("INVARIANT")
                    || trimmed.contains("SAFETY:")
                    || trimmed.contains("PASSKEY"))
                    && clauses.len() < 16
                {
                    clauses.push(AhtlClause::new(
                        ClauseOp::Verified,
                        "pin",
                        Some(Self::compact_token(trimmed)),
                    ));
                }
            } else if let Some(rest) = line.strip_prefix('-') {
                removed += 1;
                if let Some(sym) = Self::extract_symbol_signature(rest.trim()) {
                    if clauses.len() < 16 {
                        clauses.push(AhtlClause::new(ClauseOp::Removed, "sym", Some(sym)));
                    }
                }
            }
        }

        clauses.insert(
            0,
            AhtlClause::new(
                ClauseOp::Neutral,
                "hunks",
                Some(format!("+{added}/-{removed}")),
            ),
        );

        AhtlPacket::new(
            PacketKind::CodeDelta,
            Self::compact_token(subject),
            Some((1, total_lines)),
            clauses,
            deref_id,
            hologram_bytes,
        )
    }

    fn skeletonize_source_or_text(
        subject: &str,
        text: &str,
        total_lines: u32,
        deref_id: Option<ArtifactId>,
        hologram_bytes: [u8; 8],
    ) -> AhtlPacket {
        let lines: Vec<&str> = text.lines().collect();
        let mut clauses = Vec::new();
        let mut import_first_line: Option<usize> = None;
        let mut import_last_line = 0usize;
        let mut import_count = 0usize;
        let mut still_in_header_imports = true;
        let mut in_go_import_block = false;

        for (idx, &line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            let line_no = idx + 1;

            if in_go_import_block {
                if trimmed.starts_with(')') {
                    in_go_import_block = false;
                    import_last_line = line_no;
                } else if !trimmed.is_empty() && !trimmed.starts_with("//") {
                    import_count += 1;
                    import_last_line = line_no;
                }
                continue;
            }

            if trimmed == "import (" && still_in_header_imports {
                in_go_import_block = true;
                if import_first_line.is_none() {
                    import_first_line = Some(line_no);
                }
                import_last_line = line_no;
                continue;
            }

            if Self::is_import_line(trimmed) {
                if still_in_header_imports {
                    if import_first_line.is_none() {
                        import_first_line = Some(line_no);
                    }
                    import_last_line = line_no;
                    import_count += 1;
                }
                continue;
            }

            if let Some(mod_name) = Self::extract_module_header(trimmed) {
                if clauses.len() < 18 {
                    clauses.push(AhtlClause::new(
                        ClauseOp::Added,
                        format!("mod@L{line_no}"),
                        Some(mod_name),
                    ));
                }
                continue;
            }

            if let Some(sig) = Self::extract_symbol_signature(trimmed) {
                still_in_header_imports = false;
                if clauses.len() < 18 {
                    let end_line = Self::resolve_block_end_line(&lines, idx);
                    let span_key = if end_line > line_no {
                        format!("L{line_no}-{end_line}")
                    } else {
                        format!("L{line_no}")
                    };
                    clauses.push(AhtlClause::new(ClauseOp::Added, span_key, Some(sig)));
                }
            } else if (trimmed.contains("PASSKEY")
                || trimmed.contains("INVARIANT")
                || trimmed.contains("SAFETY:")
                || trimmed.contains("TODO:")
                || trimmed.contains("FIXME:"))
                && clauses.len() < 20
            {
                clauses.push(AhtlClause::new(
                    ClauseOp::Verified,
                    format!("pin@L{line_no}"),
                    Some(Self::compact_token(trimmed)),
                ));
            }
        }

        if let Some(first_imp) = import_first_line {
            clauses.insert(
                0,
                AhtlClause::new(
                    ClauseOp::Neutral,
                    "imports",
                    Some(format!("L{first_imp}-{import_last_line}({import_count})")),
                ),
            );
        }

        if clauses.is_empty() {
            let head = lines.first().copied().unwrap_or("");
            clauses.push(AhtlClause::new(
                ClauseOp::Neutral,
                "head",
                Some(Self::compact_token(head)),
            ));
        }

        clauses.push(AhtlClause::new(
            ClauseOp::Neutral,
            "raw_B",
            Some(text.len().to_string()),
        ));

        AhtlPacket::new(
            PacketKind::CodeDelta,
            Self::compact_token(subject),
            Some((1, total_lines)),
            clauses,
            deref_id,
            hologram_bytes,
        )
    }

    fn is_import_line(trimmed: &str) -> bool {
        trimmed.starts_with("use ")
            || trimmed.starts_with("pub use ")
            || trimmed.starts_with("import ")
            || (trimmed.starts_with("from ") && trimmed.contains(" import "))
            || trimmed.starts_with("#include ")
    }

    fn extract_module_header(trimmed: &str) -> Option<String> {
        let prefixes = [
            "pub mod ",
            "pub(crate) mod ",
            "mod ",
            "package ",
            "namespace ",
        ];
        for p in prefixes {
            if let Some(rest) = trimmed.strip_prefix(p) {
                let end = rest
                    .find(|c: char| c == ';' || c == '{' || c.is_whitespace())
                    .unwrap_or(rest.len());
                let name = &rest[..end];
                if !name.is_empty() {
                    return Some(Self::compact_token(name));
                }
            }
        }
        None
    }

    fn extract_symbol_signature(trimmed: &str) -> Option<String> {
        let prefixes = [
            "pub(crate) async fn ",
            "pub(crate) const fn ",
            "pub(crate) fn ",
            "pub async fn ",
            "pub const fn ",
            "pub fn ",
            "async fn ",
            "const fn ",
            "fn ",
            "pub(crate) struct ",
            "pub struct ",
            "struct ",
            "pub(crate) enum ",
            "pub enum ",
            "enum class ",
            "enum ",
            "pub trait ",
            "trait ",
            "impl<",
            "impl ",
            "pub type ",
            "type ",
            "macro_rules! ",
            "async def ",
            "def ",
            "export default async function ",
            "export default function ",
            "export async function ",
            "export function ",
            "async function ",
            "function ",
            "export abstract class ",
            "export class ",
            "abstract class ",
            "class ",
            "export interface ",
            "interface ",
            "export type ",
            "export enum ",
            "func ",
            "template<",
            "template <",
            "typedef struct ",
        ];
        for p in prefixes {
            if let Some(rest) = trimmed.strip_prefix(p) {
                let raw_end = rest.find('{').unwrap_or(rest.len()).min(56);
                let sig_end = rest.floor_char_boundary(raw_end);
                let clean = Self::compact_token(&rest[..sig_end]);
                if !clean.is_empty() {
                    return Some(clean);
                }
            }
        }
        None
    }

    /// Scans a single source line for unquoted, uncommented `{` and `}` braces, returning
    /// `(net_brace_delta, saw_opening_brace)`. Distinguishes single-character literals (`'}'`,
    /// `'\''`) from Rust lifetime/loop annotations (`&'static str`, `View<'a>`, `'outer: loop`).
    fn scan_code_braces(line: &str) -> (i32, bool) {
        let mut delta = 0i32;
        let mut saw_open = false;
        let mut in_double = false;
        let mut chars = line.chars().peekable();

        while let Some(ch) = chars.next() {
            if in_double {
                if ch == '\\' {
                    let _ = chars.next();
                } else if ch == '"' {
                    in_double = false;
                }
                continue;
            }
            if ch == '/' && chars.peek() == Some(&'/') {
                break;
            }
            if ch == '"' {
                in_double = true;
            } else if ch == '\'' {
                // Consume `'c'` or `'\x'` character literals while leaving Rust lifetimes untouched
                let mut lookahead = chars.clone();
                if let Some(first_after) = lookahead.next() {
                    if first_after == '\\' {
                        let _ = lookahead.next();
                        if lookahead.next() == Some('\'') {
                            let _ = chars.next();
                            let _ = chars.next();
                            let _ = chars.next();
                        }
                    } else if lookahead.next() == Some('\'') {
                        let _ = chars.next();
                        let _ = chars.next();
                    }
                }
            } else if ch == '{' {
                delta += 1;
                saw_open = true;
            } else if ch == '}' {
                delta -= 1;
            }
        }
        (delta, saw_open)
    }

    /// Resolves the 1-based ending line number of a function, struct, class, or impl block starting
    /// at 0-based `start_idx` using indentation tracking (Python) or quote/lifetime/comment-aware
    /// brace depth tracking (Rust, TypeScript/JS, Go, C/C++).
    fn resolve_block_end_line(lines: &[&str], start_idx: usize) -> usize {
        let first_line = lines[start_idx];
        let trimmed = first_line.trim();

        // Python indentation-delimited block (`def ...` / `async def ...` / `class ...` without `{`)
        let is_python_decl = trimmed.starts_with("def ")
            || trimmed.starts_with("async def ")
            || (trimmed.starts_with("class ")
                && !trimmed.contains('{')
                && (trimmed.ends_with(':') || trimmed.contains('(')));
        if is_python_decl {
            let base_indent = first_line.len() - first_line.trim_start().len();
            let mut body_start_idx = start_idx + 1;
            if !trimmed.ends_with(':') {
                for (offset, &line) in lines[start_idx + 1..].iter().enumerate().take(10) {
                    if line.trim().ends_with(':') {
                        body_start_idx = start_idx + 1 + offset + 1;
                        break;
                    }
                }
            }
            let mut last_body_line = body_start_idx.min(lines.len());
            for (offset, &line) in lines
                .get(body_start_idx..)
                .unwrap_or(&[])
                .iter()
                .enumerate()
            {
                let t = line.trim();
                if t.is_empty() || t.starts_with('#') {
                    continue;
                }
                let indent = line.len() - line.trim_start().len();
                if indent > base_indent {
                    last_body_line = body_start_idx + offset + 1;
                } else {
                    break;
                }
            }
            return last_body_line;
        }

        // Single-line semicolon-terminated declaration without opening brace
        if trimmed.ends_with(';') && !trimmed.contains('{') {
            return start_idx + 1;
        }

        // Brace-delimited block (Rust, TS/JS, Go, C/C++)
        let mut depth = 0i32;
        let mut saw_open = false;
        for (offset, &line) in lines[start_idx..].iter().enumerate().take(500) {
            let (line_delta, line_opened) = Self::scan_code_braces(line);
            depth += line_delta;
            if line_opened {
                saw_open = true;
            }
            if saw_open && depth <= 0 {
                return start_idx + offset + 1;
            }
            if !saw_open && offset >= 4 {
                break;
            }
        }
        start_idx + 1
    }

    fn compact_token(s: &str) -> String {
        let mut out = String::with_capacity(s.len().min(128));
        for ch in s.chars() {
            if matches!(
                ch,
                '⟦' | '⟧' | '⟪' | '⟫' | '⊢' | '⋮' | '↦' | '⇒' | '│' | '\n' | '\r'
            ) {
                out.push('_');
            } else if ch.is_whitespace() {
                if !out.ends_with('_') {
                    out.push('_');
                }
            } else {
                out.push(ch);
            }
            if out.len() >= 128 {
                break;
            }
        }
        out.trim_matches('_').to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write;

    #[test]
    fn compresses_large_cargo_test_log_by_over_25x_while_preserving_panic_and_all_failures() {
        let mut log = String::from("running 300 tests\n");
        for i in 0..299 {
            let _ = writeln!(log, "test module::sub::test_case_number_{i:04} ... ok");
        }
        log.push_str("test module::allocator::test_page_align ... FAILED\n");
        log.push_str(
            "thread 'module::allocator::test_page_align' panicked at src/alloc.rs:412:9: index out of bounds\n",
        );
        log.push_str("@@ -1,2 +1,2 @@\n-left\n+right\n");
        log.push_str("test result: FAILED. 299 passed; 1 failed;\n");

        let (id, _) = ArtifactId::hash_bytes(log.as_bytes());
        let pkt =
            StructuralSkeletonizer::compress_to_packet("cargo_test", &log, Some(id), [0xAA; 8]);
        assert_eq!(pkt.kind, PacketKind::ToolStub);
        let rendered = pkt.render();

        let ratio = (log.len() as f64) / (rendered.len() as f64);
        assert!(
            ratio >= 25.0,
            "expected >= 25x compression ratio, got {ratio:.2}x"
        );
        assert!(rendered.contains("✓pass:299"));
        assert!(rendered.contains("!fail:1"));
        assert!(rendered.contains("src/alloc.rs:412:9"));
        assert!(rendered.contains(&id.to_string()));

        // Verify >16 failing tests are all preserved without truncation
        let mut many_fail_log = String::from("running 22 tests\n");
        for i in 0..22 {
            let _ = writeln!(many_fail_log, "error[E0425]: unresolved symbol_{i}");
            let _ = writeln!(many_fail_log, "test suite::fail_case_{i:02} ... FAILED");
        }
        many_fail_log.push_str("test result: FAILED. 0 passed; 22 failed;\n");
        let many_pkt = StructuralSkeletonizer::compress_to_packet(
            "cargo_many_fail",
            &many_fail_log,
            None,
            [0xBB; 8],
        );
        let many_str = many_pkt.render();
        assert!(many_str.contains("!fail:22"));
        assert!(many_str.contains("suite::fail_case_21"));
    }

    #[test]
    fn compresses_pytest_tsc_vitest_and_go_test_logs_with_100_percent_failure_recall() {
        // 1. Python pytest log (verify progress + summary line deduplicates to !fail:1)
        let mut pytest_log = String::from(
            "============================= test session starts =============================\n",
        );
        for i in 0..120 {
            let _ = writeln!(
                pytest_log,
                "tests/test_engine.py::test_step_{i:03} PASSED [ {i}%]"
            );
        }
        pytest_log.push_str("tests/test_mmu.py::test_page_fault FAILED [100%]\n");
        pytest_log.push_str(
            "================================== FAILURES ===================================\n",
        );
        pytest_log.push_str("File \"tests/test_mmu.py\", line 88, in test_page_fault\n");
        pytest_log.push_str("E   AssertionError: expected epoch 4, got 3\n");
        pytest_log.push_str("FAILED tests/test_mmu.py::test_page_fault - AssertionError\n");

        let py_pkt =
            StructuralSkeletonizer::compress_to_packet("pytest", &pytest_log, None, [0x11; 8]);
        let py_str = py_pkt.render();
        assert!(
            (pytest_log.len() as f64) / (py_str.len() as f64) >= 15.0,
            "pytest log should compress >= 15x"
        );
        assert!(py_str.contains("✓pass:120"));
        assert!(py_str.contains("!fail:1"));
        assert!(py_str.contains("tests/test_mmu.py::test_page_fault"));
        assert!(py_str.contains("AssertionError"));

        // 2. TypeScript tsc + vitest log
        let mut ts_log = String::from("Test Files  1 failed | 80 passed (81)\n");
        for i in 0..80 {
            let _ = writeln!(
                ts_log,
                " ✓ src/components/widget_{i:03}.test.ts (4 tests) 12ms"
            );
        }
        ts_log.push_str(" ❯ src/router/auth.test.ts (2 tests | 1 failed)\n");
        ts_log.push_str(
            "src/router/auth.ts(142,17): error TS2345: Argument of type 'null' is not assignable\n",
        );
        ts_log.push_str("AssertionError: expected 200 to equal 401\n");

        let ts_pkt = StructuralSkeletonizer::compress_to_packet("vitest", &ts_log, None, [0x22; 8]);
        let ts_str = ts_pkt.render();
        assert!(ts_str.contains("✓pass:80"));
        assert!(ts_str.contains("!fail:1"));
        assert!(ts_str.contains("!FAIL@L82:src/router/auth.test.ts"));
        assert!(ts_str.contains("TS2345"));

        // 3. Go `go test` log
        let mut go_log = String::new();
        for i in 0..80 {
            let _ = writeln!(go_log, "=== RUN   TestWorkerPool_{i:03}");
            let _ = writeln!(go_log, "--- PASS: TestWorkerPool_{i:03} (0.00s)");
        }
        go_log.push_str("=== RUN   TestRaftQuorumSplit\n");
        go_log.push_str("    raft_test.go:214: expected leader term 7, got 6\n");
        go_log.push_str("--- FAIL: TestRaftQuorumSplit (0.04s)\n");
        go_log.push_str("FAIL\tgithub.com/acme/consensus\t0.412s\n");

        let go_pkt =
            StructuralSkeletonizer::compress_to_packet("go_test", &go_log, None, [0x33; 8]);
        let go_str = go_pkt.render();
        assert!(go_str.contains("✓pass:80"));
        assert!(go_str.contains("!fail:1"));
        assert!(go_str.contains("TestRaftQuorumSplit"));
        assert!(go_str.contains("raft_test.go:214"));
    }

    #[test]
    fn skeletonizes_unified_diffs_multi_language_source_code_quoted_braces_lifetimes_and_utf8() {
        let diff = "\
diff --git a/src/lib.rs b/src/lib.rs
--- /dev/null
+++ b/src/lib.rs
@@ -0,0 +1,5 @@
-fn old_helper() {}
--- removed sql comment that starts with three dashes
+pub fn new_helper() {}
+pub struct Config { pub x: u32 }
+// INVARIANT: monotonic epoch counter
";
        let diff_pkt = StructuralSkeletonizer::compress_to_packet("git_diff", diff, None, [0; 8]);
        assert_eq!(diff_pkt.kind, PacketKind::CodeDelta);
        let diff_str = diff_pkt.render();
        assert!(diff_str.contains("hunks:+3/-2"));
        assert!(diff_str.contains("file:src/lib.rs"));
        assert!(diff_str.contains("hunk:-0,0_+1,5"));
        assert!(diff_str.contains("⊕sym:new_helper()"));
        assert!(diff_str.contains("⊖sym:old_helper()"));
        assert!(diff_str.contains("✓pin:"));

        // Multi-line Rust + Python + Go source with lifetimes, quoted braces, multi-line Python def, and Go import block
        let src = "\
package scheduler

import (
    \"context\"
    \"fmt\"
)

pub mod submod;
use std::collections::BTreeMap;

pub async fn serve() {}
pub fn multi_line_worker<'a>(label: &'static str, limit: usize) -> &'a str {
    let url = \"http://example.com/}@@ -1,1 +1,1 @@ panicked at\";
    let ch = '}';
    let esc_ch = '\\'';
    let escaped = \"\\\"}\";
    let _ = limit + url.len() + (ch as usize) + (esc_ch as usize) + escaped.len(); // trailing comment }
    label
}
pub enum Mode { Fast }
pub trait Handler {
    fn on_event(&self);
}
type RequestId = u64;
def validate_assertion(
    steps: int,
) -> float:
    # comment inside python block
    if steps < 0:
        raise AssertionError(\"bad steps\")

    return 1.0
pub fn unclosed_signature(
    a: u32,
    b: u32,
    c: u32,
    d: u32,
    e: u32,
)
pub fn unicode_boundary_signature_with_multibyte_glyphs_αβγδεζηθ() -> u32 { 1 }
use super::*;
// INVARIANT: lock-free ring
// PASSKEY_ALPHA_123
";
        let src_pkt = StructuralSkeletonizer::compress_to_packet("src/lib.rs", src, None, [1; 8]);
        assert_eq!(src_pkt.kind, PacketKind::CodeDelta);
        let src_str = src_pkt.render();
        assert!(src_str.contains("imports:L3-9(3)"));
        assert!(src_str.contains("⊕mod@L1:scheduler"));
        assert!(src_str.contains("⊕mod@L8:submod"));
        assert!(src_str.contains("⊕L11:serve()"));
        assert!(
            src_str.contains("⊕L12-19:multi_line_worker<'a>(label:_&'static_str,_limit:_usize)")
        );
        assert!(src_str.contains("⊕L24:RequestId_=_u64;"));
        assert!(src_str.contains("⊕L25-32:validate_assertion("));
        assert!(src_str.contains("PASSKEY_ALPHA_123"));

        let plain = "just a plain note without any symbol declarations\nsecond line\n";
        let plain_pkt =
            StructuralSkeletonizer::compress_to_packet("notes.txt", plain, None, [2; 8]);
        assert!(plain_pkt.render().contains("head:just_a_plain_note"));
    }
}
