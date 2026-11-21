#!/usr/bin/env python3
"""Repeat the highest-impact retrieval row in a separate standalone executable."""
import json
import os
from pathlib import Path
import subprocess

root = Path.cwd()
tmp = Path('/tmp/Infinity-Context/retrieval-recheck')
(tmp / 'src').mkdir(parents=True, exist_ok=True)
source = (root / 'docs/codex/evidence/codec-probe.rs').read_text()
old = 'for kind in ["repetitive","varied"] {for seed in [1u64,17,97]{for n in [100usize,1000,10000]{'
assert source.count(old) == 1
source = source.replace(old, 'for kind in ["varied"] {for seed in [1u64]{for n in [10000usize]{')
(tmp / 'src/main.rs').write_text(source)
manifest = '[package]\nname="icx-retrieval-recheck"\nversion="0.0.0"\nedition="2024"\n[workspace]\n[dependencies]\n'
for crate in ['icx-core', 'icx-codec', 'icx-hdc', 'icx-store']:
    manifest += crate + '={path=' + json.dumps(str(root / 'crates' / crate)) + '}\n'
(tmp / 'Cargo.toml').write_text(manifest)
env = dict(os.environ, CARGO_TARGET_DIR='/tmp/Infinity-Context/codec-audit/target', RUSTFLAGS='', CARGO_NET_OFFLINE='true')
cmd = ['cargo', 'run', '--release', '--offline', '--manifest-path', str(tmp / 'Cargo.toml'), '--', 'recall']
p = subprocess.run(cmd, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
(root / 'docs/codex/evidence/retrieval-recheck.json').write_text(json.dumps({'command': cmd, 'exit': p.returncode, 'output': p.stdout}, indent=2) + '\n')
print(p.stdout)
raise SystemExit(p.returncode)
