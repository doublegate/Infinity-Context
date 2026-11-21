#!/usr/bin/env python3
"""Confirm the equals-containing YAML scalar leakage using an isolated real CLI store."""
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

base = Path(tempfile.mkdtemp(prefix='codec-yaml-', dir='/tmp/Infinity-Context'))
runtime = base / 'runtime'
runtime.mkdir(mode=0o700)
env = dict(os.environ, ICX_CONFIG_HOME=str(base/'config'), ICX_STATE_HOME=str(base/'state'),
           XDG_RUNTIME_DIR=str(runtime), ICX_NO_DAEMON='1')
binary = str(Path(sys.argv[1]).resolve())
workspace = str(base/'workspace')
fixture = base/'plain.yaml'
fixture.write_text('password:\n  user=correcthorse\npublic: ok\n')
rows = []
def run(args):
    p = subprocess.run([binary, *args], env=env, capture_output=True, text=True)
    rows.append({'args':args, 'exit':p.returncode, 'stdout':p.stdout, 'stderr':p.stderr})
    assert p.returncode == 0
    return p.stdout
run(['init','--dir',workspace])
packet = run(['encode-ahtl',str(fixture),'--dir',workspace])
handle = re.search(r'#icx:[0-9a-f]{16}',packet).group()
body = run(['deref',handle,'--raw','--dir',workspace])
result = {'fixture':fixture.read_text(), 'commands':rows, 'synthetic_password_survives': 'user=correcthorse' in body,
          'note':'Input is valid YAML: password scalar contains an equals sign, not a nested mapping.'}
assert result['synthetic_password_survives']
Path('docs/codex/evidence/codec-redact-cli-result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
