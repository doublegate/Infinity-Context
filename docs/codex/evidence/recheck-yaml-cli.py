#!/usr/bin/env python3
"""Independent end-to-end redaction reproduction using a real, isolated CLI workspace."""
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

binary = Path(sys.argv[1]).resolve()
repo = Path.cwd()
base = Path(tempfile.mkdtemp(prefix='yaml-cli-', dir='/tmp/Infinity-Context'))
env = dict(os.environ, ICX_CONFIG_HOME=str(base / 'config'), ICX_STATE_HOME=str(base / 'state'),
           XDG_RUNTIME_DIR=str(base / 'runtime'), ICX_NO_DAEMON='1')
(base / 'runtime').mkdir(mode=0o700)
fixture = base / 'fixture.yaml'
fixture.write_text('password: |\n  lowentropyfirst\n  lowentropysecond\npublic: ok\n')
ws = base / 'workspace'
rows = []
def run(args):
    p = subprocess.run([str(binary), *args], env=env, capture_output=True, text=True)
    rows.append({'args': args, 'exit': p.returncode, 'stdout': p.stdout, 'stderr': p.stderr})
    if p.returncode:
        raise RuntimeError(p.stderr)
    return p.stdout
run(['init', '--dir', str(ws)])
packet = run(['encode-ahtl', str(fixture), '--dir', str(ws)])
handle = re.search(r'#icx:[0-9a-f]{16}', packet).group()
body = run(['deref', handle, '--raw', '--dir', str(ws)])
result = {'commands': rows, 'synthetic_secret_survives_cas': 'lowentropyfirst' in body and 'lowentropysecond' in body,
          'public_field_preserved': 'public: ok' in body, 'binary': str(binary)}
(repo / 'docs/codex/evidence/yaml-cli-result.json').write_text(json.dumps(result, indent=2) + '\n')
assert result['synthetic_secret_survives_cas']
assert result['public_field_preserved']
print(json.dumps({k: v for k, v in result.items() if k != 'commands'}, indent=2))
