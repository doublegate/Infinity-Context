use icx_core::cas::CasStore;
use icx_core::dag::MerkleContextDag;
use icx_core::{AgentKind, ArtifactId};

#[test]
fn chunk_identity_and_full_digest_are_independent_restore_requirements() {
    let mut store = CasStore::new();
    store.put_bytes_checked(b"checked chunk").unwrap();
    let (id, digest, bytes) = store.chunk_entries().next().unwrap();
    // No artifacts: later artifact reconstruction must not accidentally catch this for us.
    let wrong_id = ArtifactId(id.0 ^ 1);
    assert!(
        CasStore::from_parts(
            store.chunk_config(),
            [(wrong_id, *digest, bytes.to_vec())],
            []
        )
        .is_err()
    );
    let mut wrong_digest = *digest;
    wrong_digest.0[31] ^= 1; // Same 64-bit handle prefix, different full digest.
    assert_eq!(ArtifactId::from_digest(&wrong_digest), id);
    assert!(
        CasStore::from_parts(
            store.chunk_config(),
            [(id, wrong_digest, bytes.to_vec())],
            []
        )
        .is_err()
    );
}

fn corruption_fixture(kind: &str) -> serde_json::Value {
    use icx_core::dag::ToolArtifactNode;
    let mut dag = MerkleContextDag::new();
    if kind == "ToolArtifact" {
        dag.append_tool_artifact(ToolArtifactNode {
            id: ArtifactId(0),
            dag_digest: icx_core::Blake3Digest::hash(b"ignored"),
            raw_artifact: ArtifactId(1),
            full_digest: icx_core::Blake3Digest::hash(b"raw"),
            agent: AgentKind::Codex,
            session_id: "session".into(),
            tool_name: "Bash".into(),
            exit_code: Some(0),
            total_bytes: 3,
            total_lines: 1,
            chunks: vec![],
            line_index: vec![],
            ahtl_stub: "tool".into(),
            surprise_score: 0.2,
            created_at_ms: 1,
            token_estimate: 4,
        });
    } else {
        dag.begin_branch("work");
        let turn = dag.append_turn(
            AgentKind::Codex,
            "user",
            ArtifactId(1),
            "original".into(),
            0.2,
            4,
        );
        if kind == "SubTrajectoryBranch" {
            dag.fold_branch("done").unwrap();
        } else {
            let epoch = dag.seal_epoch(7, vec![turn], 4);
            dag.fold_epochs(&[epoch.id()]).unwrap();
        }
    }
    assert!(dag.verify_all_merkle_integrity().is_ok());
    serde_json::to_value(dag).unwrap()
}

#[test]
fn tool_branch_and_folded_epoch_reject_independent_corruption() {
    for kind in ["ToolArtifact", "SubTrajectoryBranch", "CacheEpoch"] {
        let original = corruption_fixture(kind);
        let nodes = original["nodes"].as_object().unwrap();
        let key = nodes
            .iter()
            .find(|(_, n)| {
                n.get(kind).is_some()
                    && (kind != "CacheEpoch"
                        || n[kind]["ahtl_frame"].as_str().unwrap().contains("│folded="))
            })
            .unwrap()
            .0
            .clone();
        let digest_field = if kind == "ToolArtifact" {
            "dag_digest"
        } else if kind == "CacheEpoch" {
            "merkle_root"
        } else {
            "digest"
        };
        let content_field = if kind == "ToolArtifact" {
            "ahtl_stub"
        } else if kind == "CacheEpoch" {
            "ahtl_frame"
        } else {
            "folded_ahtl_commit"
        };
        let token_field = if kind == "ToolArtifact" {
            "token_estimate"
        } else if kind == "CacheEpoch" {
            "raw_tokens"
        } else {
            "raw_tokens_saved"
        };
        for mutation in [
            "digest",
            "map_key",
            "matching_ids",
            "content",
            "tokens",
            "epoch_index",
            "compressed_tokens",
            "prefix",
            "suffix",
        ] {
            if kind != "SubTrajectoryBranch" && ["prefix", "suffix"].contains(&mutation) {
                continue;
            }
            if kind != "CacheEpoch" && ["epoch_index", "compressed_tokens"].contains(&mutation) {
                continue;
            }
            let mut value = original.clone();
            let map = value["nodes"].as_object_mut().unwrap();
            let mut node = map.remove(&key).unwrap();
            let fields = &mut node[kind];
            let mut replacement_key = key.clone();
            match mutation {
                "prefix" => {
                    let text = fields[content_field].as_str().unwrap();
                    fields[content_field] = text.replacen('⟦', "⟧", 1).into();
                }
                "suffix" => {
                    let text = fields[content_field].as_str().unwrap();
                    fields[content_field] = format!("{}⟦", text.trim_end_matches('⟧')).into();
                }
                "digest" => {
                    let byte = fields[digest_field][31].as_u64().unwrap();
                    fields[digest_field][31] = (byte ^ 1).into();
                }
                "map_key" => replacement_key = u64::MAX.to_string(),
                "matching_ids" => {
                    fields["id"] = u64::MAX.into();
                    replacement_key = u64::MAX.to_string();
                    // The branch embeds its ID in framing. Keep that framing self-consistent
                    // so only the digest-to-ID requirement detects the corruption.
                    if kind == "SubTrajectoryBranch" {
                        let old = fields[content_field].as_str().unwrap();
                        let prefix = old.rsplit_once('↦').unwrap().0;
                        fields[content_field] =
                            format!("{prefix}↦{}⟧", ArtifactId(u64::MAX)).into();
                    }
                }
                "content" => {
                    let text = fields[content_field].as_str().unwrap().to_owned();
                    fields[content_field] = if kind == "SubTrajectoryBranch" {
                        text.replacen("done", "forged", 1)
                    } else {
                        format!("{text}forged")
                    }
                    .into();
                }
                "tokens" => {
                    fields[token_field] = (fields[token_field].as_u64().unwrap() + 1).into();
                }
                "epoch_index" => fields["epoch_index"] = 8.into(),
                "compressed_tokens" => {
                    fields["compressed_tokens"] =
                        (fields["compressed_tokens"].as_u64().unwrap() + 1).into();
                }
                _ => unreachable!(),
            }
            map.insert(replacement_key, node);
            let restored: MerkleContextDag = serde_json::from_value(value).unwrap();
            assert!(
                restored.verify_all_merkle_integrity().is_err(),
                "{kind}: {mutation}"
            );
        }
    }
}

#[test]
fn cas_restore_rejects_independent_metadata_forgery_and_duplicate_records() {
    let mut store = CasStore::new();
    store.put_bytes_checked(b"first\nsecond\n").unwrap();
    let chunks: Vec<_> = store
        .chunk_entries()
        .map(|(id, hash, bytes)| (id, *hash, bytes.to_vec()))
        .collect();
    let original: Vec<_> = store.artifact_entries().cloned().collect();
    assert_eq!(original[0].total_lines, 2);
    for field in 0..5 {
        let mut artifacts = original.clone();
        match field {
            0 => artifacts[0].total_lines = 3,
            1 => artifacts[0].total_bytes += 1,
            2 => artifacts[0].id = ArtifactId(0),
            // Alter only the suffix: the short content handle remains valid, so digest
            // verification must detect this independently of the ID-prefix check.
            3 => artifacts[0].full_digest.0[31] ^= 1,
            _ => artifacts[0].line_index.clear(),
        }
        assert!(CasStore::from_parts(store.chunk_config(), chunks.clone(), artifacts).is_err());
    }
    let mut duplicate_artifacts = original.clone();
    duplicate_artifacts.push(original[0].clone());
    assert!(
        CasStore::from_parts(store.chunk_config(), chunks.clone(), duplicate_artifacts).is_err()
    );
    let mut duplicate_chunks = chunks.clone();
    duplicate_chunks.push(chunks[0].clone());
    assert!(CasStore::from_parts(store.chunk_config(), duplicate_chunks, original).is_err());
}

#[test]
fn cas_rejects_a_single_forged_line_offset_and_deduplicates_real_chunks() {
    let mut store = CasStore::new();
    let bytes = b"first line\nsecond line\n";
    store.put_bytes_checked(bytes).unwrap();
    assert_eq!(store.unique_chunk_count(), 1);
    store.put_bytes_checked(bytes).unwrap();
    assert_eq!(store.unique_chunk_count(), 1);
    let chunks: Vec<_> = store
        .chunk_entries()
        .map(|(id, hash, bytes)| (id, *hash, bytes.to_vec()))
        .collect();
    let original: Vec<_> = store.artifact_entries().cloned().collect();
    for field in 0..5 {
        let mut artifacts = original.clone();
        let line = &mut artifacts[0].line_index[0];
        match field {
            0 => line.chunk_id = ArtifactId(0),
            1 => line.byte_offset = 1,
            2 => line.byte_len += 1,
            3 => line.start_line += 1,
            _ => line.end_line += 1,
        }
        assert!(CasStore::from_parts(store.chunk_config(), chunks.clone(), artifacts).is_err());
    }
}

#[test]
fn self_consistent_map_and_node_ids_must_still_match_the_digest_prefix() {
    for kind in ["Turn", "CacheEpoch"] {
        let mut dag = MerkleContextDag::new();
        let turn = dag.append_turn(
            AgentKind::Codex,
            "user",
            ArtifactId(1),
            "original".into(),
            0.2,
            4,
        );
        if kind == "CacheEpoch" {
            dag.seal_epoch(0, vec![turn], 4);
        }
        let mut value = serde_json::to_value(&dag).unwrap();
        let nodes = value["nodes"].as_object_mut().unwrap();
        let key = nodes
            .iter()
            .find(|(_, node)| node.get(kind).is_some())
            .unwrap()
            .0
            .clone();
        let mut node = nodes.remove(&key).unwrap();
        node[kind]["id"] = u64::MAX.into();
        nodes.insert(u64::MAX.to_string(), node);
        let restored: MerkleContextDag = serde_json::from_value(value).unwrap();
        assert!(restored.verify_all_merkle_integrity().is_err(), "{kind}");
    }
}

#[test]
fn individual_turn_and_epoch_digest_tampering_cannot_pass_verification() {
    let mut dag = MerkleContextDag::new();
    let turn = dag.append_turn(
        AgentKind::Codex,
        "user",
        ArtifactId(1),
        "original".into(),
        0.2,
        4,
    );
    dag.seal_epoch(0, vec![turn], 4);
    let original = serde_json::to_value(&dag).unwrap();
    for kind in ["Turn", "CacheEpoch"] {
        let mut tampered = original.clone();
        let node = tampered["nodes"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .find(|node| node.get(kind).is_some())
            .unwrap();
        if kind == "Turn" {
            node[kind]["ahtl_repr"] = "forged".into();
        } else {
            let byte = node[kind]["merkle_root"][0].as_u64().unwrap();
            node[kind]["merkle_root"][0] = (byte ^ 1).into();
        }
        let restored: MerkleContextDag = serde_json::from_value(tampered).unwrap();
        assert!(
            restored.verify_all_merkle_integrity_with_cas(None).is_err(),
            "{kind}"
        );
    }
    // Direct public serde restoration skips the optional lookup cache; fallback still obeys
    // role/agent/artifact identity, rather than treating every missing cache as no match.
    let restored: MerkleContextDag = serde_json::from_value(original).unwrap();
    assert_eq!(
        restored
            .find_turn(AgentKind::Codex, "user", ArtifactId(1))
            .unwrap()
            .0,
        "original"
    );
    assert!(
        restored
            .find_turn(AgentKind::ClaudeCode, "user", ArtifactId(1))
            .is_none()
    );
}
