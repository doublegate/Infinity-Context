#!/usr/bin/env python3
"""Build a public-API probe outside the source tree; preserve exit status and output."""
from pathlib import Path
import json
import os
import subprocess

root = Path.cwd()
tmp = Path('/tmp/Infinity-Context/epoch-growth')
(tmp / 'src').mkdir(parents=True, exist_ok=True)
(tmp / 'src/main.rs').write_text((root / 'docs/codex/evidence/epoch-growth.rs').read_text())
(tmp / 'Cargo.toml').write_text('[package]\nname="icx-epoch-growth-audit"\nversion="0.0.0"\nedition="2024"\n[workspace]\n[dependencies]\nicx-core={path=' + json.dumps(str(root / 'crates/icx-core')) + '}\n')
env = dict(os.environ, CARGO_TARGET_DIR='/tmp/Infinity-Context/epoch-growth-target', CARGO_NET_OFFLINE='true', RUSTFLAGS='')
command = ['cargo', 'run', '--release', '--manifest-path', str(tmp / 'Cargo.toml')]
result = subprocess.run(command, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
log = {'command': command, 'exit': result.returncode, 'output': result.stdout, 'rustflags': ''}
(root / 'docs/codex/evidence/epoch-growth-result.json').write_text(json.dumps(log, indent=2) + '\n')
print(result.stdout)
raise SystemExit(result.returncode)
