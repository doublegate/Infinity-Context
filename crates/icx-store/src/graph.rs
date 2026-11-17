//! Bi-Temporal Knowledge Graph (`arXiv:2501.13956`) + HippoRAG Personalized PageRank (`PPR`,
//! `arXiv:2405.14831`) for multi-hop structural associative recall and automatic contradiction
//! invalidation.

use icx_core::{ArtifactId, IcxError};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static GRAPH_TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

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

/// Compressed Sparse Row (`CSR`) snapshot of valid edges at a point in time `at_ms`,
/// enabling cache-contiguous $O(V + E)$ HippoRAG Personalized PageRank (`PPR`) power iteration.
#[derive(Clone, Debug, PartialEq)]
pub struct CsrGraphSnapshot {
    nodes: Vec<String>,
    row_ptr: Vec<usize>,
    col_idx: Vec<usize>,
    weights: Vec<f32>,
    inv_out_weight: Vec<f32>,
    prov_row_ptr: Vec<usize>,
    prov_ids: Vec<ArtifactId>,
}

impl CsrGraphSnapshot {
    /// Number of distinct active entity vertices in the CSR snapshot.
    #[inline]
    pub const fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Number of directed CSR adjacency entries (forward + reverse weighted transitions).
    #[inline]
    pub const fn directed_edge_count(&self) -> usize {
        self.col_idx.len()
    }

    /// Read-only slice of entity names in CSR vertex index order.
    #[inline]
    pub fn nodes(&self) -> &[String] {
        &self.nodes
    }

    /// Read-only CSR row pointer array (`len == node_count() + 1`).
    #[inline]
    pub fn row_ptr(&self) -> &[usize] {
        &self.row_ptr
    }

    /// Read-only CSR column index array (`len == directed_edge_count()`).
    #[inline]
    pub fn col_idx(&self) -> &[usize] {
        &self.col_idx
    }

    /// Read-only CSR edge weight array (`len == directed_edge_count()`).
    #[inline]
    pub fn weights(&self) -> &[f32] {
        &self.weights
    }

    /// Read-only per-vertex inverse outgoing weight array (`len == node_count()`).
    #[inline]
    pub fn inv_out_weight(&self) -> &[f32] {
        &self.inv_out_weight
    }

    /// Read-only provenance row pointer array (`len == node_count() + 1`).
    #[inline]
    pub fn prov_row_ptr(&self) -> &[usize] {
        &self.prov_row_ptr
    }

    /// Read-only flattened provenance `ArtifactId` array.
    #[inline]
    pub fn prov_ids(&self) -> &[ArtifactId] {
        &self.prov_ids
    }

    /// Returns the provenance `ArtifactId` slice associated with node index `node_idx`.
    #[inline]
    pub fn node_provenance(&self, node_idx: usize) -> &[ArtifactId] {
        if node_idx + 1 >= self.prov_row_ptr.len() {
            return &[];
        }
        let start = self.prov_row_ptr[node_idx];
        let end = self.prov_row_ptr[node_idx + 1];
        self.prov_ids.get(start..end).unwrap_or(&[])
    }

    /// Executes HippoRAG Personalized PageRank (`arXiv:2405.14831`) power iteration over the
    /// contiguous CSR arrays in $O(\text{iterations} \cdot (V + E))$ time.
    pub fn run_ppr(
        &self,
        seed_entities: &[&str],
        alpha: f32,
        iterations: usize,
        top_k: usize,
    ) -> Vec<PprRankedNode> {
        let n = self.nodes.len();
        if n == 0 || seed_entities.is_empty() || top_k == 0 {
            return Vec::new();
        }

        let mut teleport = vec![0.0f32; n];
        let mut active_seeds = 0usize;
        for &raw_seed in seed_entities {
            let seed = raw_seed.trim();
            if seed.is_empty() {
                continue;
            }
            for (idx, entity_str) in self.nodes.iter().enumerate() {
                if entity_str == seed {
                    teleport[idx] += 1.0;
                    active_seeds += 1;
                } else if entity_str.contains(seed) {
                    teleport[idx] += 0.25;
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

        let restart = if alpha.is_nan() {
            0.15
        } else {
            alpha.clamp(0.05, 0.50)
        };
        let mut rank = teleport.clone();
        let mut next_rank = vec![0.0f32; n];

        for _ in 0..iterations.max(1) {
            for i in 0..n {
                next_rank[i] = restart * teleport[i];
            }
            for u in 0..n {
                let mass = (1.0 - restart) * rank[u] * self.inv_out_weight[u];
                let start = self.row_ptr[u];
                let end = self.row_ptr[u + 1];
                for idx in start..end {
                    next_rank[self.col_idx[idx]] += mass * self.weights[idx];
                }
            }
            rank.copy_from_slice(&next_rank);
        }

        let mut results: Vec<PprRankedNode> = (0..n)
            .filter(|&i| rank[i] > 1e-5)
            .map(|i| PprRankedNode {
                entity: self.nodes[i].clone(),
                score: rank[i],
                provenance_artifacts: self.node_provenance(i).to_vec(),
            })
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(core::cmp::Ordering::Equal)
                .then_with(|| a.entity.cmp(&b.entity))
        });
        results.truncate(top_k);
        results
    }
}

/// Bi-Temporal Knowledge Graph supporting automatic functional contradiction invalidation and
/// HippoRAG Personalized PageRank (`PPR`) traversal over a Compressed Sparse Row (`CSR`) snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BiTemporalGraph {
    edges: Vec<BiTemporalEdge>,
    entities: BTreeSet<String>,
    next_edge_id: u64,
}

impl Default for BiTemporalGraph {
    fn default() -> Self {
        Self {
            edges: Vec::new(),
            entities: BTreeSet::new(),
            next_edge_id: 1,
        }
    }
}

impl BiTemporalGraph {
    /// Creates a new empty `BiTemporalGraph`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Total number of distinct entities in the graph.
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    /// Returns a slice of all edges (active and historical) in the graph.
    pub fn edges(&self) -> &[BiTemporalEdge] {
        &self.edges
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
    /// with the same `(source_entity, relation)` is automatically superseded by stamping
    /// `t_invalid = Some(now_ms)` and `t_tx_end = Some(now_ms)`, guaranteeing at most one active
    /// edge per functional `(source_entity, relation)` pair.
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
        if src.is_empty() || rel.is_empty() || dst.is_empty() {
            return 0;
        }

        let mut effective_valid_ms = now_ms;
        let mut effective_tx_ms = now_ms;
        if is_functional {
            for e in &self.edges {
                if e.is_functional && e.source_entity == src && e.relation == rel {
                    let req_valid = e.t_invalid.unwrap_or_else(|| e.t_valid.saturating_add(1));
                    let req_tx = e.t_tx_end.unwrap_or_else(|| e.t_tx_start.saturating_add(1));
                    effective_valid_ms = effective_valid_ms.max(req_valid);
                    effective_tx_ms = effective_tx_ms.max(req_tx);
                }
            }
            for e in &mut self.edges {
                if e.is_functional
                    && e.t_invalid.is_none()
                    && e.source_entity == src
                    && e.relation == rel
                {
                    e.t_invalid = Some(effective_valid_ms);
                    e.t_tx_end = Some(effective_tx_ms);
                }
            }
        }

        self.entities.insert(src.clone());
        self.entities.insert(dst.clone());

        let edge_id = self.next_edge_id.max(1);
        self.next_edge_id = edge_id + 1;

        let clean_weight = if weight.is_finite() {
            weight.clamp(0.01, 1_000.0)
        } else {
            1.0
        };

        self.edges.push(BiTemporalEdge {
            edge_id,
            source_entity: src,
            relation: rel,
            target_entity: dst,
            is_functional,
            t_valid: effective_valid_ms,
            t_invalid: None,
            t_tx_start: effective_tx_ms,
            t_tx_end: None,
            provenance,
            weight: clean_weight,
        });

        edge_id
    }

    /// Explicitly invalidates an active edge by `edge_id`, stamping `t_invalid = Some(now_ms)`
    /// and `t_tx_end = Some(now_ms)`. Returns `true` if an active edge was found and invalidated.
    pub fn invalidate_edge(&mut self, edge_id: u64, now_ms: u64) -> bool {
        for e in &mut self.edges {
            if e.edge_id == edge_id && e.t_invalid.is_none() {
                e.t_invalid = Some(now_ms.max(e.t_valid.saturating_add(1)));
                e.t_tx_end = Some(now_ms.max(e.t_tx_start.saturating_add(1)));
                return true;
            }
        }
        false
    }

    /// Queries edges leaving `source_entity` valid at `at_ms` (`None` = current active edges).
    pub fn query_edges_from(
        &self,
        source_entity: &str,
        at_ms: Option<u64>,
    ) -> Vec<&BiTemporalEdge> {
        let src = source_entity.trim();
        self.edges
            .iter()
            .filter(|e| e.source_entity == src && e.is_active_at(at_ms))
            .collect()
    }

    /// Queries edges entering `target_entity` valid at `at_ms` (`None` = current active edges).
    pub fn query_edges_to(&self, target_entity: &str, at_ms: Option<u64>) -> Vec<&BiTemporalEdge> {
        let dst = target_entity.trim();
        self.edges
            .iter()
            .filter(|e| e.target_entity == dst && e.is_active_at(at_ms))
            .collect()
    }

    /// Queries all edges with a given `relation` valid at `at_ms` (`None` = current active edges).
    pub fn query_relation(&self, relation: &str, at_ms: Option<u64>) -> Vec<&BiTemporalEdge> {
        let rel = relation.trim();
        self.edges
            .iter()
            .filter(|e| e.relation == rel && e.is_active_at(at_ms))
            .collect()
    }

    /// Compiles a contiguous Compressed Sparse Row (`CSR`) snapshot of all edges valid at `at_ms`.
    /// Only entities incident to at least one edge active at `at_ms` are included in the snapshot,
    /// ensuring invalidated historical entities cannot be seeded or returned with empty provenance.
    pub fn build_csr(&self, at_ms: Option<u64>) -> CsrGraphSnapshot {
        let mut active_entities = BTreeSet::new();
        for e in &self.edges {
            if e.is_active_at(at_ms) {
                active_entities.insert(e.source_entity.clone());
                active_entities.insert(e.target_entity.clone());
            }
        }
        let nodes: Vec<String> = active_entities.into_iter().collect();
        let node_to_idx: BTreeMap<&str, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, s)| (s.as_str(), i))
            .collect();
        let n = nodes.len();

        let mut adj: Vec<Vec<(usize, f32)>> = vec![Vec::new(); n];
        let mut provenance_map: Vec<BTreeSet<ArtifactId>> = vec![BTreeSet::new(); n];

        for e in &self.edges {
            if !e.is_active_at(at_ms) {
                continue;
            }
            let u = node_to_idx[e.source_entity.as_str()];
            let v = node_to_idx[e.target_entity.as_str()];
            adj[u].push((v, e.weight));
            adj[v].push((u, e.weight * 0.5));
            provenance_map[u].insert(e.provenance);
            provenance_map[v].insert(e.provenance);
        }

        let total_edges: usize = adj.iter().map(Vec::len).sum();
        let total_prov: usize = provenance_map.iter().map(BTreeSet::len).sum();

        let mut row_ptr = Vec::with_capacity(n + 1);
        let mut col_idx = Vec::with_capacity(total_edges);
        let mut weights = Vec::with_capacity(total_edges);
        let mut inv_out_weight = Vec::with_capacity(n);

        let mut prov_row_ptr = Vec::with_capacity(n + 1);
        let mut prov_ids = Vec::with_capacity(total_prov);

        row_ptr.push(0);
        prov_row_ptr.push(0);

        for u in 0..n {
            let mut sum_w = 0.0f32;
            for &(v, w) in &adj[u] {
                col_idx.push(v);
                weights.push(w);
                sum_w += w;
            }
            row_ptr.push(col_idx.len());
            inv_out_weight.push(if sum_w > 0.0 { 1.0 / sum_w } else { 0.0 });

            for &aid in &provenance_map[u] {
                prov_ids.push(aid);
            }
            prov_row_ptr.push(prov_ids.len());
        }

        CsrGraphSnapshot {
            nodes,
            row_ptr,
            col_idx,
            weights,
            inv_out_weight,
            prov_row_ptr,
            prov_ids,
        }
    }

    /// Runs HippoRAG Personalized PageRank (`arXiv:2405.14831`) seeded on `seed_entities` over
    /// edges valid at `at_ms` (`None` = currently valid edges) via a CSR snapshot.
    pub fn personalized_pagerank(
        &self,
        seed_entities: &[&str],
        at_ms: Option<u64>,
        alpha: f32,
        iterations: usize,
        top_k: usize,
    ) -> Vec<PprRankedNode> {
        if self.entities.is_empty() || seed_entities.is_empty() || top_k == 0 {
            return Vec::new();
        }
        self.build_csr(at_ms)
            .run_ppr(seed_entities, alpha, iterations, top_k)
    }

    /// Atomically persists the `BiTemporalGraph` to `<dir>/bitemporal_graph.json`.
    pub fn save_to_dir(&self, dir: &Path) -> Result<(), IcxError> {
        fs::create_dir_all(dir).map_err(|e| IcxError::Io(e.to_string()))?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| IcxError::Io(e.to_string()))?;
        let seq = GRAPH_TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let tmp_path = dir.join(format!(
            "bitemporal_graph.json.{}.{}.tmp",
            std::process::id(),
            seq
        ));
        let final_path = dir.join("bitemporal_graph.json");
        let write_res = (|| -> std::io::Result<()> {
            let mut opts = OpenOptions::new();
            opts.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(0o600);
            }
            let mut f = opts.open(&tmp_path)?;
            f.write_all(&bytes)?;
            f.sync_all()?;
            drop(f);
            fs::rename(&tmp_path, &final_path)
        })();
        if let Err(e) = write_res {
            let _ = fs::remove_file(&tmp_path);
            return Err(IcxError::Io(e.to_string()));
        }
        Ok(())
    }

    /// Loads and validates a persisted `BiTemporalGraph` from `<dir>/bitemporal_graph.json`.
    pub fn load_from_dir(dir: &Path) -> Result<Self, IcxError> {
        let path = dir.join("bitemporal_graph.json");
        let bytes = fs::read(&path).map_err(|e| IcxError::Io(e.to_string()))?;
        let mut loaded: Self =
            serde_json::from_slice(&bytes).map_err(|e| IcxError::Io(e.to_string()))?;
        loaded.entities.clear();
        loaded.edges.retain(|e| {
            !e.source_entity.trim().is_empty()
                && !e.relation.trim().is_empty()
                && !e.target_entity.trim().is_empty()
        });
        let mut seen_ids = BTreeSet::new();
        let mut functional_groups: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();
        let mut max_next_id = loaded.next_edge_id.max(1);

        for idx in 0..loaded.edges.len() {
            let edge_id = loaded.edges[idx].edge_id;
            if !seen_ids.insert(edge_id) {
                return Err(IcxError::Io(format!(
                    "duplicate edge_id {edge_id} in bitemporal_graph.json"
                )));
            }
            let src = loaded.edges[idx].source_entity.trim().to_owned();
            let rel = loaded.edges[idx].relation.trim().to_owned();
            let dst = loaded.edges[idx].target_entity.trim().to_owned();
            loaded.edges[idx].source_entity.clone_from(&src);
            loaded.edges[idx].relation.clone_from(&rel);
            loaded.edges[idx].target_entity.clone_from(&dst);
            loaded.entities.insert(src.clone());
            loaded.entities.insert(dst);

            let t_valid = loaded.edges[idx].t_valid;
            let t_tx_start = loaded.edges[idx].t_tx_start;
            if let Some(inv) = loaded.edges[idx].t_invalid {
                let clamped_inv = inv.max(t_valid);
                loaded.edges[idx].t_invalid = Some(clamped_inv);
                let tx_end = loaded.edges[idx]
                    .t_tx_end
                    .unwrap_or(clamped_inv)
                    .max(t_tx_start);
                loaded.edges[idx].t_tx_end = Some(tx_end);
            } else {
                loaded.edges[idx].t_tx_end = None;
            }
            if loaded.edges[idx].is_functional {
                functional_groups.entry((src, rel)).or_default().push(idx);
            }

            let w = loaded.edges[idx].weight;
            loaded.edges[idx].weight = w.clamp(0.01, 1_000.0);
            max_next_id = max_next_id.max(edge_id.saturating_add(1));
        }

        for indices in functional_groups.values() {
            if indices.len() <= 1 {
                continue;
            }
            let mut active: Vec<usize> = indices
                .iter()
                .copied()
                .filter(|&i| loaded.edges[i].t_invalid.is_none())
                .collect();
            active.sort_by_key(|&i| {
                (
                    loaded.edges[i].t_valid,
                    loaded.edges[i].t_tx_start,
                    loaded.edges[i].edge_id,
                )
            });
            for pos in 0..active.len() {
                let cur_i = active[pos];
                if pos > 0 {
                    let prev_i = active[pos - 1];
                    let prev_inv = loaded.edges[prev_i]
                        .t_invalid
                        .unwrap_or(loaded.edges[prev_i].t_valid);
                    loaded.edges[cur_i].t_valid = loaded.edges[cur_i].t_valid.max(prev_inv);
                    let prev_tx_end = loaded.edges[prev_i]
                        .t_tx_end
                        .unwrap_or(loaded.edges[prev_i].t_tx_start);
                    loaded.edges[cur_i].t_tx_start =
                        loaded.edges[cur_i].t_tx_start.max(prev_tx_end);
                }
                if pos + 1 < active.len() {
                    let next_valid = loaded.edges[active[pos + 1]].t_valid;
                    let next_tx = loaded.edges[active[pos + 1]].t_tx_start;
                    loaded.edges[cur_i].t_invalid =
                        Some(next_valid.max(loaded.edges[cur_i].t_valid.saturating_add(1)));
                    loaded.edges[cur_i].t_tx_end =
                        Some(next_tx.max(loaded.edges[cur_i].t_tx_start.saturating_add(1)));
                }
            }
            let winner_idx = active.last().copied();
            if let Some(w_idx) = winner_idx {
                let w_edge_id = loaded.edges[w_idx].edge_id;
                for &ci in indices {
                    if let Some(inv) = loaded.edges[ci]
                        .t_invalid
                        .filter(|&inv| inv > loaded.edges[ci].t_valid)
                    {
                        if loaded.edges[ci].edge_id < w_edge_id {
                            loaded.edges[w_idx].t_valid = loaded.edges[w_idx].t_valid.max(inv);
                            let tx_end = loaded.edges[ci].t_tx_end.unwrap_or(inv);
                            loaded.edges[w_idx].t_tx_start =
                                loaded.edges[w_idx].t_tx_start.max(tx_end);
                        }
                    }
                }
            }
            let mut ordered = indices.clone();
            ordered.sort_by_key(|&i| {
                (
                    loaded.edges[i].t_valid,
                    loaded.edges[i].t_tx_start,
                    loaded.edges[i].edge_id,
                )
            });
            for (pos, &ci) in ordered.iter().enumerate() {
                let Some(mut inv) = loaded.edges[ci]
                    .t_invalid
                    .filter(|_| Some(ci) != winner_idx)
                else {
                    continue;
                };
                let mut tx_end = loaded.edges[ci].t_tx_end.unwrap_or(inv);
                if let Some(w_idx) = winner_idx {
                    inv = inv.min(loaded.edges[w_idx].t_valid);
                    tx_end = tx_end.min(loaded.edges[w_idx].t_tx_start);
                }
                let next_bound = ordered[pos + 1..]
                    .iter()
                    .copied()
                    .find(|&ni| loaded.edges[ni].t_invalid != Some(loaded.edges[ni].t_valid));
                inv = inv.min(next_bound.map_or(inv, |ni| loaded.edges[ni].t_valid));
                tx_end = tx_end.min(next_bound.map_or(tx_end, |ni| loaded.edges[ni].t_tx_start));
                loaded.edges[ci].t_invalid = Some(inv.max(loaded.edges[ci].t_valid));
                loaded.edges[ci].t_tx_end = Some(tx_end.max(loaded.edges[ci].t_tx_start));
            }
        }

        loaded.next_edge_id = max_next_id.max(1);
        Ok(loaded)
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

        // Re-upserting a functional relation with the SAME target_entity supersedes the old edge
        // so at most one active edge exists for `(fn:alloc_page, status)`
        let e_same = g.upsert_edge(
            "fn:alloc_page",
            "status",
            "PASS_ALIGN",
            true,
            300,
            art_pass,
            f32::NAN,
        );
        let current_after = g.query_edges_from("fn:alloc_page", None);
        assert_eq!(current_after.len(), 1);
        assert_eq!(current_after[0].edge_id, e_same);
        assert_eq!(current_after[0].t_valid, 300);

        // Query incoming edges and by relation
        let incoming = g.query_edges_to("PASS_ALIGN", None);
        assert_eq!(incoming.len(), 1);
        let by_rel = g.query_relation("status", None);
        assert_eq!(by_rel.len(), 1);
        assert_eq!(g.entity_count(), 5);
        assert_eq!(g.edge_count(), 5);
        assert_eq!(g.active_edge_count(), 3);
        assert_eq!(g.edges().len(), 5);

        // Inspect CSR snapshot (invalidated `FAIL_ALIGN` has no active edges at `None` and is excluded)
        let csr = g.build_csr(None);
        assert_eq!(csr.node_count(), 4);
        assert_eq!(csr.directed_edge_count(), 6);
        assert_eq!(csr.nodes().len(), 4);
        assert_eq!(csr.row_ptr().len(), 5);
        assert_eq!(csr.col_idx().len(), 6);
        assert_eq!(csr.weights().len(), 6);
        assert_eq!(csr.inv_out_weight().len(), 4);
        assert_eq!(csr.prov_row_ptr().len(), 5);
        assert_ne!(csr.prov_ids().len(), 0);
        assert_eq!(csr.node_provenance(999).len(), 0);
        assert_eq!(csr.run_ppr(&["", "FAIL_ALIGN"], f32::NAN, 4, 5).len(), 0);
        let ppr_sub = csr.run_ppr(&["  ", "Pager"], f32::NAN, 4, 5);
        assert_ne!(ppr_sub.len(), 0);
        assert_eq!(csr.run_ppr(&["nonexistent_seed"], 0.15, 4, 5).len(), 0);
        assert_eq!(csr.run_ppr(&["mod:mmu"], 0.15, 4, 0).len(), 0);
        assert_eq!(
            g.personalized_pagerank(&["mod:mmu"], None, 0.15, 4, 0)
                .len(),
            0
        );

        // Explicit edge invalidation (including non-monotonic timestamp clamping to t_valid + 1)
        assert!(!g.invalidate_edge(0, 250));
        assert!(g.invalidate_edge(e_same, 250));
        assert_eq!(
            g.edges()
                .iter()
                .find(|e| e.edge_id == e_same)
                .and_then(|e| e.t_invalid),
            Some(301)
        );
        assert!(!g.invalidate_edge(e_same, 450));
        assert!(!g.invalidate_edge(999_999, 450));
        assert_eq!(g.query_edges_from("fn:alloc_page", Some(50)).len(), 0);

        // Atomic save_to_dir / load_from_dir round-trip and validation error paths
        let seq = GRAPH_TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let tmp_dir =
            std::env::temp_dir().join(format!("icx_graph_test_{}_{}", std::process::id(), seq));
        let _ = fs::remove_dir_all(&tmp_dir);
        g.save_to_dir(&tmp_dir).expect("save_to_dir succeeds");
        let loaded = BiTemporalGraph::load_from_dir(&tmp_dir).expect("load_from_dir succeeds");
        assert_eq!(loaded, g);

        // Verify load_from_dir rejects duplicate edge_id and empty entity names
        let json_path = tmp_dir.join("bitemporal_graph.json");
        fs::write(
            &json_path,
            r#"{"edges":[{"edge_id":0,"source_entity":"a","relation":"r","target_entity":"b","is_functional":true,"t_valid":10,"t_invalid":null,"t_tx_start":10,"t_tx_end":null,"provenance":1,"weight":1.0},{"edge_id":0,"source_entity":"a","relation":"r","target_entity":"c","is_functional":true,"t_valid":20,"t_invalid":null,"t_tx_start":20,"t_tx_end":null,"provenance":1,"weight":1.0}],"entities":[],"next_edge_id":2}"#,
        )
        .unwrap();
        assert!(BiTemporalGraph::load_from_dir(&tmp_dir).is_err());

        // Verify load_from_dir prunes legacy empty-endpoint edges while keeping valid edges intact
        fs::write(
            &json_path,
            r#"{"edges":[{"edge_id":0,"source_entity":" ","relation":"r","target_entity":"b","is_functional":false,"t_valid":10,"t_invalid":null,"t_tx_start":10,"t_tx_end":null,"provenance":1,"weight":1.0},{"edge_id":1,"source_entity":"a","relation":"head","target_entity":"","is_functional":false,"t_valid":10,"t_invalid":null,"t_tx_start":10,"t_tx_end":null,"provenance":1,"weight":1.0},{"edge_id":2,"source_entity":"a","relation":"status","target_entity":"ok","is_functional":true,"t_valid":10,"t_invalid":null,"t_tx_start":10,"t_tx_end":null,"provenance":1,"weight":1.0}],"entities":[],"next_edge_id":3}"#,
        )
        .unwrap();
        let pruned = BiTemporalGraph::load_from_dir(&tmp_dir).expect("prunes legacy empty edges");
        assert_eq!(pruned.edge_count(), 1);
        assert_eq!(pruned.active_edge_count(), 1);

        // Verify load_from_dir advances next_edge_id past legacy 0-based edge_ids and repairs 3 out-of-order functional edges with non-overlapping intervals
        fs::write(
            &json_path,
            r#"{"edges":[{"edge_id":2,"source_entity":"a","relation":"status","target_entity":"NEW","is_functional":true,"t_valid":10,"t_invalid":null,"t_tx_start":10,"t_tx_end":null,"provenance":1,"weight":1.0},{"edge_id":1,"source_entity":"a","relation":"status","target_entity":"MID","is_functional":true,"t_valid":10,"t_invalid":null,"t_tx_start":10,"t_tx_end":null,"provenance":1,"weight":1.0},{"edge_id":0,"source_entity":"a","relation":"status","target_entity":"OLD","is_functional":true,"t_valid":10,"t_invalid":null,"t_tx_start":10,"t_tx_end":null,"provenance":1,"weight":1.0}],"entities":["stale"],"next_edge_id":0}"#,
        )
        .unwrap();
        let mut repaired =
            BiTemporalGraph::load_from_dir(&tmp_dir).expect("repaired load succeeds");
        assert_eq!(repaired.active_edge_count(), 1);
        assert_eq!(repaired.entity_count(), 4);
        assert_eq!(repaired.query_edges_from("a", Some(10)).len(), 1);
        assert_eq!(repaired.query_edges_from("a", Some(11)).len(), 1);
        let active_edge = repaired
            .edges()
            .iter()
            .find(|e| e.t_invalid.is_none())
            .unwrap();
        assert_eq!(active_edge.target_entity, "NEW");
        assert_eq!(active_edge.edge_id, 2);
        assert!(repaired.invalidate_edge(active_edge.edge_id, 50));

        // Verify load_from_dir preserves an active winner whose t_valid (50) precedes an already-invalidated empty edge (100..100)
        // and clamps an earlier closed edge [10, 80) against its successor at 50
        fs::write(
            &json_path,
            r#"{"edges":[{"edge_id":0,"source_entity":"a","relation":"status","target_entity":"OLD_INV","is_functional":true,"t_valid":100,"t_invalid":100,"t_tx_start":100,"t_tx_end":null,"provenance":1,"weight":1.0},{"edge_id":1,"source_entity":"a","relation":"status","target_entity":"ACTIVE_WINNER","is_functional":true,"t_valid":50,"t_invalid":null,"t_tx_start":50,"t_tx_end":null,"provenance":1,"weight":1.0},{"edge_id":2,"source_entity":"a","relation":"status","target_entity":"EARLY_CLOSED","is_functional":true,"t_valid":10,"t_invalid":80,"t_tx_start":10,"t_tx_end":80,"provenance":1,"weight":1.0}],"entities":[],"next_edge_id":0}"#,
        )
        .unwrap();
        let winner_graph =
            BiTemporalGraph::load_from_dir(&tmp_dir).expect("preserves active winner");
        assert_eq!(winner_graph.active_edge_count(), 1);
        assert_eq!(
            winner_graph.query_edges_from("a", Some(50))[0].target_entity,
            "ACTIVE_WINNER"
        );
        assert_eq!(winner_graph.query_edges_from("a", Some(100)).len(), 1);
        assert_eq!(
            winner_graph.query_edges_from("a", Some(100))[0].target_entity,
            "ACTIVE_WINNER"
        );

        // Verify load_from_dir advances an out-of-order active winner past an earlier non-empty closed interval [100, 200)
        // and preserves chronological supersession order when two closed edges share the same t_valid
        fs::write(
            &json_path,
            r#"{"edges":[{"edge_id":1,"source_entity":"a","relation":"status","target_entity":"HIST_SHORT","is_functional":true,"t_valid":100,"t_invalid":150,"t_tx_start":100,"t_tx_end":150,"provenance":1,"weight":1.0},{"edge_id":2,"source_entity":"a","relation":"status","target_entity":"HIST_LONG","is_functional":true,"t_valid":100,"t_invalid":200,"t_tx_start":100,"t_tx_end":200,"provenance":1,"weight":1.0},{"edge_id":3,"source_entity":"a","relation":"status","target_entity":"ACTIVE_LATE","is_functional":true,"t_valid":50,"t_invalid":null,"t_tx_start":50,"t_tx_end":null,"provenance":1,"weight":1.0}],"entities":[],"next_edge_id":4}"#,
        )
        .unwrap();
        let hist_graph = BiTemporalGraph::load_from_dir(&tmp_dir)
            .expect("preserves non-empty historical closed interval");
        assert_eq!(
            hist_graph.query_edges_from("a", Some(175))[0].target_entity,
            "HIST_LONG"
        );
        assert_eq!(
            hist_graph.query_edges_from("a", Some(200))[0].target_entity,
            "ACTIVE_LATE"
        );
        assert_eq!(
            g.upsert_edge(" ", "status", "ok", true, 100, ArtifactId(1), 1.0),
            0
        );
        assert_eq!(
            g.upsert_edge("src", "", "ok", true, 100, ArtifactId(1), 1.0),
            0
        );
        assert_eq!(
            g.upsert_edge("src", "status", "  ", true, 100, ArtifactId(1), 1.0),
            0
        );
        let _ = fs::remove_file(&json_path);
        fs::create_dir_all(&json_path).unwrap();
        assert!(g.save_to_dir(&tmp_dir).is_err());

        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
