#!/usr/bin/env python3
"""Prepare per-asset review/exclusion candidates; do not delete assets or invent grants."""
import csv
import hashlib
from pathlib import Path

root = Path(__file__).resolve().parents[4]
out = Path(__file__).resolve().parent / 'governance-assets.csv'
base = root / 'ref-docs/unicode-braille-patterns'
fields = ['path', 'bytes', 'sha256', 'source_reference', 'grant_evidence', 'proposed_action', 'status']
with out.open('w', newline='') as stream:
    writer = csv.DictWriter(stream, fieldnames=fields)
    writer.writeheader()
    for path in sorted(base.rglob('*')):
        if not path.is_file():
            continue
        publication = path.suffix.lower() in ['.pdf', '.doc']
        generated = 'generated' in path.parts or path.name == 'README.md'
        writer.writerow({
            'path': str(path.relative_to(root)), 'bytes': path.stat().st_size,
            'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
            'source_reference': ('https://www.unicode.org/charts/PDF/U2800.pdf' if path.name == 'Unicode_U2800_Braille_Patterns_Chart.pdf'
                                 else 'Local source/provenance table: ref-docs/unicode-braille-patterns/README.md; exact external source requires verification'),
            'grant_evidence': 'No asset-specific independent grant established by this implementation; current REUSE annotation is not permission evidence',
            'proposed_action': ('Exclude publication from source/release distribution until grant verified; preserve original privately' if publication
                                else 'Retain first-party file with provenance review' if generated
                                else 'Verify embedded upstream grant and attribution before retention'),
            'status': 'owner/qualified review pending',
        })
print('Wrote asset-level review ledger without changing any archived asset')
