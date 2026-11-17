//! Bi-Temporal Knowledge Graph (`arXiv:2501.13956`) + HippoRAG Personalized PageRank (`PPR`,
//! `arXiv:2405.14831`) for multi-hop structural associative recall and automatic contradiction
//! invalidation.

use icx_core::ArtifactId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A directed edge in the `BiTemporalGraph` carrying valid-time and transaction-time intervals.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BiTemporalEdge {
    pub edge_id: u64,
    pub source_entity: String,
    pub relation: String,
    pub target_entity: String,
    pub is_functional: bool,
    pub t_valid: u64,
    pub t_invalid: Option<u64>,
    pub t_tx_start: u64,
    pub t_tx_end: Option<u64>,
    pub provenance: ArtifactId,
    pub weight: f32,
}

impl BiTemporalEdge {
    /// Returns true if this edge is active at timestamp `at_ms` (`None` checks current active state).
    pub const fn is_active_at(&self, at_ms: Option<u64>) -> bool {
        match at_ms {
            None => self.t_invalid.is_none(),
            Some(ts) => {
                if ts < self.t_valid {
                    return false;
                }
                match self.t_invalid {
                    Some(inv) => ts < inv,
                    None => true,
                }
            }
        }
    }
}

/// Ranked entity and provenance result from Personalized PageRank (`PPR`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PprRankedNode {
    pub entity: String,
    pub score: f32,
    pub provenance_artifacts: Vec<ArtifactId>,
}

/// Bi-Temporal Knowledge Graph supporting automatic functional contradiction invalidation and
/// HippoRAG Personalized PageRank (`PPR`) traversal.
#[derive(Clone, Debug, Default)]
pub struct BiTemporalGraph {
    edges: Vec<BiTemporalEdge>,
    entities: BTreeSet<String>,
    next_edge_id: u64,
}

impl BiTemporalGraph {
    /// Creates a new empty `BiTemporalGraph`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Total number of historical + active edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Total number of active (`t_invalid == None`) edges in the graph.
    pub fn active_edge_count(&self) -> usize {
        self.edges.iter().filter(|e| e.t_invalid.is_none()).count()
    }

    /// Inserts or updates a relational edge. If `is_functional` is true, any existing active edge
    /// with the same `(source_entity, relation)` and a different `target_entity` is automatically
    /// invalidated by stamping `t_invalid = Some(now_ms)` and `t_tx_end = Some(now_ms)`.
    pub fn upsert_edge(
        &mut self,
        source_entity: &str,
        relation: &str,
        target_entity: &str,
        is_functional: bool,
        now_ms: u64,
        provenance: ArtifactId,
        weight: f32,
    ) -> u64 {
        let src = source_entity.trim().to_owned();
        let rel = relation.trim().to_owned();
        let dst = target_entity.trim().to_owned();

        if is_functional {
            for e in &mut self.edges {
                if e.t_invalid.is_none()
                    && e.source_entity == src
                    && e.relation == rel
                    && e.target_entity != dst
                {
                    e.t_invalid = Some(now_ms);
                    e.t_tx_end = Some(now_ms);
                }
            }
        }

        self.entities.insert(src.clone());
        self.entities.insert(dst.clone());

        let edge_id = self.next_edge_id;
        self.next_edge_id += 1;

        self.edges.push(BiTemporalEdge {
            edge_id,
            source_entity: src,
            relation: rel,
            target_entity: dst,
            is_functional,
            t_valid: now_ms,
            t_invalid: None,
            t_tx_start: now_ms,
            t_tx_end: None,
            provenance,
            weight: weight.max(0.01),
        });

        edge_id
    }

    /// Queries edges leaving `source_entity` valid at `at_ms` (`None` = current active edges).
    pub fn query_edges_from(
        &self,
        source_entity: &str,
        at_ms: Option<u64>,
    ) -> Vec<&BiTemporalEdge> {
        self.edges
            .iter()
            .filter(|e| e.source_entity == source_entity && e.is_active_at(at_ms))
            .collect()
    }

    /// Runs HippoRAG Personalized PageRank (`arXiv:2405.14831`) seeded on `seed_entities` over
    /// edges valid at `at_ms` (`None` = currently valid edges).
    pub fn personalized_pagerank(
        &self,
        seed_entities: &[&str],
        at_ms: Option<u64>,
        alpha: f32,
        iterations: usize,
        top_k: usize,
    ) -> Vec<PprRankedNode> {
        if self.entities.is_empty() || seed_entities.is_empty() {
            return Vec::new();
        }

        let nodes: Vec<String> = self.entities.iter().cloned().collect();
        let node_to_idx: BTreeMap<&str, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, s)| (s.as_str(), i))
            .collect();
        let n = nodes.len();

        let mut teleport = vec![0.0f32; n];
        let mut active_seeds = 0usize;
        for &seed in seed_entities {
            for (entity_str, &idx) in &node_to_idx {
                if *entity_str == seed || entity_str.contains(seed) {
                    teleport[idx] += 1.0;
                    active_seeds += 1;
                }
            }
        }
        if active_seeds == 0 {
            return Vec::new();
        }
        let seed_sum: f32 = teleport.iter().sum();
        for v in &mut teleport {
            *v /= seed_sum;
        }

        // Build bidirectional weighted adjacency list over valid edges
        let mut adj: Vec<Vec<(usize, f32)>> = vec![Vec::new(); n];
        let mut provenance_map: Vec<BTreeSet<ArtifactId>> = vec![BTreeSet::new(); n];

        for e in &self.edges {
            if !e.is_active_at(at_ms) {
                continue;
            }
            if let (Some(&u), Some(&v)) = (
                node_to_idx.get(e.source_entity.as_str()),
                node_to_idx.get(e.target_entity.as_str()),
            ) {
                adj[u].push((v, e.weight));
                adj[v].push((u, e.weight * 0.5));
                provenance_map[u].insert(e.provenance);
                provenance_map[v].insert(e.provenance);
            }
        }

        let restart = alpha.clamp(0.05, 0.50);
        let mut rank = teleport.clone();
        let mut next_rank = vec![0.0f32; n];

        for _ in 0..iterations.max(1) {
            for i in 0..n {
                next_rank[i] = restart * teleport[i];
            }
            for u in 0..n {
                let out_weight_sum: f32 = adj[u].iter().map(|(_, w)| *w).sum();
                if out_weight_sum > 0.0 {
                    let mass = (1.0 - restart) * rank[u] / out_weight_sum;
                    for &(v, w) in &adj[u] {
                        next_rank[v] += mass * w;
                    }
                } else {
                    // Dangling node redistributes to teleport vector
                    for i in 0..n {
                        next_rank[i] += (1.0 - restart) * rank[u] * teleport[i];
                    }
                }
            }
            rank.copy_from_slice(&next_rank);
        }

        let mut results: Vec<PprRankedNode> = (0..n)
            .filter(|&i| rank[i] > 1e-5)
            .map(|i| PprRankedNode {
                entity: nodes[i].clone(),
                score: rank[i],
                provenance_artifacts: provenance_map[i].iter().copied().collect(),
            })
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(core::cmp::Ordering::Equal)
        });
        results.truncate(top_k);
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitemporal_graph_invalidates_functional_contradictions_and_walks_multi_hop_ppr() {
        let mut g = BiTemporalGraph::new();
        let (art_fail, _) = ArtifactId::hash_bytes(b"fail_log");
        let (art_pass, _) = ArtifactId::hash_bytes(b"pass_log");
        let (art_dep, _) = ArtifactId::hash_bytes(b"dep_edge");

        // At t=100, allocator::alloc_page has status FAIL
        g.upsert_edge(
            "fn:alloc_page",
            "status",
            "FAIL_ALIGN",
            true,
            100,
            art_fail,
            1.0,
        );
        // At t=200, allocator::alloc_page is fixed -> status PASS_ALIGN
        g.upsert_edge(
            "fn:alloc_page",
            "status",
            "PASS_ALIGN",
            true,
            200,
            art_pass,
            1.0,
        );
        // Multi-hop chain: mod:mmu -> trait:Pager -> fn:alloc_page
        g.upsert_edge(
            "mod:mmu",
            "uses_trait",
            "trait:Pager",
            false,
            200,
            art_dep,
            1.0,
        );
        g.upsert_edge(
            "trait:Pager",
            "calls",
            "fn:alloc_page",
            false,
            200,
            art_pass,
            1.0,
        );

        // Time-travel at t=150 sees FAIL_ALIGN; current sees PASS_ALIGN
        let at_150 = g.query_edges_from("fn:alloc_page", Some(150));
        assert_eq!(at_150.len(), 1);
        assert_eq!(at_150[0].target_entity, "FAIL_ALIGN");

        let current = g.query_edges_from("fn:alloc_page", None);
        assert_eq!(current.len(), 1);
        assert_eq!(current[0].target_entity, "PASS_ALIGN");

        // PPR seeded on `mod:mmu` reaches `fn:alloc_page` and `PASS_ALIGN`, NOT invalidated `FAIL_ALIGN`
        let ppr = g.personalized_pagerank(&["mod:mmu"], None, 0.15, 8, 10);
        let names: Vec<&str> = ppr.iter().map(|n| n.entity.as_str()).collect();
        assert!(names.contains(&"fn:alloc_page"));
        assert!(names.contains(&"PASS_ALIGN"));
        assert!(!names.contains(&"FAIL_ALIGN"));
    }
}
