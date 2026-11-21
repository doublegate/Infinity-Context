#!/usr/bin/env python3
"""Replay zero-byte shutdown artifacts with leak detection disabled only for each child."""
import json
import os
from pathlib import Path
import subprocess

out = Path(__file__).resolve().parent
base = Path('/tmp/Infinity-Context/validation/fuzz')
rows = []
for target in ['ahtl_stream', 'hdc_snapshot', 'mcp_jsonrpc']:
    binary = base / target / 'target/x86_64-unknown-linux-gnu/release' / target
    artifact = base / target / 'artifacts/crash-da39a3ee5e6b4b0d3255bfef95601890afd80709'
    assert artifact.read_bytes() == b''
    env = os.environ.copy()
    env['ASAN_OPTIONS'] = env.get('ASAN_OPTIONS', '') + ':detect_leaks=0'
    command = [str(binary), '-runs=1', str(artifact)]
    result = subprocess.run(command, env=env, cwd=base / target, capture_output=True, text=True, timeout=15)
    (out / f'state-debug-replay-{target}.log').write_text(result.stdout + result.stderr)
    rows.append({'target': target, 'command': command, 'exit': result.returncode,
                 'child_override': 'ASAN_OPTIONS appended :detect_leaks=0',
                 'artifact_bytes': 0, 'scope': 'single-input replay only; original 901s campaign unchanged'})
(out / 'state-debug-sanitizer-replay.json').write_text(json.dumps(rows, indent=2) + '\n')
print(json.dumps(rows, indent=2))
