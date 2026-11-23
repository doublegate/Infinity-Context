//! T-65-009: the Hermes `transform_terminal_output` event, driven as the shipped general plugin
//! drives it (stand-in test; live Hermes verification is deferred to v1.5.0, #89).

use icx_adapters::{HermesAdapter, InfinityContextMmu};
use serde_json::{Value, json};

fn reply(payload: &Value, mmu: &mut InfinityContextMmu) -> Value {
    let out =
        HermesAdapter::handle_hook("transform_terminal_output", &payload.to_string(), mmu).unwrap();
    serde_json::from_str(&out).unwrap()
}

fn big() -> String {
    (1..=3000).map(|i| i.to_string() + "\n").collect()
}

#[test]
fn a_large_successful_result_becomes_a_stub_whose_handle_holds_the_output() {
    let mut mmu = InfinityContextMmu::default();
    let r = reply(
        &json!({"command": "seq 1 3000", "output": big(), "returncode": 0}),
        &mut mmu,
    );
    let stub = r["output"].as_str().expect("a stub");
    assert!(stub.starts_with("⟪ICX:STUB│#icx:"), "{stub}");
    assert!(stub.len() < big().len() / 4);
    assert_eq!(mmu.cas.artifact_count(), 1);
}

#[test]
fn small_failed_and_empty_results_pass_through() {
    let mut mmu = InfinityContextMmu::default();
    for payload in [
        json!({"command": "echo hi", "output": "hi\n", "returncode": 0}),
        json!({"command": "make", "output": big(), "returncode": 2}),
        json!({"command": "true", "output": "", "returncode": 0}),
        json!({}),
        // icx's own commands and already-framed output are never stubbed again.
        json!({"command": "icx deref '#icx:0123456789abcdef'", "output": big(), "returncode": 0}),
        json!({"command": "ICX_DIR=/w /home/u/.local/bin/icx deref x", "output": big(), "returncode": 0}),
        json!({"command": "cat log", "output": format!("⟪ICX:PROVENANCE│id=x⟫\n{}", big()), "returncode": 0}),
    ] {
        assert_eq!(reply(&payload, &mut mmu), json!({}), "{payload}");
    }
}
