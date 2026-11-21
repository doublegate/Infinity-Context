#!/usr/bin/env python3
"""Extract documentary leads and check local Markdown targets without rewriting sources."""
import csv
import re
import subprocess
from pathlib import Path

root = Path.cwd()
out = root / 'docs/codex/evidence'
files = [Path(p) for p in subprocess.check_output(['git', 'ls-files'], text=True).splitlines() if p.endswith('.md')]
leads, broken = [], []
for path in files:
    fenced = False
    for n, line in enumerate(path.read_text().splitlines(), 1):
        if line.lstrip().startswith(('```', '~~~')):
            fenced = not fenced
            continue
        if fenced:
            continue
        if re.search(r'\b(?:must|shall|guarantee|guarantees|unlimited|always|never|enforced)\b|(?:\d[\d,.]*\s*(?:%|µs|ns|ms|KiB|MiB|bits|tokens|x\b))', line, re.I):
            status = 'historical/design; no independent guarantee inferred' if str(path).startswith(('ref-docs/', 'to-dos/', 'docs/claude/', 'docs/adr/')) else 'lead; see material-claim matrix for adjudication'
            leads.append([str(path), n, line, status])
        for target in re.findall(r'\]\(([^\s)]+)(?:\s+[^)]*)?\)', line):
            if re.match(r'^[a-zA-Z][a-zA-Z0-9+.-]*:', target) or target.startswith(('#', '/', '~')):
                continue
            target = target.split('#')[0].split('?')[0]
            if target and not (path.parent / target).exists():
                broken.append([str(path), n, target])
for name, header, rows in [('documentary-claim-leads.csv', ['path','line','text','evidence_status'], leads), ('broken-local-links.csv', ['path','line','target'], broken)]:
    with (out / name).open('w', newline='') as f:
        writer = csv.writer(f)
        writer.writerow(header)
        writer.writerows(rows)
print(f'{len(files)} tracked Markdown files; {len(leads)} claim leads; {len(broken)} missing local link targets (heuristic; inspect before findings)')
for row in broken:
    print(':'.join(map(str, row)))
