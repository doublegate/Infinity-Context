#!/usr/bin/env python3
"""Independent arithmetic/codepoint oracle for all 256 reference-table rows."""
import json
from pathlib import Path
import re
import unicodedata

source = Path('ref-docs/unicode-braille-patterns/README.md')
rows, errors = 0, []
for n, line in enumerate(source.read_text().splitlines(), 1):
    if not re.match(r'^\| `0x[0-9A-F]{2}` \| `U\+28[0-9A-F]{2}`', line):
        continue
    cells = [s.strip().strip('`') for s in line.strip('|').split('|')]
    byte = int(cells[0], 16)
    codepoint = chr(0x2800 + byte)
    checks = [cells[1] == f'U+{ord(codepoint):04X}', int(cells[2]) == byte,
              cells[3] == f'{byte:08b}', cells[4] == codepoint.encode().hex(' ').upper(),
              cells[5] == codepoint, cells[8] == unicodedata.name(codepoint)]
    if not all(checks):
        errors.append({'line': n, 'byte': byte, 'checks': checks})
    rows += 1
example_glyph = '͇'
result = {'table_rows_checked': rows, 'table_errors': errors, 'python_unicode_version': unicodedata.unidata_version,
          'worked_example_actual': f'U+{ord(example_glyph):04X}', 'worked_example_claimed': 'U+28C7',
          'correct_braille_utf8': chr(0x28c7).encode().hex(' ')}
Path('docs/codex/evidence/braille-table-result.json').write_text(json.dumps(result, indent=2) + '\n')
assert rows == 256 and not errors
assert ord(example_glyph) != 0x28c7
print(json.dumps(result, indent=2))
