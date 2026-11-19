// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]
use icx_adapters::McpPagerServer;
use icx_adapters::mmu::InfinityContextMmu;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let mut mmu = InfinityContextMmu::default();
        let _ = McpPagerServer::handle_jsonrpc(s, &mut mmu);
    }
});
