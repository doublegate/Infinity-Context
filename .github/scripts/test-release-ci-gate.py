#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 DoubleGate
# SPDX-License-Identifier: GPL-3.0-or-later
"""Offline contract tests: only the external GitHub API transport is replaced."""

import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("release-ci-gate.py")
SHA = "a" * 40
REPO = "owner/project"
JOBS = {
    "ci.yml": [
        "Format, Clippy, Test, Docs, Benchmark & Self-Tests",
        "Dependency Advisories, Licenses & Bans (cargo-deny)",
        "Code Coverage (Codecov)",
        "Secret Scan (gitleaks)",
    ],
    "reuse.yml": ["REUSE 3.3 Compliance"],
}


def fixtures():
    result = {}
    for index, (workflow, names) in enumerate(JOBS.items(), 1):
        run = {
            "id": index,
            "run_attempt": 2,
            "head_sha": SHA,
            "head_branch": "main",
            "event": "push",
            "head_repository": {"full_name": REPO},
            "path": ".github/workflows/" + workflow,
            "status": "completed",
            "conclusion": "success",
        }
        result[workflow] = run
        result[index] = [
            {
                "name": name,
                "head_sha": SHA,
                "run_id": index,
                "status": "completed",
                "conclusion": "success",
            }
            for name in names
        ]
    return result


class GateTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        spec = importlib.util.spec_from_file_location("release_ci_gate", SCRIPT)
        cls.gate = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.gate)

    def check(self, data):
        calls = []

        def api(endpoint):
            calls.append(endpoint)
            if "/workflows/" in endpoint:
                workflow = endpoint.split("/workflows/")[1].split("/")[0]
                return [{"workflow_runs": [data[workflow]] if data[workflow] else []}]
            run_id = int(endpoint.split("/runs/")[1].split("/")[0])
            return [{"jobs": data[run_id]}]

        result = self.gate.check_once(REPO, SHA, api)
        return result, calls

    def test_exact_green_commit_passes(self):
        result, calls = self.check(fixtures())
        self.assertEqual(result, (True, "All required workflows and jobs succeeded"))
        self.assertTrue(any("head_sha=" + SHA in call for call in calls))
        self.assertTrue(any("/attempts/2/jobs" in call for call in calls))

    def test_missing_run_waits_instead_of_passing(self):
        data = fixtures()
        data["reuse.yml"] = None
        self.assertEqual(self.check(data)[0][0], False)

    def test_failed_or_skipped_required_job_blocks_even_green_run(self):
        for conclusion in [
            "failure",
            "cancelled",
            "timed_out",
            "skipped",
            "neutral",
            None,
        ]:
            with self.subTest(conclusion=conclusion):
                data = fixtures()
                data[1][2]["conclusion"] = conclusion
                with self.assertRaises(self.gate.GateError):
                    self.check(data)

    def test_missing_required_job_blocks(self):
        data = fixtures()
        data[1].pop()
        with self.assertRaises(self.gate.GateError):
            self.check(data)

    def test_pending_run_waits_but_failed_run_blocks(self):
        data = fixtures()
        data["ci.yml"].update(status="in_progress", conclusion=None)
        self.assertEqual(self.check(data)[0][0], False)
        for conclusion in ["failure", "cancelled", "timed_out", "skipped", "neutral"]:
            data["ci.yml"].update(status="completed", conclusion=conclusion)
            with self.assertRaises(self.gate.GateError):
                self.check(data)

    def test_wrong_sha_repository_workflow_or_event_is_rejected(self):
        changes = {
            "head_sha": "b" * 40,
            "head_branch": "feature",
            "event": "pull_request",
            "path": ".github/workflows/spoof.yml",
            "head_repository": {"full_name": "fork/project"},
        }
        for field, value in changes.items():
            with self.subTest(field=field):
                data = fixtures()
                data["ci.yml"][field] = value
                with self.assertRaises(self.gate.GateError):
                    self.check(data)

    def test_latest_run_failure_cannot_use_older_success(self):
        data = fixtures()
        old = dict(data["ci.yml"], id=1)
        new = dict(old, id=99, conclusion="failure")

        def api(endpoint):
            return [{"workflow_runs": [old, new]}]

        with self.assertRaises(self.gate.GateError):
            self.gate.check_once(REPO, SHA, api)

    def test_wrong_sha_or_duplicate_job_is_rejected(self):
        for duplicate in [False, True]:
            data = fixtures()
            if duplicate:
                data[1].append(dict(data[1][0]))
            else:
                data[1][0]["head_sha"] = "b" * 40
            with self.assertRaises(self.gate.GateError):
                self.check(data)

    def test_pending_deadline_fails_closed(self):
        with self.assertRaises(self.gate.GateError):
            self.gate.wait_for_green(REPO, SHA, 0, 1, lambda _: [{"workflow_runs": []}])

    def test_cli_uses_read_only_api_and_fails_closed_on_transport_or_timeout(self):
        with tempfile.TemporaryDirectory(prefix="icx-release-tests-") as tmp:
            folder = Path(tmp)
            (folder / "fixtures.json").write_text(json.dumps(fixtures()))
            (folder / "gh").write_text(
                f"#!{sys.executable}\n"
                + """
import json, os, sys
from pathlib import Path
assert '--method' in sys.argv and sys.argv[sys.argv.index('--method') + 1] == 'GET'
assert '--paginate' in sys.argv and '--slurp' in sys.argv
mode = os.environ['GATE_TEST_MODE']
if mode == 'outage':
    sys.exit(1)
data = json.loads(Path(__file__).with_name('fixtures.json').read_text())
endpoint = sys.argv[-1]
if '/workflows/' in endpoint:
    name = endpoint.split('/workflows/')[1].split('/')[0]
    print(json.dumps([{'workflow_runs': [] if mode == 'pending' else [data[name]]}]))
else:
    run_id = endpoint.split('/runs/')[1].split('/')[0]
    jobs = data[run_id]
    print(json.dumps([{'jobs': jobs[:1]}, {'jobs': jobs[1:]}]))
"""
            )
            (folder / "gh").chmod(0o755)
            for mode, want in [("green", 0), ("outage", 1), ("pending", 1)]:
                with self.subTest(mode=mode):
                    env = dict(
                        os.environ,
                        PATH=str(folder) + os.pathsep + os.environ["PATH"],
                        GATE_TEST_MODE=mode,
                    )
                    result = subprocess.run(
                        [
                            sys.executable,
                            str(SCRIPT),
                            "--repo",
                            REPO,
                            "--sha",
                            SHA,
                            "--timeout",
                            "0",
                        ],
                        check=False,
                        env=env,
                        capture_output=True,
                        text=True,
                        timeout=10,
                    )
                    self.assertEqual(result.returncode, want, result.stderr)
                    if mode == "outage":
                        self.assertIn("GitHub API unavailable", result.stderr)
                    elif mode == "pending":
                        self.assertIn("Timed out", result.stderr)


if __name__ == "__main__":
    unittest.main()
