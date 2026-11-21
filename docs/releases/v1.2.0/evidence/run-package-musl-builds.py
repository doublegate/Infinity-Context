#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 DoubleGate
# SPDX-License-Identifier: GPL-3.0-or-later
"""Build the already-frozen preliminary package snapshot using default Rust linkage."""
import json
import os
from pathlib import Path
import subprocess
import time

HERE = Path(__file__).resolve().parent
BASE = Path('/tmp/Infinity-Context/v120')
source = Path(json.loads((HERE / 'package-rebuild-source-manifest.json').read_text())['source'])
processes = []
for label, cpu in [('baseline', 'x86-64'), ('v3', 'x86-64-v3')]:
    env = os.environ.copy()
    env.pop('CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER', None)
    env.pop('CARGO_ENCODED_RUSTFLAGS', None)
    env.update({'CARGO_TARGET_DIR': str(BASE / f'package-musl-{label}'), 'RUSTFLAGS': f'-C target-cpu={cpu}',
                'CC_x86_64_unknown_linux_musl': 'musl-gcc', 'CARGO_BUILD_JOBS': '2'})
    log = HERE / f'package-musl-default-{label}-build.log'
    stream = log.open('w')
    argv = ['cargo', 'build', '--release', '--locked', '-p', 'icx-cli', '--target', 'x86_64-unknown-linux-musl']
    process = subprocess.Popen(argv, cwd=source, env=env, stdout=stream, stderr=subprocess.STDOUT)
    processes.append((process, stream, time.monotonic(), {'label': label, 'argv': argv, 'source': str(source),
                      'rustflags': env['RUSTFLAGS'], 'rust_linker_override': 'unset; default self-contained linker',
                      'c_compiler': 'musl-gcc', 'log': log.name, 'preliminary': True}))
rows = []
for process, stream, start, row in processes:
    row['exit'] = process.wait()
    row['elapsed_seconds'] = round(time.monotonic() - start, 3)
    stream.close()
    rows.append(row)
(HERE / 'package-default-musl-build-results.json').write_text(json.dumps(rows, indent=2) + '\n')
print(json.dumps(rows))
raise SystemExit(any(row['exit'] != 0 for row in rows))
