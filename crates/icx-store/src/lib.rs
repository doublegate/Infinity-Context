//! `icx-store` — Bi-Temporal CSR Knowledge Graph and HippoRAG Personalized PageRank (`PPR`)
//! multi-hop associative traversal engine.

pub mod graph;

pub use graph::{BiTemporalEdge, BiTemporalGraph, CsrGraphSnapshot, PprRankedNode};
