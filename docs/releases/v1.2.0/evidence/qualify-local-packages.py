#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 DoubleGate
# SPDX-License-Identifier: GPL-3.0-or-later
"""Inspect existing binaries and smoke/package them locally; never build or certify a release.

All writes except the explicitly chosen JSON report stay below --work-dir (/tmp by default).
A candidate that fails linkage, CPU provenance or smoke is never packaged. Existing binaries
are preliminary: this script cannot bind them to a final source commit or attest CI success.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[4]
LABELS = {
    "x86_64-unknown-linux-musl", "x86_64_v3-unknown-linux-musl",
    "x86_64-unknown-linux-gnu", "x86_64_v3-unknown-linux-gnu",
}


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def run(argv, *, cwd=None, env=None, timeout=30):
    try:
        process = subprocess.run(argv, cwd=cwd, env=env, text=True, capture_output=True, timeout=timeout, check=False)
        return {"argv": list(map(str, argv)), "exit": process.returncode,
                "stdout": process.stdout, "stderr": process.stderr}
    except (OSError, subprocess.TimeoutExpired) as error:
        return {"argv": list(map(str, argv)), "exit": None, "error": str(error)}


def smoke(binary, directory):
    directory.mkdir(mode=0o700)
    for name in ("home", "proj", "run"):
        (directory / name).mkdir(mode=0o700)
    # No inherited credentials, user-specific ICX paths, agent settings or daemon invocation.
    env = {"PATH": "/usr/bin:/bin", "LANG": "C.UTF-8", "HOME": str(directory / "home"),
           "XDG_STATE_HOME": str(directory / "home/.local/state"),
           "XDG_CONFIG_HOME": str(directory / "home/.config"),
           "XDG_DATA_HOME": str(directory / "home/.local/share"),
           "XDG_CACHE_HOME": str(directory / "home/.cache"),
           "XDG_RUNTIME_DIR": str(directory / "run")}
    return [run([str(binary), command], cwd=directory / "proj", env=env, timeout=20)
            for command in ("doctor", "init", "status")]


def package(binary, label, notices, musl_notice, directory, epoch):
    """Workflow-compatible layout, normalized modes/mtime/owner and gzip -n headers."""
    name = f"icx-v1.2.0-{label}"
    staging = directory / "staging" / name
    staging.mkdir(parents=True)
    (staging / "LICENSES").mkdir()
    shutil.copyfile(binary, staging / "icx")
    (staging / "icx").chmod(0o755)
    stripped = run(["strip", str(staging / "icx")])
    if stripped["exit"] != 0:
        raise RuntimeError(stripped)
    for source in [ROOT / "LICENSE", ROOT / "COPYING", ROOT / "README.md", ROOT / "CHANGELOG.md", notices]:
        shutil.copyfile(source, staging / ("THIRD-PARTY-NOTICES.txt" if source == notices else source.name))
    for name_ in ("GPL-3.0-or-later.txt", "AGPL-3.0-or-later.txt"):
        shutil.copyfile(ROOT / "LICENSES" / name_, staging / "LICENSES" / name_)
    if label.endswith("-musl"):
        shutil.copyfile(musl_notice, staging / "MUSL-COPYRIGHT.txt")
    for path in staging.rglob("*"):
        path.chmod(0o755 if path.is_dir() or path.name == "icx" else 0o644)
    staging.chmod(0o755)
    archives = []
    for suffix in ("", ".recheck"):
        archive = directory / f"{name}{suffix}.tar.gz"
        tar = subprocess.run(["tar", "-C", str(staging.parent), "--owner=0", "--group=0", "--numeric-owner",
                              "--sort=name", f"--mtime=@{epoch}", "-cf", "-", name], capture_output=True, check=True)
        compressed = subprocess.run(["gzip", "-n"], input=tar.stdout, capture_output=True, check=True)
        archive.write_bytes(compressed.stdout)
        archives.append(archive)
    if digest(archives[0]) != digest(archives[1]):
        raise RuntimeError("archive bytes differ across repeated packaging")
    listing = run(["tar", "tvzf", str(archives[0])])
    return {"archive": str(archives[0]), "sha256": digest(archives[0]),
            "binary_sha256_after_strip": digest(staging / "icx"),
            "repeat_identical": True, "listing": listing,
            "input_sha256": {str(p.relative_to(staging)): digest(p) for p in sorted(staging.rglob("*")) if p.is_file()}}


def inspect(candidate, work, notices, musl_notice, epoch):
    label, binary = candidate["label"], Path(candidate["binary"]).resolve()
    if label not in LABELS:
        raise ValueError(f"unsupported advertised label: {label}")
    result = {"label": label, "binary": str(binary), "preliminary": True,
              "final_source_binding": "not established", "checks": {}}
    if not binary.is_file():
        result["error"] = "binary absent; build not attempted"
        return result
    result["binary_sha256"] = digest(binary)
    result["binary_mtime_ns"] = binary.stat().st_mtime_ns
    result["file"] = run(["file", str(binary)])
    result["program_headers"] = run(["readelf", "-l", str(binary)])
    result["dynamic"] = run(["readelf", "-d", str(binary)])
    flags = candidate.get("build_flags", "")
    provenance = candidate.get("build_provenance")
    if provenance:
        provenance = Path(provenance)
        result["build_provenance_sha256"] = digest(provenance)
        result["build_provenance"] = provenance.read_text()
        match = re.search(r"ICX_BUILD_RUSTFLAGS=(.*)", result["build_provenance"])
        if match:
            flags = match[1]
    result["recorded_build_flags"] = flags
    cpu_match = re.search(r"target-cpu(?:=|\s+)(\S+)", flags)
    cpu = cpu_match[1] if cpu_match else "x86-64"
    result["checks"]["cpu_provenance"] = bool(provenance) and cpu == ("x86-64-v3" if label.startswith("x86_64_v3-") else "x86-64")
    if label.endswith("-musl"):
        result["checks"]["static_linkage"] = (result["program_headers"]["exit"] == 0 and result["dynamic"]["exit"] == 0
                and "INTERP" not in result["program_headers"].get("stdout", "")
                and "(NEEDED)" not in result["dynamic"].get("stdout", "")
                and bool(re.search(r"static(?:ally|-pie) linked", result["file"].get("stdout", ""))))
        result["checks"]["musl_notice"] = musl_notice.is_file() and "Permission is hereby granted" in musl_notice.read_text()
    else:
        result["symbols"] = run(["objdump", "-T", str(binary)])
        versions = set(re.findall(r"GLIBC_([0-9.]+)", result["symbols"].get("stdout", "")))
        maximum = max(versions, key=lambda v: tuple(map(int, v.split(".")))) if versions else None
        result["highest_glibc_symbol"] = maximum
        result["checks"]["glibc_at_most_2_35"] = maximum is not None and tuple(map(int, maximum.split("."))) <= (2, 35)
        result["genuine_glibc_2_35_runtime_smoke"] = "not performed; host smoke does not establish this"
    cpuinfo = Path("/proc/cpuinfo").read_text()
    # Workflow checks avx2; x86-64-v3 also needs the other specified v3 ISA features.
    required = {"avx", "avx2", "bmi1", "bmi2", "f16c", "fma", "abm", "movbe", "xsave"}
    compatible = not label.startswith("x86_64_v3-") or required.issubset(set(cpuinfo.split()))
    result["checks"]["host_cpu_compatible"] = compatible
    if compatible:
        result["smoke"] = smoke(binary, work / label / "smoke")
        result["checks"]["isolated_smoke"] = all(p["exit"] == 0 for p in result["smoke"])
    else:
        result["smoke_skip"] = "host lacks required v3 features"
        result["checks"]["isolated_smoke"] = False
    result["checks"]["third_party_notices"] = notices.is_file() and "THIRD-PARTY NOTICES for icx" in notices.read_text() and "====" in notices.read_text()
    result["local_eligibility"] = all(result["checks"].values())
    if result["local_eligibility"]:
        result["package"] = package(binary, label, notices, musl_notice, work / label, epoch)
        packaged_binary = work / label / "staging" / f"icx-v1.2.0-{label}" / "icx"
        result["packaged_smoke"] = smoke(packaged_binary, work / label / "packaged-smoke")
        result["checks"]["stripped_package_smoke"] = all(p["exit"] == 0 for p in result["packaged_smoke"])
        result["local_eligibility"] = all(result["checks"].values())
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidates", type=Path, required=True)
    parser.add_argument("--notices", type=Path, required=True)
    parser.add_argument("--musl-notice", type=Path, default=Path("/usr/share/licenses/musl/COPYRIGHT"))
    parser.add_argument("--work-dir", type=Path, default=Path("/tmp/Infinity-Context/v120/package-qualification"))
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    args.work_dir.mkdir(parents=True, exist_ok=True)
    work = Path(tempfile.mkdtemp(prefix="preliminary-", dir=args.work_dir))
    epoch = int(subprocess.check_output(["git", "log", "-1", "--format=%ct"], cwd=ROOT, text=True).strip())
    report = {"scope": "preliminary existing-binary local checks only; no builds, install, daemon, tag or publication",
              "work_dir": str(work), "commit_epoch": epoch, "host_glibc": run(["ldd", "--version"]),
              "notices_sha256": digest(args.notices), "results": []}
    for candidate in json.loads(args.candidates.read_text()):
        (work / candidate["label"]).mkdir()
        report["results"].append(inspect(candidate, work, args.notices.resolve(), args.musl_notice.resolve(), epoch))
    report["final_release_certified"] = False
    report["counts"] = {"locally_eligible": sum(r.get("local_eligibility", False) for r in report["results"]),
                        "candidates": len(report["results"])}
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"report": str(args.report), "counts": report["counts"], "final_release_certified": False}))
    return 0 if report["counts"]["locally_eligible"] == len(report["results"]) else 1


if __name__ == "__main__":
    raise SystemExit(main())
