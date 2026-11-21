#!/usr/bin/env python3
"""Enumerate every mutation and execute deterministic 1/100 samples per crate."""
import json, os, subprocess, time
from pathlib import Path
out=Path(__file__).resolve().parent; base=Path('/tmp/Infinity-Context/validation'); env=os.environ.copy(); env.update(CARGO_NET_OFFLINE='true',TMPDIR=str(base))
rows=[]
for crate in ['core','hdc','codec','store','bus','adapters','proxy','cli']:
 name='icx-'+crate
 listing=subprocess.run(['cargo','mutants','-p',name,'--list','--json'],env=env,capture_output=True,text=True)
 (out/f'validation-mutation-{name}-inventory.json').write_text(listing.stdout)
 cmd=['cargo','mutants','-p',name,'--shard','1/100','--sharding','round-robin','--timeout','60','--build-timeout','180','-j','2','--output',str(base/('mutants-'+name))]
 start=time.time()
 with (out/f'validation-mutation-{name}.log').open('w') as f: result=subprocess.run(cmd,env=env,stdout=f,stderr=subprocess.STDOUT)
 row=dict(crate=name,command=cmd,exit=result.returncode,seconds=round(time.time()-start,2)); rows.append(row); print(row,flush=True)
 (out/'validation-mutations.json').write_text(json.dumps(rows,indent=2)+'\n')
