#!/usr/bin/env python3
"""Bounded all-crate samples in a tiny frozen source copy; never mutate the checkout."""

import argparse
import concurrent.futures
import datetime
import hashlib
import json
import os
import re
import shutil
import subprocess
import time
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--campaign", default="mutation-campaign")
parser.add_argument("--prefix", default="mutation")
parser.add_argument(
    "--crates",
    nargs="+",
    choices=["core", "codec", "hdc", "store", "bus", "adapters", "proxy", "cli"],
    default=["core", "codec", "hdc", "store", "bus", "adapters", "proxy", "cli"],
)
args = parser.parse_args()
if not all(re.fullmatch(r"[a-z0-9-]+", name) for name in [args.campaign, args.prefix]):
    parser.error(
        "Campaign and evidence prefix must contain only lowercase letters, digits or hyphens"
    )
prefix = args.prefix
root = Path(__file__).resolve().parents[4]
out = Path(__file__).resolve().parent
scratch = Path("/tmp/Infinity-Context/v120") / args.campaign
source = scratch / "source"
if any(out.glob(f"{prefix}-*.json")) or any(out.glob(f"{prefix}-*.log")):
    raise SystemExit("Evidence prefix exists; choose a new prefix to preserve previous results")
if source.exists():
    raise SystemExit("Snapshot exists; preserve it and choose a new campaign path")
source.mkdir(parents=True)
for name in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]:
    shutil.copy2(root / name, source / name)
shutil.copytree(
    root / "crates",
    source / "crates",
    ignore=shutil.ignore_patterns("target", "__pycache__"),
)
manifest = {
    str(path.relative_to(source)): hashlib.sha256(path.read_bytes()).hexdigest()
    for path in source.rglob("*")
    if path.is_file()
}
(out / f"{prefix}-source-manifest.json").write_text(
    json.dumps(manifest, indent=2) + "\n"
)
env = dict(
    os.environ, CARGO_NET_OFFLINE="true", CARGO_BUILD_JOBS="2", TMPDIR=str(scratch)
)
env.pop("CARGO_TARGET_DIR", None)
crates = args.crates


def run(crate):
    name = "icx-" + crate
    common = ["cargo", "mutants", "-d", str(source), "-p", name]
    inventory_command = common + ["--list", "--json"]
    inventory = subprocess.run(
        inventory_command,
        env=env,
        check=True,
        capture_output=True,
        text=True,
        timeout=120,
    )
    (out / f"{prefix}-{name}-inventory.json").write_text(inventory.stdout)
    total = len(json.loads(inventory.stdout))
    output = scratch / name
    command = common + [
        "--shard",
        "1/40",
        "--sharding",
        "round-robin",
        "--timeout",
        "90",
        "--build-timeout",
        "240",
        "-j",
        "1",
        "--output",
        str(output),
    ]
    start = time.monotonic()
    with (out / f"{prefix}-{name}.log").open("w") as log:
        result = subprocess.run(
            command, env=env, check=False, stdout=log, stderr=subprocess.STDOUT
        )
    folder = output / "mutants.out"
    outcomes = json.loads((folder / "outcomes.json").read_text())
    selected = json.loads((folder / "mutants.json").read_text())
    counts = {}
    mutated = []
    for row in outcomes["outcomes"]:
        if "Mutant" in row.get("scenario", {}):
            counts[row["summary"]] = counts.get(row["summary"], 0) + 1
            mutated.append(row)
    (out / f"{prefix}-{name}-outcomes.json").write_text(
        json.dumps(outcomes, indent=2) + "\n"
    )
    for row in mutated:
        if row["summary"] in ["MissedMutant", "Timeout", "Unviable"]:
            src = folder / row["log_path"]
            if src.exists():
                dest = out / f"{prefix}-{name}-{Path(row['log_path']).name}"
                shutil.copy2(src, dest)
    report = {
        "crate": name,
        "full_inventory_count": total,
        "selected": len(selected),
        "completed_mutants": len(mutated),
        "counts": counts,
        "command": command,
        "inventory_command": inventory_command,
        "exit": result.returncode,
        "seconds": round(time.monotonic() - start, 2),
    }
    print(json.dumps(report), flush=True)
    return report


with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
    rows = list(pool.map(run, crates))
(out / f"{prefix}-summary.json").write_text(
    json.dumps(
        {
            "observed_utc": datetime.datetime.now(datetime.UTC).isoformat(),
            "scope": "round-robin 1/40 per crate, full inventories recorded; full space NOT exhausted",
            "source_files": len(manifest),
            "rows": rows,
        },
        indent=2,
    )
    + "\n"
)
