#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 DoubleGate
# SPDX-License-Identifier: GPL-3.0-or-later
"""Exercise the real series runner with an offline benchmark-process fixture."""

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


class SeriesTests(unittest.TestCase):
    def run_series(self, folder, changed=False):
        binary = folder / "benchmark"
        binary.write_text(
            f"#!{sys.executable}\n"
            + """
import json
from pathlib import Path
counter = Path(__file__).with_name('counter')
number = int(counter.read_text()) if counter.exists() else 0
counter.write_text(str(number + 1))
report = {'schema_version':1, 'version':'1.2.0', 'gates_passed':True,
          'build':{'rustflags':FLAGS}, 'fixtures':{'hdc_blake3':'fixture'},
          'measurements':{'bind_permute_ns':number}}
print(json.dumps(report))
""".replace("FLAGS", "str(number)" if changed else "'same'")
        )
        binary.chmod(0o755)
        output = folder / "results.json"
        result = subprocess.run(
            [
                sys.executable,
                str(Path(__file__).with_name("benchmark_series.py")),
                "--binary",
                str(binary),
                "--output",
                str(output),
            ],
            check=False,
            capture_output=True,
            text=True,
            timeout=15,
        )
        return result, json.loads(output.read_text())

    def test_warmup_is_excluded_from_ten_independent_run_summary(self):
        root = Path("/tmp/Infinity-Context/v120/series-tests")
        root.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=root) as tmp:
            result, report = self.run_series(Path(tmp))
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(len(report["runs"]), 11)
            self.assertEqual(report["passed_runs"], 10)
            self.assertEqual(
                report["summary"]["bind_permute_ns"],
                {"min": 1, "p50": 5.5, "p95": 10, "max": 10},
            )

    def test_changed_build_provenance_refuses_mixed_summary(self):
        root = Path("/tmp/Infinity-Context/v120/series-tests")
        root.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=root) as tmp:
            result, report = self.run_series(Path(tmp), changed=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(report["consistent_provenance"])
            self.assertEqual(report["summary"], {})
            self.assertEqual(
                len(report["runs"]), 11, "retain raw evidence even on refusal"
            )


if __name__ == "__main__":
    unittest.main()
