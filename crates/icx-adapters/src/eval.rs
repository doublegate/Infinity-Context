//! Retrieval evaluation harness (v1.5.0, T-72-002): scores a ranker on a query set by recall@1,
//! recall@5, recall@10 and MRR@10.
//!
//! - **Rankers:** `legacy` is the HDC associative index as recall uses it today; `bm25` is the
//!   lexical index alone; `rrf` fuses the two by reciprocal rank (`lexical::rrf`, k = 60).
//! - **Metrics:** recall@k is the share of queries with a relevant artifact in the top k; MRR@10
//!   is the mean of `1 / rank` of the first relevant artifact, 0 when none is in the top 10.
//!   Repeated artifacts are counted once, at their best rank.
//! - **Workspaces:** the lexical tables are built in a scratch SQLite file, never in a workspace
//!   store, so evaluating a live workspace writes nothing to it.
//! - **Determinism:** the synthetic set is generated from a seed; the same seed gives the same
//!   cases and the same scores.

use crate::InfinityContextMmu;
use icx_core::{AgentKind, ArtifactId, IcxError};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// How deep each ranker's list goes before fusion and scoring.
pub const DEPTH: usize = 50;

/// A ranker under evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Ranker {
    Legacy,
    Bm25,
    Rrf,
}

impl Ranker {
    pub const ALL: [Self; 3] = [Self::Legacy, Self::Bm25, Self::Rrf];

    /// Parses `legacy`, `bm25` or `rrf`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "legacy" => Some(Self::Legacy),
            "bm25" => Some(Self::Bm25),
            "rrf" => Some(Self::Rrf),
            _ => None,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::Bm25 => "bm25",
            Self::Rrf => "rrf",
        }
    }
}

/// One judged query: the artifacts that answer it, and its split (`train` or `heldout`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Case {
    pub query: String,
    /// Relevant artifacts as `#icx:<16 hex>` handles.
    pub relevant: Vec<String>,
    #[serde(default = "default_split")]
    pub split: String,
}

fn default_split() -> String {
    "heldout".to_owned()
}

/// Scores of one ranker over one set.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scores {
    pub ranker: Ranker,
    pub queries: usize,
    pub recall_at_1: f64,
    pub recall_at_5: f64,
    pub recall_at_10: f64,
    pub mrr_at_10: f64,
}

/// Parses `#icx:<16 hex>` (or bare 16 hex digits) into an artifact id.
#[must_use]
pub fn parse_handle(handle: &str) -> Option<ArtifactId> {
    let hex = handle.strip_prefix("#icx:").unwrap_or(handle);
    (hex.len() == 16)
        .then(|| u64::from_str_radix(hex, 16).ok())
        .flatten()
        .map(ArtifactId)
}

/// Builds the lexical tables for every artifact in `mmu`'s CAS into `conn` (a scratch database).
pub fn build_lexical(mmu: &InfinityContextMmu, conn: &Connection) -> Result<usize, IcxError> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS meta (k TEXT PRIMARY KEY, v BLOB NOT NULL);")
        .map_err(|e| IcxError::Io(format!("eval scratch store: {e}")))?;
    crate::lexical::ensure_tables(conn)?;
    let mut n = 0;
    let ids: Vec<ArtifactId> = mmu.cas.artifact_entries().map(|a| a.id).collect();
    for id in ids {
        let bytes = mmu.cas.deref_bytes(id)?;
        crate::lexical::insert(conn, id, &String::from_utf8_lossy(&bytes))?;
        n += 1;
    }
    Ok(n)
}

fn dedup(ids: impl IntoIterator<Item = ArtifactId>) -> Vec<ArtifactId> {
    let mut seen = std::collections::BTreeSet::new();
    ids.into_iter().filter(|id| seen.insert(*id)).collect()
}

/// The ranked artifacts `ranker` returns for `query`, at most [`DEPTH`], without repeats.
/// `lexical` is the scratch database from [`build_lexical`]; without it, `bm25` returns nothing
/// and `rrf` is the legacy ranking.
#[must_use]
pub fn rank(
    mmu: &InfinityContextMmu,
    lexical: Option<&Connection>,
    ranker: Ranker,
    query: &str,
) -> Vec<ArtifactId> {
    let legacy = || {
        dedup(
            mmu.hdc_index
                .query_associative(query, DEPTH)
                .into_iter()
                .map(|h| h.artifact_id),
        )
    };
    let bm25 = || {
        lexical.map_or_else(Vec::new, |conn| {
            dedup(
                crate::lexical::search_words(conn, query, DEPTH)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(id, _)| id)
                    .filter(|id| mmu.cas.get_meta(*id).is_some()),
            )
        })
    };
    let mut out = match ranker {
        Ranker::Legacy => legacy(),
        Ranker::Bm25 => bm25(),
        Ranker::Rrf => crate::lexical::rrf(&[legacy(), bm25()], crate::lexical::RRF_K),
    };
    out.truncate(DEPTH);
    out
}

/// Scores ranked lists against the cases (one list per case, in order).
#[must_use]
pub fn score(ranker: Ranker, cases: &[Case], ranked: &[Vec<ArtifactId>]) -> Scores {
    let (mut r1, mut r5, mut r10, mut rr) = (0usize, 0usize, 0usize, 0.0f64);
    for (case, list) in cases.iter().zip(ranked) {
        let relevant: std::collections::BTreeSet<ArtifactId> = case
            .relevant
            .iter()
            .filter_map(|h| parse_handle(h))
            .collect();
        let first = list.iter().position(|id| relevant.contains(id));
        if let Some(pos) = first {
            r1 += usize::from(pos < 1);
            r5 += usize::from(pos < 5);
            r10 += usize::from(pos < 10);
            if pos < 10 {
                rr += 1.0 / (pos as f64 + 1.0);
            }
        }
    }
    let n = cases.len().max(1) as f64;
    Scores {
        ranker,
        queries: cases.len(),
        recall_at_1: r1 as f64 / n,
        recall_at_5: r5 as f64 / n,
        recall_at_10: r10 as f64 / n,
        mrr_at_10: rr / n,
    }
}

/// Evaluates `ranker` on `cases`.
#[must_use]
pub fn evaluate(
    mmu: &InfinityContextMmu,
    lexical: Option<&Connection>,
    ranker: Ranker,
    cases: &[Case],
) -> Scores {
    let ranked: Vec<Vec<ArtifactId>> = cases
        .iter()
        .map(|c| rank(mmu, lexical, ranker, &c.query))
        .collect();
    score(ranker, cases, &ranked)
}

/// Builds the lexical tables for `mmu` in an in-memory scratch database and evaluates each of
/// `rankers` on `cases`. Nothing is written to any workspace store.
pub fn run(
    mmu: &InfinityContextMmu,
    cases: &[Case],
    rankers: &[Ranker],
) -> Result<Vec<Scores>, IcxError> {
    let conn = Connection::open_in_memory()
        .map_err(|e| IcxError::Io(format!("eval scratch store: {e}")))?;
    build_lexical(mmu, &conn)?;
    Ok(rankers
        .iter()
        .map(|&r| evaluate(mmu, Some(&conn), r, cases))
        .collect())
}

/// Reads a JSON Lines query set (one [`Case`] per line; blank lines skipped), keeping the cases
/// of `split` (`train`, `heldout`, or `all`).
pub fn read_cases(text: &str, split: &str) -> Result<Vec<Case>, IcxError> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let case: Case = serde_json::from_str(line)
            .map_err(|e| IcxError::Io(format!("query set line {}: {e}", i + 1)))?;
        if split == "all" || case.split == split {
            out.push(case);
        }
    }
    Ok(out)
}

/// A small deterministic generator (xorshift64*).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// A synthetic workspace and query set: `n` events of 24 words from a `vocab`-word vocabulary,
/// and `queries` queries of three of a random event's words, that event being the one relevant
/// artifact. Every event is distinct (it carries its own index), so each query has one target.
#[must_use]
pub fn synthetic(
    n: usize,
    vocab: usize,
    queries: usize,
    seed: u64,
) -> (InfinityContextMmu, Vec<Case>) {
    let mut rng = Rng(seed | 1);
    let mut mmu = InfinityContextMmu::default();
    let mut events = Vec::with_capacity(n);
    for i in 0..n {
        let words: Vec<usize> = (0..24).map(|_| rng.below(vocab)).collect();
        let text: Vec<String> = words.iter().map(|w| format!("w{w}")).collect();
        let id = mmu
            .ingest_payload(
                AgentKind::GenericMcp,
                &format!("Bash:run-{i}"),
                &format!("{}\nevent {i}\n", text.join(" ")),
                false,
            )
            .artifact_id;
        events.push((id, words));
    }
    let cases = (0..queries)
        .map(|q| {
            let (id, words) = &events[rng.below(n)];
            let picked: Vec<String> = (0..3)
                .map(|_| format!("w{}", words[rng.below(24)]))
                .collect();
            Case {
                query: picked.join(" "),
                relevant: vec![id.to_string()],
                split: if q % 5 == 0 { "train" } else { "heldout" }.to_owned(),
            }
        })
        .collect();
    (mmu, cases)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_parse_and_metrics_follow_their_definitions() {
        assert_eq!(parse_handle("#icx:00000000000000ff"), Some(ArtifactId(255)));
        assert_eq!(parse_handle("00000000000000ff"), Some(ArtifactId(255)));
        assert_eq!(parse_handle("#icx:ff"), None);
        let case = |h: u64| Case {
            query: String::new(),
            relevant: vec![ArtifactId(h).to_string()],
            split: "heldout".into(),
        };
        let cases = [case(1), case(2), case(3), case(4)];
        let ids = |v: &[u64]| v.iter().map(|&x| ArtifactId(x)).collect::<Vec<_>>();
        let ranked = [
            ids(&[1, 9]),                            // rank 1
            ids(&[9, 9, 9, 2]),                      // rank 4
            ids(&[9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 3]), // rank 11: outside every cut
            ids(&[]),                                // missing
        ];
        let s = score(Ranker::Legacy, &cases, &ranked);
        assert_eq!(s.queries, 4);
        assert!((s.recall_at_1 - 0.25).abs() < 1e-12);
        assert!((s.recall_at_5 - 0.5).abs() < 1e-12);
        assert!((s.recall_at_10 - 0.5).abs() < 1e-12);
        assert!((s.mrr_at_10 - (1.0 + 0.25) / 4.0).abs() < 1e-12);
    }

    #[test]
    fn the_same_seed_gives_the_same_set_and_scores() {
        let (a, ca) = synthetic(300, 400, 40, 7);
        let (b, cb) = synthetic(300, 400, 40, 7);
        assert_eq!(ca, cb);
        let la = Connection::open_in_memory().unwrap();
        let lb = Connection::open_in_memory().unwrap();
        assert_eq!(build_lexical(&a, &la).unwrap(), 300);
        build_lexical(&b, &lb).unwrap();
        for r in Ranker::ALL {
            assert_eq!(
                evaluate(&a, Some(&la), r, &ca),
                evaluate(&b, Some(&lb), r, &cb)
            );
        }
        let (_, other) = synthetic(300, 400, 40, 8);
        assert_ne!(ca, other);
    }

    #[test]
    fn every_ranker_finds_rare_word_targets() {
        let (mmu, cases) = synthetic(500, 50_000, 60, 3);
        let lex = Connection::open_in_memory().unwrap();
        build_lexical(&mmu, &lex).unwrap();
        for r in Ranker::ALL {
            let s = evaluate(&mmu, Some(&lex), r, &cases);
            assert!(s.recall_at_5 > 0.9, "{}: {s:?}", r.name());
        }
        // Without the lexical tables, bm25 finds nothing and rrf is the legacy ranking.
        assert_eq!(evaluate(&mmu, None, Ranker::Bm25, &cases).recall_at_10, 0.0);
        let legacy = evaluate(&mmu, None, Ranker::Legacy, &cases);
        let rrf = evaluate(&mmu, None, Ranker::Rrf, &cases);
        assert_eq!(legacy.recall_at_10, rrf.recall_at_10);
        for r in ["legacy", "bm25", "rrf"] {
            assert_eq!(Ranker::parse(r).unwrap().name(), r);
        }
        assert_eq!(Ranker::parse("x"), None);
    }

    #[test]
    fn query_sets_read_by_split_and_run_builds_its_own_index() {
        let text = "{\"query\":\"a\",\"relevant\":[\"#icx:0000000000000001\"],\"split\":\"train\"}\n\n{\"query\":\"b\",\"relevant\":[]}\n";
        assert_eq!(read_cases(text, "all").unwrap().len(), 2);
        assert_eq!(read_cases(text, "train").unwrap().len(), 1);
        assert_eq!(read_cases(text, "heldout").unwrap()[0].query, "b");
        assert!(read_cases("{not json", "all").is_err());
        let (mmu, cases) = synthetic(200, 50_000, 20, 11);
        let scores = run(&mmu, &cases, &Ranker::ALL).unwrap();
        assert_eq!(scores.len(), 3);
        assert!(scores.iter().all(|s| s.queries == 20));
    }
}
