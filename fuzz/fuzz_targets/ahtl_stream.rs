// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]
use icx_codec::ahtl::{AhtlPacket, AhtlStream, EpochHeader, WirePacket};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        if let Ok(stream) = AhtlStream::parse_stream(s) {
            for p in &stream.packets {
                // Re-rendering a parsed Profile B packet must parse back to the same packet.
                if let WirePacket::ProfileB(pkt) = p {
                    let wire = pkt.render();
                    assert_eq!(&AhtlPacket::parse(&wire).expect("re-parse"), pkt);
                }
            }
        }
        let _ = EpochHeader::parse(s);
    }
});
