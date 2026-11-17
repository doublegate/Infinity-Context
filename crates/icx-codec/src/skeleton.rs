//! Single-pass domain-specific Structural Skeletonizers (`StructuralSkeletonizer`) for
//! source code, unified diffs, and compiler/test logs.

use crate::ahtl::{AhtlClause, AhtlPacket, ClauseOp, PacketKind};
use crate::redact::SecretRedactor;
use icx_core::ArtifactId;

/// Compresses raw source code, diffs, and build/test logs into structured `AhtlPacket` frames.
#[derive(Clone, Debug, Default)]
pub struct StructuralSkeletonizer;

impl StructuralSkeletonizer {
    /// Automatically detects payload domain (test/build log, unified diff, or source/text)
    /// and compiles a high-density `AhtlPacket`.
    pub fn compress_to_packet(
        subject: &str,
        raw_text: &str,
        deref_id: Option<ArtifactId>,
        hologram_bytes: [u8; 8],
    ) -> AhtlPacket {
        let clean = SecretRedactor::sanitize(raw_text);
        let total_lines = clean.lines().count().max(1) as u32;

        if Self::looks_like_test_or_build_log(&clean) {
            Self::skeletonize_log(subject, &clean, total_lines, deref_id, hologram_bytes)
        } else if Self::looks_like_diff(&clean) {
            Self::skeletonize_diff(subject, &clean, total_lines, deref_id, hologram_bytes)
        } else {
            Self::skeletonize_source_or_text(subject, &clean, total_lines, deref_id, hologram_bytes)
        }
    }

    fn looks_like_test_or_build_log(text: &str) -> bool {
        text.contains("running ") && text.contains("test result:")
            || text.contains("Compiling ")
            || text.contains("error[E")
            || text.contains("panicked at")
    }

    fn looks_like_diff(text: &str) -> bool {
        text.contains("diff --git ") || text.contains("@@ -")
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

        for (idx, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("test ") && trimmed.ends_with(" ... ok") {
                pass_count += 1;
            } else if trimmed.starts_with("test ") && trimmed.ends_with(" ... FAILED") {
                fail_count += 1;
                let test_name = trimmed
                    .strip_prefix("test ")
                    .and_then(|s| s.strip_suffix(" ... FAILED"))
                    .unwrap_or(trimmed);
                clauses.push(AhtlClause::new(
                    ClauseOp::Anomaly,
                    format!("FAIL@L{}", idx + 1),
                    Some(Self::compact_token(test_name)),
                ));
            } else if trimmed.contains("panicked at") || trimmed.starts_with("error[E") {
                if clauses.len() < 12 {
                    clauses.push(AhtlClause::new(
                        ClauseOp::Anomaly,
                        format!("ERR@L{}", idx + 1),
                        Some(Self::compact_token(trimmed)),
                    ));
                }
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

        AhtlPacket {
            kind: PacketKind::ToolStub,
            subject: Self::compact_token(subject),
            line_range: Some((1, total_lines)),
            clauses,
            deref_id,
            hologram_bytes,
        }
    }

    fn skeletonize_diff(
        subject: &str,
        text: &str,
        total_lines: u32,
        deref_id: Option<ArtifactId>,
        hologram_bytes: [u8; 8],
    ) -> AhtlPacket {
        let mut clauses = Vec::new();
        let mut added = 0usize;
        let mut removed = 0usize;

        for line in text.lines() {
            if line.starts_with("+++ b/") || line.starts_with("--- a/") {
                continue;
            }
            if let Some(rest) = line.strip_prefix('+') {
                added += 1;
                if let Some(sym) = Self::extract_symbol_signature(rest) {
                    if clauses.len() < 12 {
                        clauses.push(AhtlClause::new(ClauseOp::Added, "sym", Some(sym)));
                    }
                }
            } else if let Some(rest) = line.strip_prefix('-') {
                removed += 1;
                if let Some(sym) = Self::extract_symbol_signature(rest) {
                    if clauses.len() < 12 {
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

        AhtlPacket {
            kind: PacketKind::CodeDelta,
            subject: Self::compact_token(subject),
            line_range: Some((1, total_lines)),
            clauses,
            deref_id,
            hologram_bytes,
        }
    }

    fn skeletonize_source_or_text(
        subject: &str,
        text: &str,
        total_lines: u32,
        deref_id: Option<ArtifactId>,
        hologram_bytes: [u8; 8],
    ) -> AhtlPacket {
        let mut clauses = Vec::new();

        for (idx, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if let Some(sig) = Self::extract_symbol_signature(trimmed) {
                if clauses.len() < 16 {
                    clauses.push(AhtlClause::new(
                        ClauseOp::Added,
                        format!("L{}", idx + 1),
                        Some(sig),
                    ));
                }
            } else if (trimmed.contains("PASSKEY")
                || trimmed.contains("INVARIANT")
                || trimmed.contains("SAFETY:"))
                && clauses.len() < 18
            {
                clauses.push(AhtlClause::new(
                    ClauseOp::Verified,
                    format!("pin@L{}", idx + 1),
                    Some(Self::compact_token(trimmed)),
                ));
            }
        }

        if clauses.is_empty() {
            let head = text.lines().next().unwrap_or("");
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

        AhtlPacket {
            kind: PacketKind::CodeDelta,
            subject: Self::compact_token(subject),
            line_range: Some((1, total_lines)),
            clauses,
            deref_id,
            hologram_bytes,
        }
    }

    fn extract_symbol_signature(trimmed: &str) -> Option<String> {
        let prefixes = [
            "pub fn ",
            "pub async fn ",
            "fn ",
            "pub struct ",
            "struct ",
            "pub enum ",
            "enum ",
            "pub trait ",
            "trait ",
            "def ",
            "async def ",
            "class ",
            "func ",
            "export function ",
            "export interface ",
        ];
        for p in prefixes {
            if let Some(rest) = trimmed.strip_prefix(p) {
                let sig_end = rest.find('{').unwrap_or(rest.len()).min(56);
                let clean = Self::compact_token(&rest[..sig_end]);
                return Some(clean);
            }
        }
        None
    }

    fn compact_token(s: &str) -> String {
        let mut out = String::with_capacity(s.len().min(128));
        for ch in s.chars() {
            if matches!(ch, '⟦' | '⟧' | '⟪' | '⟫' | '⊢' | '⋮' | '↦' | '\n' | '\r') {
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
    fn compresses_large_cargo_test_log_by_over_25x_while_preserving_panic() {
        let mut log = String::from("running 300 tests\n");
        for i in 0..299 {
            let _ = writeln!(log, "test module::sub::test_case_number_{i:04} ... ok");
        }
        log.push_str("test module::allocator::test_page_align ... FAILED\n");
        log.push_str(
            "thread 'module::allocator::test_page_align' panicked at src/alloc.rs:412:9: index out of bounds\n",
        );
        log.push_str("test result: FAILED. 299 passed; 1 failed;\n");

        let (id, _) = ArtifactId::hash_bytes(log.as_bytes());
        let pkt =
            StructuralSkeletonizer::compress_to_packet("cargo_test", &log, Some(id), [0xAA; 8]);
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
    }

    #[test]
    fn skeletonizes_unified_diffs_source_code_and_plain_text() {
        let diff = "\
diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,4 @@
-fn old_helper() {}
+pub fn new_helper() {}
+pub struct Config { pub x: u32 }
";
        let diff_pkt = StructuralSkeletonizer::compress_to_packet("git_diff", diff, None, [0; 8]);
        assert_eq!(diff_pkt.kind, PacketKind::CodeDelta);
        let diff_str = diff_pkt.render();
        assert!(diff_str.contains("hunks:+2/-1"));
        assert!(diff_str.contains("⊕sym:new_helper()"));
        assert!(diff_str.contains("⊖sym:old_helper()"));

        let src = "\
pub async fn serve() {}
pub enum Mode { Fast }
pub trait Handler {}
// INVARIANT: lock-free ring
// PASSKEY_ALPHA_123
";
        let src_pkt = StructuralSkeletonizer::compress_to_packet("src/lib.rs", src, None, [1; 8]);
        let src_str = src_pkt.render();
        assert!(src_str.contains("serve()"));
        assert!(src_str.contains("PASSKEY_ALPHA_123"));

        let plain = "just a plain note without any symbol declarations\nsecond line\n";
        let plain_pkt =
            StructuralSkeletonizer::compress_to_packet("notes.txt", plain, None, [2; 8]);
        assert!(plain_pkt.render().contains("head:just_a_plain_note"));
    }
}
