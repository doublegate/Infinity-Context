#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 DoubleGate
# SPDX-License-Identifier: GPL-3.0-or-later
"""Require exact-commit, successful trusted CI before release packaging."""

import argparse
import json
import re
import subprocess
import sys
import time

REQUIRED = {
    "ci.yml": {
        "Format, Clippy, Test, Docs, Benchmark & Self-Tests",
        "Dependency Advisories, Licenses & Bans (cargo-deny)",
        "Code Coverage (Codecov)",
        "Secret Scan (gitleaks)",
    },
    "reuse.yml": {"REUSE 3.3 Compliance"},
}


class GateError(RuntimeError):
    """Release evidence is missing, invalid, unsuccessful or unavailable."""


def check_once(repo, sha, api):
    for workflow, required in REQUIRED.items():
        pages = api(
            f"repos/{repo}/actions/workflows/{workflow}/runs"
            f"?head_sha={sha}&branch=main&event=push&per_page=100"
        )
        runs = [run for page in pages for run in page["workflow_runs"]]
        if not runs:
            return False, f"Waiting for {workflow} push-to-main run at {sha}"
        # Never fall back to an older green run when the latest run failed or is pending.
        run = max(runs, key=lambda row: row["id"])
        if (
            run["head_sha"] != sha
            or run["head_branch"] != "main"
            or run["event"] != "push"
            or run["head_repository"]["full_name"].casefold() != repo.casefold()
            or run["path"] != ".github/workflows/" + workflow
        ):
            raise GateError(f"{workflow}: run provenance does not match release commit")
        if run["status"] != "completed":
            return False, f"Waiting for {workflow} run {run['id']}: {run['status']}"
        if run["conclusion"] != "success":
            raise GateError(f"{workflow}: latest run concluded {run['conclusion']}")
        run_id, attempt = run["id"], run["run_attempt"]
        pages = api(
            f"repos/{repo}/actions/runs/{run_id}/attempts/{attempt}/jobs?per_page=100"
        )
        jobs = [job for page in pages for job in page["jobs"]]
        names = [job["name"] for job in jobs]
        if len(names) != len(set(names)) or not required.issubset(names):
            raise GateError(f"{workflow}: missing or duplicate required jobs")
        # Also require success from additional jobs introduced by later CI policy changes.
        for job in jobs:
            if (
                job["head_sha"] != sha
                or job["run_id"] != run_id
                or job["status"] != "completed"
                or job["conclusion"] != "success"
            ):
                raise GateError(
                    f"{workflow}: job {job['name']} is not successful at {sha}"
                )
        print(f"PASS {workflow} run={run_id} attempt={attempt} sha={sha}", flush=True)
    return True, "All required workflows and jobs succeeded"


def wait_for_green(repo, sha, timeout, interval, api):
    deadline = time.monotonic() + timeout
    while True:
        passed, message = check_once(repo, sha, api)
        if passed:
            return
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise GateError(f"Timed out: {message}")
        print(message, flush=True)
        time.sleep(min(interval, remaining))


def github_api(endpoint):
    try:
        result = subprocess.run(
            [
                "gh",
                "api",
                "--method",
                "GET",
                "--paginate",
                "--slurp",
                "-H",
                "Accept: application/vnd.github+json",
                endpoint,
            ],
            check=True,
            capture_output=True,
            text=True,
            timeout=30,
        )
        return json.loads(result.stdout)
    except (OSError, subprocess.SubprocessError, ValueError) as exc:
        # Avoid printing external response bodies or inherited authentication details.
        raise GateError("GitHub API unavailable or returned invalid JSON") from exc


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", required=True)
    parser.add_argument("--sha", required=True)
    parser.add_argument("--timeout", type=int, default=600)
    parser.add_argument("--interval", type=int, default=15)
    args = parser.parse_args()
    if (
        not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", args.repo)
        or not re.fullmatch(r"[a-f0-9]{40}", args.sha)
        or not 0 <= args.timeout <= 1800
        or not 1 <= args.interval <= 60
    ):
        parser.error(
            "Expected owner/repo, full lowercase commit SHA, bounded timeout/interval"
        )
    try:
        wait_for_green(args.repo, args.sha, args.timeout, args.interval, github_api)
    except (GateError, KeyError, TypeError, ValueError) as exc:
        print(f"ERROR: release CI gate refused: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
