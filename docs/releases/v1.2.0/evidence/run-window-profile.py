#!/usr/bin/env python3
"""Compare original and bounded view assembly on identical synthetic history, outside source."""
import json
import os
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[4]
out = Path(__file__).resolve().parent
tmp = Path('/tmp/Infinity-Context/v120/window-profile')
legacy = tmp / 'legacy'
(legacy / 'src').mkdir(parents=True, exist_ok=True)
for name in ['lib.rs', 'cas.rs', 'chunker.rs', 'dag.rs', 'epoch.rs']:
    data = subprocess.check_output(['git', 'show', 'a996d59:crates/icx-core/src/' + name], cwd=root)
    (legacy / 'src' / name).write_bytes(data)
(legacy / 'Cargo.toml').write_text('''[package]
name="icx-core-legacy"
version="0.0.0"
edition="2024"
[workspace]
[dependencies]
blake3="1.8.7"
serde={version="1.0.229",features=["derive"]}
serde_json="1.0.154"
thiserror="2.0.21"
''')
(tmp / 'src').mkdir(exist_ok=True)
(tmp / 'Cargo.toml').write_text('''[package]
name="icx-window-profile"
version="0.0.0"
edition="2024"
[workspace]
exclude=["legacy"]
[dependencies]
serde_json="1.0.154"
icx-core-legacy={path="legacy"}
icx-core={path=''' + json.dumps(str(root / 'crates/icx-core')) + '}\n')
(tmp / 'src/main.rs').write_text(r'''
macro_rules! measure {
    ($lib:ident, $n:expr) => {{
        use $lib::{AgentKind, ArtifactId};
        use $lib::dag::MerkleContextDag;
        use $lib::epoch::{CacheEpochController, EpochControllerConfig};
        let mut dag = MerkleContextDag::new();
        let mut ctrl = CacheEpochController::new("head", EpochControllerConfig::default());
        for i in 0u64..$n {
            let (raw, _) = ArtifactId::hash_bytes(&i.to_le_bytes());
            ctrl.record_turn(&mut dag, AgentKind::ClaudeCode, "tool", raw,
                format!("unique operation {i} has artifact {raw}"), 0.5, 4096);
        }
        let mut samples = Vec::new();
        for _ in 0..100 {
            let start = std::time::Instant::now();
            let view = ctrl.assemble_window(&dag, String::new());
            std::hint::black_box(view.deterministic_prefix());
            samples.push(start.elapsed().as_nanos() as u64);
        }
        samples.sort_unstable();
        let view = ctrl.assemble_window(&dag, String::new());
        (serde_json::json!({"prefix_bytes":view.deterministic_prefix().len(),
            "stored_epochs":ctrl.sealed_epoch_count(), "selected_epochs":view.sealed_epochs.len(),
            "p50_ns":samples[50],"p95_ns":samples[95]}), view.sealed_epochs)
    }};
}
fn main() {
    let mut rows = Vec::new();
    for n in [100u64,1000,10000] {
        let (before, old_frames) = measure!(icx_core_legacy, n);
        let (after, selected) = measure!(icx_core, n);
        assert!(selected.iter().all(|frame| old_frames.contains(frame)));
        rows.push(serde_json::json!({"turns":n,"before_a996d59":before,"after":after,
            "retained_frames_byte_identical":true}));
    }
    println!("{}",serde_json::to_string_pretty(&rows).unwrap());
}
''')
env = dict(os.environ, CARGO_NET_OFFLINE='true', RUSTFLAGS='',
           CARGO_TARGET_DIR='/tmp/Infinity-Context/v120/window-profile-target')
command = ['cargo', 'run', '--release', '--offline', '--manifest-path', str(tmp / 'Cargo.toml')]
with (out / 'window-profile-build.log').open('w') as log, (out / 'window-profile.json').open('w') as result:
    completed = subprocess.run(command, env=env, stdout=result, stderr=log)
raise SystemExit(completed.returncode)
