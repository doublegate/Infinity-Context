#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 DoubleGate
# SPDX-License-Identifier: GPL-3.0-or-later
"""Execute reviewed offline container build plan; no pulls or user configuration mounts."""
import json
from pathlib import Path
import subprocess
import time

here = Path(__file__).resolve().parent
rows = []
for row in json.loads((here / 'package-bookworm-plan.json').read_text()):
    start = time.monotonic()
    log = here / f'package-gnu-bookworm-{row["label"]}-build.log'
    with log.open('w') as stream:
        process = subprocess.run(row['argv'], stdout=stream, stderr=subprocess.STDOUT, check=False)
    rows.append({**row, 'exit': process.returncode, 'log': log.name, 'seconds': round(time.monotonic() - start, 3)})
    (here / 'package-bookworm-build-results.json').write_text(json.dumps(rows, indent=2) + '\n')
print(json.dumps(rows))
raise SystemExit(any(row['exit'] for row in rows))
