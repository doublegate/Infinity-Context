#!/usr/bin/env python3
"""Exercise the release assembler with a local gh stub, no GitHub writes."""
import hashlib,json,os,subprocess,tempfile
from pathlib import Path
repo=Path.cwd(); out=repo/'docs/releases/v1.2.0/evidence'; base=Path('/tmp/Infinity-Context/v120/release-fixtures'); base.mkdir(exist_ok=True)
mock="#!/usr/bin/env python3\nimport os,sys,shutil\nfrom pathlib import Path\nargs=sys.argv[1:]; source=Path(os.environ['FIXTURE_ATTACHED'])\nif args[:2]==['release','view']:\n mode=os.environ.get('FIXTURE_VIEW','present')\n if mode=='missing': print('release not found',file=sys.stderr);sys.exit(1)\n if mode=='outage': print('HTTP 503',file=sys.stderr);sys.exit(1)\n print('\\n'.join(p.name for p in source.iterdir()));sys.exit(0)\nif args[:2]==['release','download']:\n name=args[args.index('--pattern')+1]; target=Path(args[args.index('--dir')+1]); shutil.copyfile(source/name,target/name);sys.exit(0)\nsys.exit(99)\n"
rows=[]
for case in ['new','preserve','extend','incompatible','outage']:
 root=Path(tempfile.mkdtemp(prefix=case+'-',dir=base)); (root/'bin').mkdir();(root/'dist').mkdir();(root/'source').mkdir()
 gh=root/'bin/gh';gh.write_text(mock);gh.chmod(0o755)
 (root/'dist/icx-old.tar.gz').write_bytes(b'fresh-rebuild')
 (root/'dist/THIRD-PARTY-NOTICES.txt').write_text('fixture notices')
 if case in ['preserve','extend','incompatible']:
  attached=root/'source/icx-old.tar.gz'; attached.write_bytes(b'previously-published')
  sums=hashlib.sha256(attached.read_bytes()).hexdigest()+'  icx-old.tar.gz\n'
  if case=='incompatible': sums='0'*64+'  icx-old.tar.gz\n'
  (root/'source/SHA256SUMS').write_text(sums)
 if case=='extend': (root/'dist/icx-new.tar.gz').write_bytes(b'additional-target')
 env=os.environ.copy();env.update(PATH=str(root/'bin')+':'+env['PATH'],TAG='v1.2.0',FIXTURE_ATTACHED=str(root/'source'),FIXTURE_VIEW='missing' if case=='new' else 'outage' if case=='outage' else 'present',GITHUB_OUTPUT=str(root/'outputs'))
 result=subprocess.run(['bash',str(repo/'.github/scripts/release-assemble.sh')],cwd=root,env=env,capture_output=True,text=True)
 expected=1 if case in ['incompatible','outage'] else 0
 kept=(root/'final/icx-old.tar.gz').read_bytes() if (root/'final/icx-old.tar.gz').exists() else None
 assertions={'exit_expected':result.returncode==expected}
 if case in ['preserve','extend']: assertions['attached_bytes_preserved']=kept==b'previously-published';assertions['no_reupload_attached']=not (root/'upload/icx-old.tar.gz').exists()
 if case=='extend': assertions['new_asset_uploaded']=(root/'upload/icx-new.tar.gz').exists()
 row=dict(case=case,exit=result.returncode,assertions=assertions,stdout=result.stdout,stderr=result.stderr);rows.append(row)
 assert all(assertions.values()),row
(out/'release-assembler-fixtures.json').write_text(json.dumps(rows,indent=2)+'\n')
print('PASS: five release-assembly cases')
