#!/usr/bin/env python3
"""Read-only source inventory; outputs audit evidence only. Run from repository root."""
import csv
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path.cwd()
OUT = ROOT / 'docs/codex/evidence'

def git(*args):
    return subprocess.check_output(['git', *args], text=True)

def main():
    paths = git('ls-files', '-z').split('\0')
    rows = []
    for name in sorted(filter(None, paths)):
        p = ROOT / name
        data = p.read_bytes()
        if name.startswith(('crates/icx-codec/', 'crates/icx-hdc/', 'crates/icx-store/', 'docs/ahtl/')):
            report, depth = 'codec_math', 'source/spec review; targeted executable probes'
        elif name.startswith(('crates/icx-core/', 'crates/icx-bus/', 'crates/icx-adapters/', 'crates/icx-proxy/')) or name == 'crates/icx-cli/src/daemon.rs':
            report, depth = 'state_security', 'source/spec review; targeted executable probes'
        elif name.startswith(('.github/', 'scripts/', 'fuzz/', 'LICENSES/')) or p.suffix in ('.toml', '.lock', '.yml', '.yaml'):
            report, depth = 'validation_supply', 'configuration/source review or licensing metadata'
        elif p.suffix == '.md':
            report, depth = 'documentary corpus', 'document review; historical assertions distinguished'
        elif p.suffix in ('.svg', '.png', '.ico', '.jpg', '.webp', '.dot', '.mmd'):
            report, depth = 'assets', 'inventory/hash; source labels sampled; visual completeness not certified'
        elif name.startswith('ref-docs/'):
            report, depth = 'reference assets', 'inventory/hash; citation checks; not full external-archive semantic review'
        else:
            report, depth = 'supporting files', 'inventory; targeted review; see coverage limitations'
        rows.append([name, len(data), hashlib.sha256(data).hexdigest(), report, depth])
    with (OUT / 'inventory.csv').open('w', newline='') as f:
        w = csv.writer(f)
        w.writerow(['path', 'bytes', 'sha256', 'workstream', 'disposition_and_depth'])
        w.writerows(rows)
    baseline = {'revision': git('rev-parse', 'HEAD').strip(), 'branch': git('branch', '--show-current').strip(),
                'tracked_count': len(rows), 'starting_branch': 'ci/release-workflow',
                'starting_status': ' M AGENTS.md', 'starting_diff_stat': 'AGENTS.md | 25; 24 insertions(+), 1 deletion(-)',
                'agents_sha256': hashlib.sha256((ROOT / 'AGENTS.md').read_bytes()).hexdigest(),
                'notes': 'Starting status independently captured before audit files were created. Inventory includes the preserved local AGENTS.md content.'}
    (OUT / 'baseline.json').write_text(json.dumps(baseline, indent=2) + '\n')
    (OUT / 'starting-AGENTS.patch').write_text(git('diff', '--', 'AGENTS.md'))
    print(json.dumps(baseline, indent=2))

if __name__ == '__main__':
    main()
