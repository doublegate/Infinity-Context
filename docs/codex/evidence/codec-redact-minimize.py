#!/usr/bin/env python3
"""Delta-debug the preserved synthetic redactor idempotence failure, without production edits."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

binary = Path(sys.argv[1]).resolve()
original = Path(sys.argv[2]).read_bytes()
scratch = Path('/tmp/Infinity-Context/codec-audit/redact-minimize-input')

def run(data):
    scratch.write_bytes(data)
    p = subprocess.run([str(binary), 'redact', str(scratch)], capture_output=True, text=True)
    return p.returncode, p.stdout, p.stderr

def fails(data):
    try:
        data.decode('utf-8')
    except UnicodeDecodeError:
        return False
    return run(data)[0] == 2

assert fails(original)
data = original
granularity = 2
while len(data) > 1:
    chunk = (len(data) + granularity - 1) // granularity
    for start in range(0, len(data), chunk):
        candidate = data[:start] + data[start + chunk:]
        if candidate and fails(candidate):
            data = candidate
            granularity = max(2, granularity - 1)
            break
    else:
        if granularity >= len(data):
            break
        granularity = min(len(data), granularity * 2)
readable = b'password:\n  ask-123456789=x\n'
assert fails(readable)
result = {
    'original_bytes': len(original), 'original_sha256': hashlib.sha256(original).hexdigest(),
    'original_hex': original.hex(), 'original_replay': run(original),
    'minimal_bytes': len(data), 'minimal_sha256': hashlib.sha256(data).hexdigest(),
    'minimal_hex': data.hex(), 'minimal_replay': run(data),
    'readable_bytes': len(readable), 'readable_sha256': hashlib.sha256(readable).hexdigest(),
    'readable_utf8': readable.decode(), 'readable_replay': run(readable),
    'criterion': 'Non-idempotence returns exit2 from audit harness, not a production panic',
    'minimality': '1-minimal by byte deletion, not a proof of globally shortest input'
}
Path('docs/codex/evidence/codec-redact-idempotence.json').write_text(json.dumps(result, indent=2) + '\n')
Path('docs/codex/evidence/codec-redact-minimal.bin').write_bytes(data)
Path('docs/codex/evidence/codec-redact-readable.txt').write_bytes(readable)
print(json.dumps(result, indent=2))
