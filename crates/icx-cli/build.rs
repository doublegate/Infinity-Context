// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later
//! Embed compilation provenance; runtime RUSTFLAGS may differ from the built executable.
use std::env;
use std::process::Command;

fn main() {
    for name in ["TARGET", "PROFILE", "CARGO_ENCODED_RUSTFLAGS", "RUSTC"] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    for (source, target) in [
        ("TARGET", "ICX_BUILD_TARGET"),
        ("PROFILE", "ICX_BUILD_PROFILE"),
        ("CARGO_ENCODED_RUSTFLAGS", "ICX_BUILD_RUSTFLAGS"),
    ] {
        let value = env::var(source)
            .unwrap_or_default()
            .replace('\u{1f}', " ")
            .replace('\n', "\\n");
        println!("cargo:rustc-env={target}={value}");
    }
    let version = Command::new(env::var_os("RUSTC").expect("Cargo supplies RUSTC"))
        .arg("--version")
        .output()
        .expect("read compiler version");
    assert!(version.status.success(), "rustc --version failed");
    println!(
        "cargo:rustc-env=ICX_BUILD_RUSTC={}",
        String::from_utf8_lossy(&version.stdout).trim()
    );
}
