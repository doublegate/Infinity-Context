#!/usr/bin/env python3
"""Repeat the frozen audit corpus against the implementation; never overwrite audit evidence."""
import hashlib
import json
import os
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[4]
out = Path(__file__).resolve().parent
tmp = Path('/tmp/Infinity-Context/v120/retrieval')
(tmp / 'src').mkdir(parents=True, exist_ok=True)
source = (root / 'docs/codex/evidence/codec-probe.rs').read_bytes()
(tmp / 'src/main.rs').write_bytes(source)
manifest = '[package]\nname="icx-v120-retrieval"\nversion="0.0.0"\nedition="2024"\n[workspace]\n[dependencies]\n'
for crate in ['icx-core', 'icx-codec', 'icx-hdc', 'icx-store']:
    manifest += crate + '={path=' + json.dumps(str(root / 'crates' / crate)) + '}\n'
(tmp / 'Cargo.toml').write_text(manifest)
env = dict(os.environ, CARGO_TARGET_DIR='/tmp/Infinity-Context/v120/retrieval-target',
           RUSTFLAGS='', CARGO_NET_OFFLINE='true')
build = ['cargo', 'build', '--release', '--offline', '--manifest-path', str(tmp / 'Cargo.toml')]
with (out / 'retrieval-build.log').open('w') as log:
    built = subprocess.run(build, env=env, stdout=log, stderr=subprocess.STDOUT)
rows = []
if built.returncode == 0:
    for mode, name in [('recall', 'retrieval'), ('multilingual', 'retrieval-multilingual')]:
        command = [env['CARGO_TARGET_DIR'] + '/release/icx-v120-retrieval', mode]
        with (out / (name + '.csv')).open('w') as stdout, (out / (name + '.log')).open('w') as stderr:
            result = subprocess.run(command, env=env, stdout=stdout, stderr=stderr)
        rows.append({'mode': mode, 'command': command, 'exit': result.returncode})
(out / 'retrieval-results.json').write_text(json.dumps({
    'source': 'docs/codex/evidence/codec-probe.rs',
    'source_sha256': hashlib.sha256(source).hexdigest(), 'build_command': build,
    'build_exit': built.returncode, 'runs': rows,
    'scope': 'frozen synthetic corpus; quality comparison valid; timings share host with fuzz/mutation'}, indent=2) + '\n')
raise SystemExit(built.returncode or any(row['exit'] for row in rows))
