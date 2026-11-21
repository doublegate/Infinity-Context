#!/usr/bin/env python3
"""Reproduce local audit gates; output stays in the provided evidence directory."""
import json, os, subprocess, time, shutil
from pathlib import Path
out=Path(__file__).resolve().parent
scratch=Path('/tmp/Infinity-Context/validation'); scratch.mkdir(parents=True,exist_ok=True)
env=os.environ.copy(); env['CARGO_TARGET_DIR']=str(scratch/'target'); env['TMPDIR']=str(scratch); env['CARGO_NET_OFFLINE']='true'
env['UV_TOOL_DIR']=str(scratch/'uv-tools')
# Local advisory DB locks must not touch the user's read-only stores.
advisory_copy=scratch/'runner-advisory-dbs'
if not advisory_copy.exists():
 shutil.copytree(Path.home()/'.cargo/advisory-dbs',advisory_copy,ignore=shutil.ignore_patterns('*.lock','*.ipc'))
deny_config=scratch/'runner-deny.toml'
deny_config.write_text(Path('deny.toml').read_text().replace('[advisories]',f'[advisories]\ndb-path = "{advisory_copy}"'))
commands=[('fmt','cargo fmt --all -- --check'),('clippy','cargo clippy --workspace --all-targets -- -D warnings'),('tests','cargo test --workspace'),('rustdoc','RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps'),('coverage','cargo llvm-cov --workspace --all-targets --lcov --output-path /tmp/Infinity-Context/validation/lcov.info --fail-under-lines 95'),('benchmark','cargo run --release -p icx-cli -- benchmark'),('doctor','cargo run --release -p icx-cli -- doctor'),('deny','cargo deny --workspace --disable-fetch check'),('audit','cargo audit --no-fetch'),('about','cargo about generate --offline --locked --fail -c .github/about.toml .github/about.hbs -o /tmp/Infinity-Context/validation/THIRD-PARTY-NOTICES.txt'),('machete','cargo machete'),('reuse','UV_CACHE_DIR=/tmp/Infinity-Context/validation/uv uvx --offline reuse lint'),('gitleaks','gitleaks git . --log-opts="--all --full-history -m" --no-banner --redact --exit-code 1'),('actionlint','actionlint -color=false'),('agy-selftest','bash scripts/agy-review-selftest.sh'),('cla-selftest','bash .github/scripts/cla-check.sh --selftest')]
rows=[]
for name,cmd in commands:
 if name=='deny': cmd=f'cargo deny --workspace --offline --locked --config {deny_config} check'
 if name=='doctor': cmd=f'{scratch}/target/release/icx doctor --dir {scratch}/uninitialized-doctor'
 start=time.time()
 with (out/f'validation-{name}.log').open('w') as f:
  p=subprocess.run(cmd,shell=True,env=env,stdout=f,stderr=subprocess.STDOUT)
 row=dict(name=name,command=cmd,exit=p.returncode,seconds=round(time.time()-start,3)); rows.append(row)
 (out/'validation-gates.json').write_text(json.dumps(rows,indent=2)+'\n')
 print(row,flush=True)
