#!/usr/bin/env python3
"""Verify semantic survivor families against new release oracles in a frozen tiny copy."""

import argparse
import concurrent.futures
import hashlib
import json
import os
import re
import shutil
import subprocess
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--campaign", default="mutation-critical")
parser.add_argument("--prefix", default="mutation-critical")
parser.add_argument("--selector", help="Override the semantic mutation regex for a bounded recheck")
parser.add_argument(
    "--crates",
    nargs="+",
    choices=["core", "hdc", "bus", "codec"],
    default=["core", "hdc", "bus", "codec"],
)
args = parser.parse_args()
if not all(re.fullmatch(r"[a-z0-9-]+", name) for name in [args.campaign, args.prefix]):
    parser.error("Use lowercase letters, digits and hyphens for campaign/prefix")
prefix = args.prefix
root = Path(__file__).resolve().parents[4]
out = Path(__file__).resolve().parent
scratch = Path("/tmp/Infinity-Context/v120") / args.campaign
source = scratch / "source"
if (
    source.exists()
    or any(out.glob(f"{prefix}-*.json"))
    or any(out.glob(f"{prefix}-*.log"))
):
    raise SystemExit("Preserve existing critical campaign; choose a new source/prefix")
source.mkdir(parents=True)
for name in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]:
    shutil.copy2(root / name, source / name)
shutil.copytree(
    root / "crates",
    source / "crates",
    ignore=shutil.ignore_patterns("target", "__pycache__"),
)
manifest = {
    str(p.relative_to(source)): hashlib.sha256(p.read_bytes()).hexdigest()
    for p in source.rglob("*")
    if p.is_file()
}
(out / f"{prefix}-source-manifest.json").write_text(
    json.dumps(manifest, indent=2) + "\n"
)
patterns = {
    "core": r"replace <= with > in CacheEpochController::assemble_window|replace \|\| with && in CasStore::from_parts|replace \|\| with && in MerkleContextDag::verify_merkle_integrity_with_ancestors|replace match guard .* with false in MerkleContextDag::find_turn$",
    "hdc": r"replace >> with << in Hypervector::from_u64_seed|replace \^= with &= in Hypervector::sketch_256|replace \* with / in FractalCoord::morton_z64|replace \^ with & in FractalCoord::digest_u32|replace - with / in TitansSurpriseGate::observe_with_exit_code",
    "bus": r"replace >> with << in unpack_meta|replace \* with \+ in SeqlockTelemetryBus::poll_since",
    "codec": r"replace \|\| with && in AhtlPacket::to_profile_a|replace \|\| with && in AhtlWirePacketV1::parse|replace \|\| with && in EpochHeader::parse",
}
env = dict(
    os.environ, CARGO_NET_OFFLINE="true", CARGO_BUILD_JOBS="2", TMPDIR=str(scratch)
)
env.pop("CARGO_TARGET_DIR", None)


def run(crate):
    common = ["cargo", "mutants", "-d", str(source), "-p", "icx-" + crate]
    inv = subprocess.run(
        common + ["--list", "--json"],
        env=env,
        check=True,
        capture_output=True,
        text=True,
    )
    (out / f"{prefix}-{crate}-inventory.json").write_text(inv.stdout)
    command = common + [
        "--re",
        args.selector or patterns[crate],
        "--timeout",
        "90",
        "--build-timeout",
        "240",
        "-j",
        "1",
        "--output",
        str(scratch / crate),
    ]
    with (out / f"{prefix}-{crate}.log").open("x") as log:
        result = subprocess.run(
            command, env=env, stdout=log, stderr=subprocess.STDOUT, check=False
        )
    folder = scratch / crate / "mutants.out"
    outcomes = json.loads((folder / "outcomes.json").read_text())
    selected = json.loads((folder / "mutants.json").read_text())
    (out / f"{prefix}-{crate}-outcomes.json").write_text(
        json.dumps(outcomes, indent=2) + "\n"
    )
    rows = [
        {"name": r["scenario"]["Mutant"]["name"], "summary": r["summary"]}
        for r in outcomes["outcomes"]
        if "Mutant" in r.get("scenario", {})
    ]
    report = {
        "crate": "icx-" + crate,
        "full_inventory": len(json.loads(inv.stdout)),
        "selected": len(selected),
        "outcomes": rows,
        "exit": result.returncode,
        "command": command,
    }
    print(json.dumps(report), flush=True)
    return report


with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
    rows = list(pool.map(run, args.crates))
(out / f"{prefix}-summary.json").write_text(
    json.dumps(
        {
            "scope": "Directed semantic families, potentially overlapping prior samples; not an additional unique coverage count",
            "rows": rows,
        },
        indent=2,
    )
    + "\n"
)
