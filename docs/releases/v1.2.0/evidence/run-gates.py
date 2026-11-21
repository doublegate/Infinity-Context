#!/usr/bin/env python3
"""Record current local gates without overwriting the historical audit or changing user config."""
import datetime
import json
import os
from pathlib import Path
import subprocess
import time

root = Path(__file__).resolve().parents[4]
out = Path(__file__).resolve().parent
env = dict(os.environ, CARGO_NET_OFFLINE='true', CARGO_TARGET_DIR='/tmp/Infinity-Context/v120/target',
           TMPDIR='/tmp/Infinity-Context/v120')
steps = [
    ('format', ['cargo', 'fmt', '--all', '--', '--check']),
    ('clippy', ['cargo', 'clippy', '--workspace', '--all-targets', '--', '-D', 'warnings']),
    ('rustdoc', ['cargo', 'doc', '--workspace', '--no-deps']),
    ('deny', ['cargo', 'deny', '--offline', '--locked', '--config', '/tmp/Infinity-Context/validation/deny-isolated.toml', 'check']),
    ('audit', ['cargo', 'audit', '--no-fetch', '--db', '/tmp/Infinity-Context/validation/advisory-dbs-clean/advisory-db-3157b0e258782691']),
    ('notices', ['cargo', 'about', 'generate', '--offline', '--locked', '--fail', '-c', '.github/about.toml', '.github/about.hbs', '-o', '/tmp/Infinity-Context/v120/THIRD-PARTY-NOTICES.txt']),
    ('machete', ['cargo', 'machete']),
    ('reuse', ['/home/parobek/.cache/uv/archive-v0/DF5VlosCbTYFZNoF/bin/reuse', 'lint']),
    ('secrets-history', ['/tmp/Infinity-Context/validation/gitleaks', 'git', '.', '--log-opts=--all --full-history -m', '--no-banner', '--redact', '--exit-code', '1']),
    ('actionlint', ['actionlint', '-color=false']),
    ('agy-selftest', ['bash', 'scripts/agy-review-selftest.sh']),
    ('cla-selftest', ['bash', '.github/scripts/cla-check.sh', '--selftest']),
    ('release-policy-tests', ['python3', '.github/scripts/test-release-ci-gate.py']),
    ('benchmark-runner-tests', ['python3', 'scripts/test_benchmark_series.py']),
]
rows = []
for name, command in steps:
    child_env = dict(env)
    if name == 'rustdoc':
        child_env['RUSTDOCFLAGS'] = '-D warnings'
    started = time.monotonic()
    with (out / ('gate-' + name + '.log')).open('w') as log:
        result = subprocess.run(command, cwd=root, env=child_env, stdout=log, stderr=subprocess.STDOUT)
    row = {'name': name, 'command': command, 'exit': result.returncode,
           'seconds': round(time.monotonic() - started, 3)}
    rows.append(row)
    print(json.dumps(row), flush=True)
(out / 'gates.json').write_text(json.dumps({'observed_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
                                          'results': rows}, indent=2) + '\n')
raise SystemExit(any(row['exit'] for row in rows))
