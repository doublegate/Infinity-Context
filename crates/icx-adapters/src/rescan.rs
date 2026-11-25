// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later AND AGPL-3.0-or-later

//! `icx rescan` (v1.5.0, T-74-001): what the current redactor would hide in content stored under
//! older rules. Report only: nothing is written (owner decision, 2026-11-25; re-redaction is
//! T-91-005, #102). The report holds counts and handles, never the text or the values.

use crate::mmu::InfinityContextMmu;
use icx_codec::SecretRedactor;
use serde::Serialize;
use std::collections::BTreeMap;

/// The report of one rescan.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RescanReport {
    /// Always `"report"`: this version never rewrites the store.
    pub mode: &'static str,
    pub artifacts_scanned: usize,
    /// Artifacts whose stored text the current redactor would change.
    pub artifacts_affected: usize,
    /// Markers the current redactor would add to stored text, by kind (`ENV_SECRET`,
    /// `HIGH_ENTROPY`, ...): the per-artifact increase, so a newly caught secret folded into an
    /// existing marker on the same line is not counted. Subjects count in `subjects_affected`.
    pub markers_by_kind: BTreeMap<String, usize>,
    /// Bytes of stored text that would be hidden, approximately: input bytes outside the common
    /// prefix and suffix of each changed line, lines compared in order (a multi-line value that
    /// collapses shifts the comparison, so this is an upper estimate).
    pub bytes_affected: usize,
    /// Indexed subjects (command lines, paths) the current redactor would change.
    pub subjects_affected: usize,
    /// Handles of affected artifacts, in id order, at most `limit`.
    pub handles: Vec<String>,
    pub handles_truncated: bool,
    /// Artifacts whose stored text could not be read back (integrity problem, reported apart).
    pub unreadable: usize,
}

/// `[REDACTED:<KIND>]` markers in `text`, by kind.
fn markers(text: &str) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    let mut rest = text;
    while let Some(i) = rest.find("[REDACTED:") {
        let after = &rest[i + "[REDACTED:".len()..];
        match after.find(']') {
            Some(end)
                if end > 0
                    && after[..end]
                        .bytes()
                        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_') =>
            {
                *out.entry(after[..end].to_owned()).or_insert(0) += 1;
                rest = &after[end + 1..];
            }
            _ => rest = after,
        }
    }
    out
}

/// Bytes of `before` that differ from `after`, line by line (outside each line's common prefix
/// and suffix). Lines are compared in order; lines `before` has beyond `after` count whole,
/// lines only `after` has count nothing.
fn changed_bytes(before: &str, after: &str) -> usize {
    let mut a = before.split('\n');
    let mut b = after.split('\n');
    let mut total = 0usize;
    loop {
        match (a.next(), b.next()) {
            (None, None) => return total,
            (Some(x), None) => total += x.len(),
            (None, Some(_)) => {}
            (Some(x), Some(y)) if x == y => {}
            (Some(x), Some(y)) => {
                let (x, y) = (x.as_bytes(), y.as_bytes());
                let pre = x.iter().zip(y).take_while(|(p, q)| p == q).count();
                let suf = x[pre..]
                    .iter()
                    .rev()
                    .zip(y[pre..].iter().rev())
                    .take_while(|(p, q)| p == q)
                    .count();
                total += x.len() - pre - suf;
            }
        }
    }
}

/// Runs the current redactor over every stored artifact and indexed subject of `mmu`, without
/// changing anything.
#[must_use]
pub fn rescan(mmu: &InfinityContextMmu, limit: usize) -> RescanReport {
    let mut report = RescanReport {
        mode: "report",
        artifacts_scanned: 0,
        artifacts_affected: 0,
        markers_by_kind: BTreeMap::new(),
        bytes_affected: 0,
        subjects_affected: 0,
        handles: Vec::new(),
        handles_truncated: false,
        unreadable: 0,
    };
    let mut ids: Vec<_> = mmu.cas.artifact_entries().map(|a| a.id).collect();
    // Id order, so a `--limit` cut is deterministic.
    ids.sort();
    for id in ids {
        report.artifacts_scanned += 1;
        let Ok(bytes) = mmu.cas.deref_bytes(id) else {
            report.unreadable += 1;
            continue;
        };
        let text = String::from_utf8_lossy(&bytes);
        let clean = SecretRedactor::redact(&text);
        if clean == text {
            continue;
        }
        report.artifacts_affected += 1;
        let (old, new) = (markers(&text), markers(&clean));
        for (kind, n) in new {
            let added = n.saturating_sub(old.get(&kind).copied().unwrap_or(0));
            if added > 0 {
                *report.markers_by_kind.entry(kind).or_insert(0) += added;
            }
        }
        report.bytes_affected += changed_bytes(&text, &clean);
        if report.handles.len() < limit {
            report.handles.push(id.to_string());
        } else {
            report.handles_truncated = true;
        }
    }
    report.subjects_affected = mmu
        .hdc_events
        .iter()
        .filter(|(_, subject, ..)| SecretRedactor::redact(subject) != *subject)
        .count();
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Content stored under an older rule: raw bytes put straight into the CAS, as a store
    /// written before the current redactor would hold them.
    #[test]
    fn a_secret_stored_under_older_rules_is_reported_without_its_value() {
        let mut mmu = InfinityContextMmu::default();
        mmu.ingest_payload(icx_core::AgentKind::ClaudeCode, "Bash:ls", "a\nb\n", false);
        let (old, _) = mmu
            .cas
            .put_bytes(b"config ok\nAPI_KEY=qqzzqqzzqqzz\nexport GITHUB_TOKEN=\"rrssrrss\"\n");
        let report = rescan(&mmu, 50);
        assert_eq!(report.mode, "report");
        assert_eq!(report.artifacts_scanned, 2);
        assert_eq!(report.artifacts_affected, 1);
        assert_eq!(report.markers_by_kind.get("ENV_SECRET"), Some(&2));
        assert_eq!(report.handles, [old.to_string()]);
        assert!(!report.handles_truncated);
        assert!(report.bytes_affected >= "qqzzqqzzqqzz".len() + "rrssrrss".len());
        let json = serde_json::to_string(&report).unwrap();
        assert!(!json.contains("qqzz") && !json.contains("rrss"), "{json}");
        // Nothing changed: the stored text is still the original.
        assert!(
            String::from_utf8(mmu.cas.deref_bytes(old).unwrap())
                .unwrap()
                .contains("qqzzqqzzqqzz")
        );
    }

    #[test]
    fn the_handle_list_is_bounded() {
        let mut mmu = InfinityContextMmu::default();
        for i in 0..3 {
            let _ = mmu
                .cas
                .put_bytes(format!("PASSWORD=qqzzqqzz{i}\n").as_bytes());
        }
        let report = rescan(&mmu, 2);
        assert_eq!(report.artifacts_affected, 3);
        assert_eq!(report.handles.len(), 2);
        assert!(report.handles_truncated);
    }

    #[test]
    fn markers_are_counted_by_kind() {
        let m =
            markers("a [REDACTED:ENV_SECRET] b [REDACTED:ENV_SECRET] [REDACTED:X] [REDACTED:bad]");
        assert_eq!(m.get("ENV_SECRET"), Some(&2));
        assert_eq!(m.get("X"), Some(&1));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn changed_bytes_counts_only_the_differing_middle() {
        assert_eq!(changed_bytes("same\nkey=secret\n", "same\nkey=[R]\n"), 6);
        assert_eq!(changed_bytes("a\nb", "a"), 1);
        assert_eq!(changed_bytes("a", "a\nb"), 0);
    }
}
