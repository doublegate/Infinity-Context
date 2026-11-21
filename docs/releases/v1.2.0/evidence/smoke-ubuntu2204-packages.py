#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 DoubleGate
# SPDX-License-Identifier: GPL-3.0-or-later
"""Smoke all four preliminary packages in pinned Ubuntu22.04, offline and isolated."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
IMAGE = 'ubuntu@sha256:f64b2ffc2f10ab8f8505aa56b2664bd846cbb9df87c7e2532e30f7667021c0e6'
work = Path(tempfile.mkdtemp(prefix='ubuntu2204-package-smoke-', dir='/tmp/Infinity-Context/v120'))
config = work / 'docker-config'
config.mkdir(mode=0o700)
(config / 'config.json').write_text('{}\n')
env = {'PATH': os.environ['PATH'], 'DOCKER_CONFIG': str(config)}


def run(argv):
    try:
        completed = subprocess.run(argv, env=env, text=True, capture_output=True, timeout=30, check=False)
        return {'argv': argv, 'exit': completed.returncode, 'stdout': completed.stdout, 'stderr': completed.stderr}
    except (OSError, subprocess.TimeoutExpired) as error:
        return {'argv': argv, 'exit': None, 'error': str(error)}


report = {'scope': 'preliminary package smoke, not final release certification', 'image': IMAGE,
          'image_inspect': run(['docker', 'image', 'inspect', IMAGE]), 'work_dir': str(work), 'rows': []}
base = ['docker', 'run', '--rm', '--pull=never', '--network=none', '--read-only', '--cap-drop=ALL',
        '--security-opt=no-new-privileges', '--user', f'{os.getuid()}:{os.getgid()}']
report['runtime_version'] = run(base + [IMAGE, 'ldd', '--version'])
report['no_musl_loader_installed'] = run(base + [IMAGE, '/bin/sh', '-ec', 'test ! -e /lib/ld-musl-x86_64.so.1'])
if report['runtime_version']['exit'] != 0 or '2.35' not in report['runtime_version'].get('stdout', ''):
    raise RuntimeError('pinned image is not the expected usable glibc2.35 environment')
packages = json.loads((HERE / 'package-four-variant-results.json').read_text())['results']
for row in packages:
    label = row['label']
    state = work / label
    state.mkdir(mode=0o700)
    for name in ['home', 'proj', 'run']:
        (state / name).mkdir(mode=0o700)
    # Test the stripped executable actually placed in the archive, rather than the build output.
    binary = Path(row['package']['archive']).parent / 'staging' / f'icx-v1.2.0-{label}' / 'icx'
    command = base + ['--mount', f'type=bind,source={binary},target=/candidate/icx,readonly',
                     '--mount', f'type=bind,source={state},target=/work', '--workdir', '/work/proj',
                     '--env', 'HOME=/work/home', '--env', 'XDG_STATE_HOME=/work/home/.local/state',
                     '--env', 'XDG_CONFIG_HOME=/work/home/.config', '--env', 'XDG_RUNTIME_DIR=/work/run',
                     '--env', 'XDG_DATA_HOME=/work/home/.local/share', '--env', 'XDG_CACHE_HOME=/work/home/.cache',
                     '--env', 'PATH=/usr/bin:/bin', IMAGE, '/candidate/icx']
    outcomes = [run(command + [subcommand]) for subcommand in ['doctor', 'init', 'status']]
    report['rows'].append({'label': label, 'stripped_binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
                           'archive_sha256': row['package']['sha256'], 'outcomes': outcomes,
                           'pass': all(item['exit'] == 0 for item in outcomes)})
report['pass'] = all(row['pass'] for row in report['rows'])
report['final_release_certified'] = False
(HERE / 'package-ubuntu2204-smoke-results.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({'pass': report['pass'], 'variants': len(report['rows']), 'glibc': '2.35', 'final_release_certified': False}))
raise SystemExit(not report['pass'])
