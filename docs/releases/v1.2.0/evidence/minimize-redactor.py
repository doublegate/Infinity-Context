#!/usr/bin/env python3
"""Minimize a synthetic idempotence counterexample without changing production source."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

binary = sys.argv[1]
original = Path(sys.argv[2]).read_bytes()
scratch = Path('/tmp/Infinity-Context/v120/redactor-minimize-input')


def replay(data):
    scratch.write_bytes(data)
    return subprocess.run([binary, 'redact', str(scratch)], capture_output=True, text=True)


def fails(data):
    try:
        data.decode('utf-8')
    except UnicodeDecodeError:
        return False
    return replay(data).returncode == 2


assert fails(original)
data = original
granularity = 2
while len(data) > 1:
    size = (len(data) + granularity - 1) // granularity
    for start in range(0, len(data), size):
        candidate = data[:start] + data[start + size:]
        if candidate and fails(candidate):
            data = candidate
            granularity = max(2, granularity - 1)
            break
    else:
        if granularity >= len(data):
            break
        granularity = min(len(data), granularity * 2)
result = {'original_sha256': hashlib.sha256(original).hexdigest(), 'original_hex': original.hex(),
          'minimal_sha256': hashlib.sha256(data).hexdigest(), 'minimal_hex': data.hex(),
          'minimal_utf8': data.decode(), 'replay': replay(data).stdout,
          'scope': 'single-byte-deletion minimal; property failure, not production panic'}
Path(sys.argv[3]).write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
