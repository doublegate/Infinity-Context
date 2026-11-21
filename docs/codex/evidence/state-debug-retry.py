#!/usr/bin/env python3
"""Retry only ENOSPC-contaminated mutants in a minimal isolated source snapshot."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

repo = Path.cwd()
out = repo / 'docs/codex/evidence'
base = Path('/tmp/Infinity-Context/state-debug-retry')
source = base / 'source'
source.mkdir(parents=True, exist_ok=False)
for name in ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml']:
    shutil.copy2(repo / name, source / name)
shutil.copytree(repo / 'crates', source / 'crates', ignore=shutil.ignore_patterns('target'))
env = os.environ.copy()
env.update(CARGO_NET_OFFLINE='true', CARGO_BUILD_JOBS='2', TMPDIR=str(base))
pattern = r'lib.rs:120:32: replace \|\| with &&|graph.rs:104:9: replace CsrGraphSnapshot::weights|graph.rs:145:9: replace CsrGraphSnapshot::run_ppr|ring.rs:280:30: replace >> with <<'
cmd = ['cargo', 'mutants', '-d', str(source), '-p', 'icx-core', '-p', 'icx-store', '-p', 'icx-bus',
       '--re', pattern, '--timeout', '60', '--build-timeout', '240', '-j', '1', '--gitignore', 'true',
       '--output', str(base / 'results')]
start = time.monotonic()
with (out / 'state-debug-retry.log').open('w') as log:
    result = subprocess.run(cmd, env=env, stdout=log, stderr=subprocess.STDOUT)
ledger = {'command': cmd, 'exit': result.returncode, 'elapsed_seconds': round(time.monotonic()-start, 3),
          'source_snapshot': 'Cargo.toml, Cargo.lock, rust-toolchain.toml and crates only; no ignored assets or fuzz targets',
          'purpose': 'Replace only original mutant outcomes whose per-mutant logs contain ENOSPC'}
outcomes = base / 'results/mutants.out/outcomes.json'
if outcomes.exists():
    ledger['outcomes'] = json.loads(outcomes.read_text())
(out / 'state-debug-retry-summary.json').write_text(json.dumps(ledger, indent=2)+'\n')
print(json.dumps({k:v for k,v in ledger.items() if k!='outcomes'}, indent=2))
