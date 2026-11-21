#!/usr/bin/env python3
"""Warm up once, then run ten independent benchmark processes per CPU target."""
import json,os,subprocess,time,statistics
from pathlib import Path
out=Path(__file__).resolve().parent; base=Path('/tmp/Infinity-Context/validation'); env=os.environ.copy(); env.update(CARGO_NET_OFFLINE='true',TMPDIR=str(base))
rows=[]
for profile,flags in [('portable','-C target-cpu=x86-64'),('native','-C target-cpu=native')]:
 env.update(RUSTFLAGS=flags,CARGO_TARGET_DIR=str(base/('bench-'+profile)))
 with (out/f'validation-benchmark-{profile}-build.log').open('w') as f: p=subprocess.run(['cargo','build','--release','--locked','-p','icx-cli'],env=env,stdout=f,stderr=subprocess.STDOUT)
 if p.returncode: rows.append(dict(profile=profile,build_exit=p.returncode)); continue
 binary=base/('bench-'+profile)/'release/icx'
 for i in range(11):
  start=time.perf_counter()
  with (out/f'validation-benchmark-{profile}-{i:02}.log').open('w') as f: p=subprocess.run([str(binary),'benchmark'],env=env,stdout=f,stderr=subprocess.STDOUT)
  rows.append(dict(profile=profile,run=i,warmup=i==0,exit=p.returncode,wall_ms=(time.perf_counter()-start)*1000))
  print(rows[-1],flush=True)
 (out/'validation-benchmarks.json').write_text(json.dumps(rows,indent=2)+'\n')
