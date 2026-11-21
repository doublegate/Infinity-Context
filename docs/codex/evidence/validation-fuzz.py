#!/usr/bin/env python3
"""Run each shipped fuzz target for 900 seconds with isolated writable output."""
import concurrent.futures, json, os, subprocess, time
from pathlib import Path
out=Path(__file__).resolve().parent
base=Path('/tmp/Infinity-Context/validation/fuzz'); base.mkdir(parents=True,exist_ok=True)
def run(target):
 work=base/target; work.mkdir(exist_ok=True); (work/'corpus').mkdir(exist_ok=True); (work/'artifacts').mkdir(exist_ok=True)
 env=os.environ.copy(); env.update(CARGO_NET_OFFLINE='true',CARGO_TARGET_DIR=str(work/'target'),TMPDIR=str(work))
 cmd=['cargo','+nightly','fuzz','run',target,str(work/'corpus'),'--','-max_total_time=900','-rss_limit_mb=2048',f'-artifact_prefix={work}/artifacts/']
 start=time.time()
 with (out/f'validation-fuzz-{target}.log').open('w') as f:
  p=subprocess.run(cmd,cwd='fuzz',env=env,stdout=f,stderr=subprocess.STDOUT)
 return dict(target=target,command=cmd,exit=p.returncode,seconds=round(time.time()-start,2))
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
 rows=[]
 for row in pool.map(run,['ahtl_stream','hdc_snapshot','mcp_jsonrpc','redact']):
  rows.append(row); print(row,flush=True)
 (out/'validation-fuzz.json').write_text(json.dumps(rows,indent=2)+'\n')
