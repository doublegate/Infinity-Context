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
    let parallel = &report["measurements"]["hdc_scan_10k_parallel"];
    assert_eq!(parallel["candidates"], 10_000);
    assert_eq!(parallel["slo_gated"], true);
    assert_eq!(parallel["ceiling_us"], 2_000.0);
    assert!(parallel["workers"].as_u64().unwrap() >= 1);
    // The parallel scan is exact: same minimum as the sequential scan of the same fixture.
    assert_eq!(
        parallel["min_distance"],
        report["measurements"]["hdc_scan_10k"]["min_distance"]
    );
    for key in ["hdc_scan_1k", "hdc_scan_10k"] {
        assert_eq!(report["measurements"][key]["kind"], "measured");
        assert!(report["measurements"][key]["us"].as_f64().unwrap() > 0.0);
        assert_eq!(
            report["measurements"][key]["cache_condition"],
            "after_independent_candidate_construction"
        );
        // The single-threaded 10k scan is reported but not gated; its exact parallel
        // counterpart below is the gated 10k measurement.
        assert_eq!(
            report["measurements"][key]["slo_gated"],
            key == "hdc_scan_1k"
        );
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
