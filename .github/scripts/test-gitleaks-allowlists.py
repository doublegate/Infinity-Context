#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 DoubleGate
# SPDX-License-Identifier: GPL-3.0-or-later
"""Exercise the installed scanner against synthetic allowlist boundary controls."""

import argparse
import hashlib
import json
import subprocess
import tempfile
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("gitleaks", type=Path, help="Explicit scanner executable")
    parser.add_argument("--config", type=Path, default=Path(__file__).resolve().parents[2] / ".gitleaks.toml")
    parser.add_argument("--report-dir", type=Path, help="Retain redacted reports and summary")
    args = parser.parse_args()
    executable = args.gitleaks.resolve(strict=True)
    config = args.config.resolve(strict=True)
    version = subprocess.run([str(executable), "version"], check=True, capture_output=True, text=True).stdout.strip()
    # Both digests identify public repository files, not credentials. Other controls are
    # deliberately nonfunctional values assembled here so the script itself stays scannable.
    approved = "2a872b81206dd5b4d960cc132db9cadbb2c3eb1115d136e5e7952f97eea3145f"
    other_hex = hashlib.sha256(b"synthetic-nonfunctional-credential-control").hexdigest()
    credential = "sk-" + "proj-" + "1234567890" + "abcdefghijklmnop"
    audit_fixture = "password:" + "\\n  " + "ask-" + "123456789=x" + "\\n"
    manifest = "docs/releases/v1.2.0/evidence/control-source-manifest.json"
    audit_path = "docs/codex/evidence/codec-redact-readable.txt"
    metadata = {"crates/icx-codec/tests/fixtures/redact-pending-key-block.bin": approved}
    controls = [
        ("approved-sha", manifest, metadata, 0, []),
        ("outside-sha", "outside-source-manifest.json", metadata, 1, [2]),
        ("credential-inside", manifest, {**metadata, "OPENAI_API_KEY": credential}, 1, [3]),
        ("hex-credential-inside", manifest, {**metadata, "API_KEY": other_hex}, 1, [3]),
        ("audit-with-credential", audit_path, audit_fixture + '\nOPENAI_API_KEY="' + credential + '"\n', 1, [2]),
        ("audit-exact", audit_path, audit_fixture, 0, []),
    ]
    scratch_root = Path(tempfile.gettempdir()) / "Infinity-Context" / "gitleaks-controls"
    scratch_root.mkdir(parents=True, exist_ok=True)
    results = []
    with tempfile.TemporaryDirectory(prefix="allowlists-", dir=scratch_root) as scratch:
        scratch = Path(scratch)
        for name, relative_path, value, expected_exit, expected_lines in controls:
            control_root = scratch / name
            target = control_root / relative_path
            target.parent.mkdir(parents=True)
            content = json.dumps(value, indent=2) + "\n" if isinstance(value, dict) else value
            target.write_text(content)
            report_path = scratch / (name + ".json")
            command = [str(executable), "dir", ".", "--config", str(config), "--redact",
                       "--report-format", "json", "--report-path", str(report_path), "--no-banner"]
            process = subprocess.run(command, cwd=control_root, capture_output=True, text=True)
            findings = json.loads(report_path.read_text()) if report_path.exists() else []
            observed = sorted((item["File"], item["RuleID"], item["StartLine"]) for item in findings)
            expected = [(relative_path, "generic-api-key", line) for line in expected_lines]
            passed = process.returncode == expected_exit and observed == expected
            results.append({"control": name, "passed": passed, "exit_code": process.returncode,
                            "expected_exit_code": expected_exit,
                            "findings": [{"file": path, "rule": rule, "line": line}
                                         for path, rule, line in observed]})
            if args.report_dir:
                args.report_dir.mkdir(parents=True, exist_ok=True)
                (args.report_dir / ("secret-policy-safe-controls-" + name + ".json")).write_text(
                    json.dumps(findings, indent=2) + "\n")
    summary = {"scanner_version": version, "config_sha256": hashlib.sha256(config.read_bytes()).hexdigest(),
               "controls": results, "passed": all(result["passed"] for result in results)}
    if args.report_dir:
        (args.report_dir / "secret-policy-safe-controls-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps(summary, indent=2))
    return 0 if summary["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
