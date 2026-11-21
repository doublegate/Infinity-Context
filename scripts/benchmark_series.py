#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 DoubleGate
# SPDX-License-Identifier: GPL-3.0-or-later
"""Run independent `icx benchmark --json` processes; retain failures and warm-up separately.

Build the executable with explicit target flags first. Compilation flags and target are embedded
by icx, so changing runtime RUSTFLAGS cannot mislabel the executable. Run on a quiet host; this
script does not establish CPU isolation or turn in-process hook timings into daemon timings.
"""

import argparse
import datetime
import json
import math
import statistics
import subprocess
from pathlib import Path


def metrics(report):
    result = {}
    for key, value in report["measurements"].items():
        if isinstance(value, (int, float)):
            result[key] = value
        elif isinstance(value, dict) and "us" in value:
            result[key + "_us"] = value["us"]
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--runs", type=int, default=10)
    args = parser.parse_args()
    if not 0 <= args.warmups <= 100 or not 1 <= args.runs <= 1000:
        parser.error("Expected 0..100 warm-ups and 1..1000 measured runs")
    if args.output.exists():
        parser.error(
            "Output already exists; use a new path to preserve previous evidence"
        )
    binary = args.binary.resolve(strict=True)
    rows = []
    for index in range(args.warmups + args.runs):
        command = [str(binary), "benchmark", "--json"]
        result = subprocess.run(
            command, check=False, capture_output=True, text=True, timeout=120
        )
        report = json.loads(result.stdout)
        if report.get("schema_version") != 1 or report.get("gates_passed") != (
            result.returncode == 0
        ):
            raise ValueError("Benchmark schema or gate/exit agreement failed")
        rows.append(
            {
                "warmup": index < args.warmups,
                "command": command,
                "exit": result.returncode,
                "stderr": result.stderr,
                "report": report,
            }
        )
    measured = [row for row in rows if not row["warmup"]]
    identity_fields = ["version", "build", "fixtures", "host", "slo_scale"]
    reference = {key: rows[0]["report"].get(key) for key in identity_fields}
    mismatches = [
        index
        for index, row in enumerate(rows)
        if {key: row["report"].get(key) for key in identity_fields} != reference
    ]
    samples = [metrics(row["report"]) for row in measured]
    summary = {}
    for key in samples[0] if not mismatches else []:
        values = sorted(sample[key] for sample in samples)
        summary[key] = {
            "min": values[0],
            "p50": statistics.median(values),
            "p95": values[math.ceil(0.95 * len(values)) - 1],
            "max": values[-1],
        }
    output = {
        "schema_version": 1,
        "observed_utc": datetime.datetime.now(datetime.UTC).isoformat(),
        "warmups": args.warmups,
        "measured_runs": args.runs,
        "passed_runs": sum(row["exit"] == 0 for row in measured),
        "consistent_provenance": not mismatches,
        "provenance_mismatch_runs": mismatches,
        "scope": "independent processes; component timing only; host contention uncontrolled",
        "summary": summary,
        "runs": rows,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("x") as stream:
        json.dump(output, stream, indent=2)
        stream.write("\n")
    print(
        f"{output['passed_runs']}/{args.runs} measured runs passed; evidence: {args.output}"
    )
    return int(bool(mismatches) or any(row["exit"] != 0 for row in measured))


if __name__ == "__main__":
    raise SystemExit(main())
