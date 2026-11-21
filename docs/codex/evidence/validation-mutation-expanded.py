#!/usr/bin/env python3
"""Execute deterministic 1/20 shards per crate with host socket permission."""
import concurrent.futures,json,os,subprocess,time
from pathlib import Path
out=Path(__file__).resolve().parent; base=Path('/tmp/Infinity-Context/validation'); env=os.environ.copy(); env.update(CARGO_NET_OFFLINE='true',TMPDIR=str(base),CARGO_BUILD_JOBS='2')
def run(crate):
 name='icx-'+crate; cmd=['cargo','mutants','-p',name,'--shard','1/20','--sharding','round-robin','--timeout','60','--build-timeout','240','-j','2','--output',str(base/('expanded-'+name))]; start=time.time()
 with (out/f'validation-mutation-expanded-{name}.log').open('w') as f: p=subprocess.run(cmd,env=env,stdout=f,stderr=subprocess.STDOUT)
 row=dict(crate=name,command=cmd,exit=p.returncode,seconds=round(time.time()-start,2)); print(row,flush=True); return row
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
 rows=list(pool.map(run,['core','codec','store','bus','hdc','adapters','proxy','cli']))
 (out/'validation-mutations-expanded.json').write_text(json.dumps(rows,indent=2)+'\n')
