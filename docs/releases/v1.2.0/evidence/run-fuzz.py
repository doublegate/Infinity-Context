#!/usr/bin/env python3
"""Run existing fuzz targets with isolated corpus/builds; preserve exact exits and times."""
import concurrent.futures
import datetime
import json
import os
from pathlib import Path
import subprocess
import time

root = Path(__file__).resolve().parents[4]
out = Path(__file__).resolve().parent
base = Path('/tmp/Infinity-Context/v120/fuzz')
env = os.environ.copy()
env.update(CARGO_NET_OFFLINE='true', TMPDIR='/tmp/Infinity-Context/v120',
           CARGO_TARGET_DIR='/tmp/Infinity-Context/validation/fuzz/ahtl_stream/target')


def run(target):
    folder = base / target
    (folder / 'corpus').mkdir(parents=True, exist_ok=True)
    (folder / 'artifacts').mkdir(exist_ok=True)
    command = ['cargo', '+nightly', 'fuzz', 'run', target, str(folder / 'corpus'), '--',
               '-max_total_time=900', '-rss_limit_mb=2048',
               '-artifact_prefix=' + str(folder / 'artifacts') + '/']
    start = time.monotonic()
    with (out / ('fuzz-' + target + '.log')).open('w') as log:
        result = subprocess.run(command, cwd=root / 'fuzz', env=env,
                                stdout=log, stderr=subprocess.STDOUT)
    row = {'target': target, 'command': command, 'exit': result.returncode,
           'outer_seconds_including_build': round(time.monotonic() - start, 2)}
    print(json.dumps(row), flush=True)
    return row


with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
    rows = list(pool.map(run, ['ahtl_stream', 'hdc_snapshot', 'mcp_jsonrpc', 'redact']))
(out / 'fuzz-results.json').write_text(json.dumps({
    'completed_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'sanitizer_policy': 'cargo-fuzz defaults; no leak-detection override; host socket/ptrace permission',
    'runs': rows}, indent=2) + '\n')
