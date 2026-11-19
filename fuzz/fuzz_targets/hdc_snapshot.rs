// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]
use icx_hdc::HdcAssociativeIndex;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Hostile snapshots must fail with an error, never panic or over-allocate.
    if let Ok(idx) = HdcAssociativeIndex::decode_snapshot(data) {
        let _ = idx.query_associative("probe", 3);
    }
});
