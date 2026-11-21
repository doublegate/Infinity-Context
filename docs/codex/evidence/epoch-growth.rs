// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later
use icx_core::{AgentKind, ArtifactId};
use icx_core::dag::MerkleContextDag;
use icx_core::epoch::{CacheEpochController, EpochControllerConfig};

fn main() {
    let mut dag = MerkleContextDag::new();
    let mut ctrl = CacheEpochController::new("head", EpochControllerConfig::default());
    println!("turns,sealed_epochs,prefix_bytes,zone1_estimated_tokens");
    for i in 0u64..1000 {
        let (raw, _) = ArtifactId::hash_bytes(&i.to_le_bytes());
        ctrl.record_turn(&mut dag, AgentKind::ClaudeCode, "tool", raw,
            format!("unique operation {i} has artifact {raw}"), 0.5, 4096);
        if [10, 100, 1000].contains(&(i + 1)) {
            let view = ctrl.assemble_window(&dag, String::new());
            println!("{},{},{},{}", i + 1, ctrl.sealed_epoch_count(),
                view.deterministic_prefix().len(), ctrl.zone1_compressed_tokens());
        }
    }
    assert_eq!(ctrl.sealed_epoch_count(), 996);
    assert!(ctrl.assemble_window(&dag, String::new()).deterministic_prefix().len() > 32768);
}
