#!/usr/bin/env python3
"""Read experiment outcomes/logs without changing or stopping any experiment."""
import datetime
import json
from pathlib import Path

out = Path(__file__).resolve().parent
base = Path('/tmp/Infinity-Context/validation')
result = {'observed_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'fuzz': [], 'expanded_mutations': []}
retry_path = out / 'state-debug-retry-summary.json'
retries = {}
if retry_path.exists():
    for row in json.loads(retry_path.read_text()).get('outcomes', {}).get('outcomes', []):
        if 'Mutant' in row.get('scenario', {}):
            retries[row['scenario']['Mutant']['name']] = row['summary']
for target in ['ahtl_stream', 'hdc_snapshot', 'mcp_jsonrpc', 'redact']:
    log = (out / f'validation-fuzz-{target}.log').read_text(errors='replace')
    result['fuzz'].append({'target': target, 'completed_900_seconds': 'Done ' in log and 'in 900 second' in log or 'in 901 second' in log,
                           'fuzzer_exit_77': 'exit status: 77' in log, 'last_log_lines': log.splitlines()[-4:]})
for crate in ['core', 'codec', 'store', 'bus', 'hdc', 'adapters', 'proxy', 'cli']:
    folder = base / f'expanded-icx-{crate}/mutants.out'
    path = folder / 'outcomes.json'
    if not path.exists():
        result['expanded_mutations'].append({'crate': 'icx-' + crate, 'status': 'no outcomes file'})
        continue
    data = json.loads(path.read_text())
    selected = json.loads((folder / 'mutants.json').read_text())
    mutated = [r for r in data['outcomes'] if 'Mutant' in r.get('scenario', {})]
    counts = {}
    infrastructure_outcomes = []
    corrected = {}
    for row in mutated:
        counts[row['summary']] = counts.get(row['summary'], 0) + 1
        log_path = folder / row['log_path']
        name = row['scenario']['Mutant']['name']
        status = row['summary']
        if log_path.exists() and 'No space left on device' in log_path.read_text(errors='replace'):
            infrastructure_outcomes.append(name)
            status = retries.get(name, 'InfrastructureError')
        corrected[status] = corrected.get(status, 0) + 1
    log = (out / f'validation-mutation-expanded-icx-{crate}.log').read_text(errors='replace')
    result['expanded_mutations'].append({'crate': 'icx-' + crate, 'selected': len(selected),
        'completed_mutants': len(mutated), 'pending': max(0, len(selected) - len(mutated)), 'counts_raw': counts,
        'counts_adjudicated_with_targeted_retries': corrected,
        'end_time': data.get('end_time'), 'worker_copy_enospc': 'No space left on device' in log,
        'outcomes_contaminated_by_enospc': infrastructure_outcomes})
(out / 'state-debug-monitor.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
