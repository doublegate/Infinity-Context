#!/usr/bin/env python3
"""Reconcile finding IDs and the prior ledger; never edits production or old audits."""
from collections import Counter
import csv
import json
from pathlib import Path
import re

root = Path.cwd()
base = root / 'docs/codex'
evidence = base / 'evidence'
sources = ['codec-findings.json', 'state-findings.json', 'validation-findings.json', 'root-findings.json']
findings = []
for source in sources:
    findings.extend(json.loads((evidence / source).read_text()))
findings.sort(key=lambda f: f['id'])
assert len({f['id'] for f in findings}) == len(findings)
for f in findings:
    report = base / f['report']
    assert report.is_file(), report
    assert f['id'] in report.read_text(), f['id']
fields = ['id', 'severity', 'category', 'confidence', 'title', 'location', 'report', 'prior_ids']
with (base / 'findings.csv').open('w', newline='') as out:
    w = csv.DictWriter(out, fieldnames=fields, extrasaction='ignore')
    w.writeheader()
    for row in findings:
        w.writerow(dict(row, prior_ids='; '.join(row.get('prior_ids', []))))
counts = Counter(f['severity'].lower() for f in findings)
text = '# Consolidated findings register\n\n'
text += 'Revision `a996d59f1eb468e7c386747e15f4c7e58d08410e`. Stable IDs have reserved gaps. '
text += 'Severity applies within the stated category: governance questions and unsupported claims are not confirmed security exploits. '
text += f'{len(findings)} entries; severity counts: ' + ', '.join(f'{k}={v}' for k,v in sorted(counts.items())) + '.\n\n'
text += 'Full prerequisites, impact, reproduction/source proof, counterevidence, recommendations and acceptance checks are in the linked reports. '
text += '[CSV companion](findings.csv).\n\n| ID | Severity | Category | Finding and full evidence |\n| --- | --- | --- | --- |\n'
for f in findings:
    text += f"| {f['id']} | {f['severity']} | {f['category']} | [{f['title']}]({f['report']}) |\n"
(base / '10-findings-register.md').write_text(text)

prior = list(csv.DictReader((root / 'docs/claude/findings.csv').open()))
reviews = {}
for path in sorted(base.glob('0[1-7]-*.md')):
    for line in path.read_text().splitlines():
        match = re.match(r'\| (ICX-AUD-\d+) \| ([^|]+) \| (.*) \|$', line)
        if match:
            old_id, status, detail = match.groups()
            reviews.setdefault(old_id, []).append((path.name, status.strip(), detail.strip()))
reviews['ICX-AUD-033'] = [('08-documentation-and-claim-traceability.md', 'Partially fixed',
    'Dated corrections identify ACM/LCM/RVQ attribution issues; current accepted ADR-0003 still repeats misattributed cost/TTFT and provider-hit assertions (100). All22 arXiv IDs resolve, not every paper claim reproduced.')]
assert len(prior) == 51
assert set(row['id'] for row in prior) <= set(reviews), set(row['id'] for row in prior) - set(reviews)
# Assess original defect scopes, rather than treating unrelated new hardening as a regression.
original_fixed = {'ICX-AUD-014': 'Confirmed fixed for original unbounded stdin defect; elapsed-time/concurrency limits remain separate hardening',
                  'ICX-AUD-008': 'Confirmed fixed for original version-negotiation defect; new envelope validation issue031 is separate',
                  'ICX-AUD-040': 'Confirmed fixed for original SHA/toolchain/policy issues; release CI-success gap070 is separate'}
text = '# Independent reassessment of all prior findings\n\n'
text += 'The old CSV has **51 entries**, not55: reserved029 and043–045 were merged. This table covers every actual row. '
text += '“Fixed” is scoped to the original failure and cited mechanism, not certification of an entire subsystem. '
text += 'No prior private exploit evidence or live repository settings were assumed available.\n\n'
text += '| Prior ID | Previous reported state | Independent disposition and evidence |\n| --- | --- | --- |\n'
for row in prior:
    parts = []
    if row['id'] in original_fixed:
        parts.append(original_fixed[row['id']])
    for path, status, detail in reviews[row['id']]:
        parts.append(f'[{status}]({path}): {detail}')
    text += f"| {row['id']} | {row['status']} | " + '<br>'.join(parts) + ' |\n'
text += '\nNo new issue is labeled regressed without comparative old-revision reproduction. Current counterexamples can instead demonstrate a residual or broader coverage gap.\n'
(base / '13-prior-audit-reassessment.md').write_text(text)
(evidence / 'finding-counts.json').write_text(json.dumps({'count':len(findings), 'severity':dict(counts), 'prior_rows':len(prior)}, indent=2)+'\n')
print(f'{len(findings)} current entries; {len(prior)} prior entries reconciled')
