#!/usr/bin/env python3
"""Turn captured agent hook payloads into committed contract fixtures (T-65-001).

Usage:
    scripts/sanitize-hook-fixtures.py --agent agy --version 1.5.14 \
        --logs <capture dir> --project <scratch project root> [--out <fixtures root>]

The capture directory holds `<Event>-<n>.in.json` files written by a capture hook (each payload
an agent sent). For each event the first capture is written to
`<out>/<agent>/<version>/<Event>.json`, and further captures of the same event as
`<Event>-2.json`, ... in capture order.

Sanitizing, applied to every string in the payload:
- the scratch project root becomes `/home/user/project`, and `$HOME` becomes `/home/user`;
- UUIDs become `00000000-0000-4000-8000-00000000000N`, numbered in order of first appearance;
- e-mail addresses become `user@example.invalid`.

The output is written with sorted keys and checked afterwards: the run fails if the real home
directory, user name or project root survives anywhere. Run gitleaks on the result before
committing it.
"""

import argparse
import json
import os
import re
import sys
from pathlib import Path

UUID = re.compile(
    r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}"
)
EMAIL = re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}")


class Sanitizer:
    def __init__(self, project: str, home: str):
        # Both the given and the resolved form of each path, longest first, so a symlinked
        # home or project is replaced however a payload spells it.
        self.paths: list[tuple[str, str]] = sorted(
            {
                (p.rstrip("/"), stand_in)
                for given, stand_in in (
                    (project, "/home/user/project"),
                    (home, "/home/user"),
                )
                for p in (given, os.path.realpath(given))
                if p.rstrip("/")
            },
            key=lambda t: -len(t[0]),
        )
        self.uuids: dict[str, str] = {}

    def _uuid(self, m: re.Match) -> str:
        key = m.group(0).lower()
        if key not in self.uuids:
            self.uuids[key] = f"00000000-0000-4000-8000-{len(self.uuids) + 1:012d}"
        return self.uuids[key]

    def text(self, s: str) -> str:
        for real, stand_in in self.paths:
            s = s.replace(real, stand_in)
        s = UUID.sub(self._uuid, s)
        return EMAIL.sub("user@example.invalid", s)

    def value(self, v):
        if isinstance(v, str):
            return self.text(v)
        if isinstance(v, list):
            return [self.value(x) for x in v]
        if isinstance(v, dict):
            return {self.text(k): self.value(x) for k, x in v.items()}
        return v


def main() -> int:
    ap = argparse.ArgumentParser(description=(__doc__ or "").strip().splitlines()[0])
    ap.add_argument("--agent", required=True)
    ap.add_argument("--version", required=True)
    ap.add_argument("--logs", required=True, type=Path)
    ap.add_argument("--project", required=True)
    ap.add_argument(
        "--out", type=Path, default=Path("crates/icx-adapters/tests/fixtures")
    )
    args = ap.parse_args()

    home = os.path.expanduser("~")
    user = os.path.basename(home)
    sanitizer = Sanitizer(args.project, home)
    dest = args.out / args.agent / args.version

    captures = sorted(
        args.logs.glob("*.in.json"),
        key=lambda p: int(p.stem.split("-")[-1].split(".")[0]),
    )
    counts: dict[str, int] = {}
    outputs: list[tuple[Path, str]] = []
    for path in captures:
        event = path.name.rsplit("-", 1)[0]
        try:
            payload = json.loads(path.read_text())
        except json.JSONDecodeError as e:
            print(f"skip {path.name}: {e}", file=sys.stderr)
            continue
        counts[event] = counts.get(event, 0) + 1
        name = event if counts[event] == 1 else f"{event}-{counts[event]}"
        text = json.dumps(sanitizer.value(payload), indent=2, sort_keys=True) + "\n"
        outputs.append((dest / f"{name}.json", text))

    # Checked before anything is written, so a failed run leaves no leaking file behind.
    real = {home, os.path.realpath(home), args.project, os.path.realpath(args.project)}
    leaks = [
        str(path)
        for path, text in outputs
        if any(s in text for s in (*real, f"/{user}/"))
    ]
    if leaks:
        print("real paths survived sanitizing in: " + ", ".join(leaks), file=sys.stderr)
        return 1
    dest.mkdir(parents=True, exist_ok=True)
    for path, text in outputs:
        path.write_text(text)
        print(path)
    return 0


if __name__ == "__main__":
    sys.exit(main())
