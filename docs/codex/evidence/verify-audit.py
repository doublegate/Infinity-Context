#!/usr/bin/env python3
"""Verify audit containment, unchanged audited inputs, and report cross-references."""
import ast
import csv
import hashlib
import json
from pathlib import Path
import re
import subprocess

root = Path.cwd()
base = root / 'docs/codex'
evidence = base / 'evidence'
errors = []
inventory = list(csv.DictReader((evidence / 'inventory.csv').open()))
for row in inventory:
    path = root / row['path']
    if not path.is_file() or hashlib.sha256(path.read_bytes()).hexdigest() != row['sha256']:
        errors.append('Audited input changed: ' + row['path'])
changed = subprocess.check_output(['git', 'diff', '--name-only'], text=True).splitlines()
untracked = subprocess.check_output(['git', 'ls-files', '--others', '--exclude-standard'], text=True).splitlines()
for name in changed:
    if name != 'AGENTS.md' and not name.startswith('docs/codex/'):
        errors.append('Unexpected tracked modification: ' + name)
for name in untracked:
    if not name.startswith('docs/codex/'):
        errors.append('Unexpected untracked path: ' + name)
patch = subprocess.check_output(['git', 'diff', '--', 'AGENTS.md'], text=True)
if patch != (evidence / 'starting-AGENTS.patch').read_text():
    errors.append('Starting AGENTS modification was not preserved verbatim')
for path in base.rglob('*.json'):
    try:
        json.loads(path.read_text())
    except (ValueError, UnicodeError) as exc:
        errors.append(f'Invalid JSON {path.relative_to(root)}: {exc}')
for path in evidence.glob('*.py'):
    try:
        ast.parse(path.read_text(), filename=str(path))
    except SyntaxError as exc:
        errors.append(f'Invalid Python {path.name}: {exc}')
checked_links = 0
for path in base.rglob('*.md'):
    fenced = False
    for line in path.read_text().splitlines():
        if line.lstrip().startswith(('```', '~~~')):
            fenced = not fenced
            continue
        if fenced:
            continue
        for target in re.findall(r'\]\(([^\s)]+)(?:\s+[^)]*)?\)', line):
            if re.match(r'^[a-zA-Z][a-zA-Z0-9+.-]*:', target) or target.startswith(('#', '/')):
                continue
            target = target.split('#')[0]
            if target:
                checked_links += 1
                if not (path.parent / target).exists():
                    errors.append(f'Missing link in {path.name}: {target}')
findings = list(csv.DictReader((base / 'findings.csv').open()))
counts = json.loads((evidence / 'finding-counts.json').read_text())
if len(findings) != counts['count'] or len(set(f['id'] for f in findings)) != len(findings):
    errors.append('Finding register count/ID mismatch')
result = {'audited_paths_unchanged': len(inventory), 'checked_local_links': checked_links,
          'current_findings': len(findings), 'prior_findings': counts['prior_rows'],
          'new_audit_files': len(list(base.rglob('*'))), 'errors': errors,
          'preserved_preexisting_diff_warning': 'AGENTS.md:99: new blank line at EOF; not modified by audit'}
(evidence / 'closeout-verification.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
raise SystemExit(bool(errors))
