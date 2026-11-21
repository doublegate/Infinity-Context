//! Benchmark output is machine-readable and reports two actual scan sizes (ICX-CODEX-072).
use std::process::Command;

#[test]
fn json_benchmark_reports_measured_scans_and_build_provenance() {
    let output = Command::new(env!("CARGO_BIN_EXE_icx"))
        .args(["benchmark", "--json"])
        .output()
        .unwrap();
    assert!(
        output.stdout.starts_with(b"{"),
        "machine mode mixed prose with JSON"
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["measurements"]["hdc_scan_1k"]["candidates"], 1_000);
    assert_eq!(report["measurements"]["hdc_scan_10k"]["candidates"], 10_000);
    for key in ["hdc_scan_1k", "hdc_scan_10k"] {
        assert_eq!(report["measurements"][key]["kind"], "measured");
        assert!(report["measurements"][key]["us"].as_f64().unwrap() > 0.0);
        assert_eq!(
            report["measurements"][key]["cache_condition"],
            "after_independent_candidate_construction"
        );
        assert_eq!(report["measurements"][key]["slo_gated"], true);
        for (suffix, condition) in [
            ("_warm", "immediately_prescanned"),
            ("_cold", "after_64MiB_scrub"),
        ] {
            let measured = &report["measurements"][format!("{key}{suffix}")];
            assert_eq!(measured["kind"], "measured");
            assert_eq!(measured["cache_condition"], condition);
            assert_eq!(measured["slo_gated"], false);
            assert_eq!(
                measured["min_distance"],
                report["measurements"][key]["min_distance"]
            );
            assert!(measured["us"].as_f64().unwrap() > 0.0);
        }
    }
    assert_eq!(report["fixtures"]["hdc_blake3"].as_str().unwrap().len(), 64);
    assert!(
        report["build"]["rustc"]
            .as_str()
            .unwrap()
            .starts_with("rustc ")
    );
    assert!(report["build"]["target"].as_str().unwrap().contains('-'));
    assert!(report["build"]["rustflags"].is_string());
    assert_eq!(report["gates_passed"], output.status.success());
}
