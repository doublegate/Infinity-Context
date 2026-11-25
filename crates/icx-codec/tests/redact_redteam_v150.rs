// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later AND AGPL-3.0-or-later

//! The independent red-team pass on the structural redactor (v1.5.0, PR #103): every input it
//! tried that the redactor now hides must stay hidden, and redaction must stay idempotent on it.
//! The inputs it found still leaking (shapes planned for a later release) are tracked in their
//! own issue, not here. Placeholders are low-entropy letters (`qqzz`) or an all-`a` UUID.

use icx_codec::SecretRedactor;

#[test]
fn red_team_inputs_stay_hidden_and_idempotent() {
    let cases: Vec<String> =
        serde_json::from_str(include_str!("fixtures/redteam-v150-clean.json")).unwrap();
    assert!(cases.len() > 250, "{}", cases.len());
    let mut seen = std::collections::HashSet::new();
    for case in &cases {
        // Every case must carry a placeholder the assertion can find, and appear once
        // (review of #103: cases without one passed without being checked).
        assert!(
            case.contains("qqzz") || case.contains("aaaa-aaaa"),
            "unchecked case: {case:?}"
        );
        assert!(seen.insert(case.as_str()), "duplicate case: {case:?}");
        let once = SecretRedactor::redact(case);
        assert!(
            !once.contains("qqzz") && !once.contains("aaaa-aaaa"),
            "{case:?} -> {once:?}"
        );
        assert_eq!(SecretRedactor::redact(&once), once, "{case:?}");
    }
}
