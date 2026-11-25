//! Retrieval evaluation harness (v1.5.0, T-72-002): scores a ranker on a query set by recall@1,
//! recall@5, recall@10 and MRR@10.
//!
//! - **Rankers:** `legacy` is the HDC associative index as recall uses it today; `bm25` is the
//!   lexical index alone; `rrf` is the shipped fusion (`InfinityContextMmu::fused_ranking`: the
//!   first 4 displayable HDC hits and the first 20 lexical matches, k = 60); `recent` is
//!   newest first.
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
    /// Newest first: the baseline every ranker must beat when the answer is usually recent.
    Recent,
}

impl Ranker {
    pub const ALL: [Self; 4] = [Self::Legacy, Self::Bm25, Self::Rrf, Self::Recent];

    /// Parses `legacy`, `bm25` or `rrf`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "legacy" => Some(Self::Legacy),
            "bm25" => Some(Self::Bm25),
            "rrf" => Some(Self::Rrf),
            "recent" => Some(Self::Recent),
            _ => None,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::Bm25 => "bm25",
            Self::Rrf => "rrf",
            Self::Recent => "recent",
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
    /// When the query was asked (RFC 3339, UTC). With it, only artifacts dated at or before it
    /// are ranked, so no ranker sees the future.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ts: Option<String>,
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

/// When each artifact entered the workspace, in milliseconds since the Unix epoch, and the
/// artifacts in ingest order.
#[derive(Debug, Default)]
pub struct Times {
    pub at: std::collections::HashMap<ArtifactId, u64>,
    pub order: Vec<ArtifactId>,
    /// Artifacts dated from a graph edge rather than carried forward from the one before.
    pub dated: usize,
}

/// Dates every indexed artifact: from the earliest graph edge it is the provenance of, and
/// otherwise from the next dated artifact in ingest order (ingest order is time order, so this
/// is an upper bound), or never (`u64::MAX`) after the last one. An upper bound fails closed: an
/// undated artifact is never treated as existing before it did (review of #98). An artifact in
/// the CAS with no index entry gets no time, so a time cutoff hides it (also failing closed).
#[must_use]
pub fn artifact_times(mmu: &InfinityContextMmu) -> Times {
    let mut first: std::collections::HashMap<ArtifactId, u64> = std::collections::HashMap::new();
    for e in mmu.graph.edges() {
        let t = first.entry(e.provenance).or_insert(e.t_tx_start);
        *t = (*t).min(e.t_tx_start);
    }
    let mut out = Times::default();
    for (id, ..) in &mmu.hdc_events {
        if !out.at.contains_key(id) {
            out.at
                .insert(*id, first.get(id).copied().unwrap_or(u64::MAX));
            out.order.push(*id);
        }
    }
    // Walk backwards: a dated artifact keeps its own time (review of #98: clamping it to a later
    // one could move it into the past and fail open); an undated one takes the time of the next
    // dated artifact, by which it certainly existed.
    let mut next = u64::MAX;
    for id in out.order.iter().rev() {
        let Some(t) = out.at.get_mut(id) else {
            continue;
        };
        if first.contains_key(id) {
            out.dated += 1;
            next = *t;
        } else {
            *t = next;
        }
    }
    out
}

/// Milliseconds since the Unix epoch for an RFC 3339 UTC time (`2026-11-24T20:51:00.123Z`).
#[must_use]
pub fn parse_rfc3339_ms(ts: &str) -> Option<u64> {
    let bytes = ts.as_bytes();
    if bytes.len() < 20
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || !ts.ends_with('Z')
    {
        return None;
    }
    // Digits only (review of #98: a signed parse accepted `-1` and `+1`).
    let num = |r: std::ops::Range<usize>| {
        let f = ts.get(r)?;
        if f.bytes().all(|c| c.is_ascii_digit()) {
            f.parse::<i64>().ok()
        } else {
            None
        }
    };
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (hour, mi, sec) = (num(11..13)?, num(14..16)?, num(17..19)?);
    // Nothing, or `.` and 1 to 9 digits, between the seconds and `Z` (review of #98).
    let rest = ts.get(19..ts.len() - 1)?;
    let frac = match rest.strip_prefix('.') {
        None if rest.is_empty() => "",
        Some(f) if (1..=9).contains(&f.len()) && f.bytes().all(|c| c.is_ascii_digit()) => f,
        _ => return None,
    };
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let month_days = match mo {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    if !(1..=month_days).contains(&d) || hour > 23 || mi > 59 || sec > 60 {
        return None;
    }
    let ms: i64 = format!("{frac:0<3}").get(..3)?.parse().ok()?;
    // Days from civil (Howard Hinnant's algorithm).
    let yy = if mo <= 2 { y - 1 } else { y };
    let era = yy.div_euclid(400);
    let yoe = yy - era * 400;
    let mp = (mo + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let total = ((days * 24 + hour) * 60 + mi) * 60 + sec;
    u64::try_from(total * 1000 + ms).ok()
}

/// The cases that have a relevant artifact existing when they were asked, and how many were
/// dropped. With prompts as queries, outputs produced later in the turn did not exist yet, so a
/// case whose answers all came later cannot be answered by any ranker (review of #98). Cases
/// without a time are kept.
#[must_use]
pub fn answer_existed(times: &Times, cases: Vec<Case>) -> (Vec<Case>, usize) {
    let total = cases.len();
    let kept: Vec<Case> = cases
        .into_iter()
        .filter(|c| {
            let Some(at) = c.ts.as_deref().and_then(parse_rfc3339_ms) else {
                return true;
            };
            c.relevant
                .iter()
                .filter_map(|h| parse_handle(h))
                .any(|id| times.at.get(&id).is_some_and(|&t| t <= at))
        })
        .collect();
    let dropped = total - kept.len();
    (kept, dropped)
}

/// The cases whose answer is old when asked: at least `min_age` artifacts entered the workspace
/// after the newest relevant one and before the query. A just-produced output is still in the
/// agent's context and is found by recency alone, so it does not measure recall (first real-set
/// run: newest-first had recall@1 above 0.8). Cases without a time are kept only when
/// `min_age` is 0.
#[must_use]
pub fn old_answers(times: &Times, cases: Vec<Case>, min_age: usize) -> Vec<Case> {
    if min_age == 0 {
        return cases;
    }
    cases
        .into_iter()
        .filter(|c| {
            let Some(at) = c.ts.as_deref().and_then(parse_rfc3339_ms) else {
                return false;
            };
            // Only answers that existed when asked (review of #98: a later one must not decide).
            let newest = c
                .relevant
                .iter()
                .filter_map(|h| parse_handle(h))
                .filter(|id| times.at.get(id).is_some_and(|&t| t <= at))
                .filter_map(|id| times.order.iter().position(|x| *x == id))
                .max();
            newest.is_some_and(|pos| {
                times.order[pos + 1..]
                    .iter()
                    .filter(|id| times.at.get(id).is_some_and(|&t| t <= at))
                    .count()
                    >= min_age
            })
        })
        .collect()
}

/// The ranked artifacts `ranker` returns for `query`, at most [`DEPTH`], without repeats.
/// `lexical` is the scratch database from [`build_lexical`]; without it, `bm25` returns nothing
/// and `rrf` is the legacy ranking. With `asked`, only artifacts dated at or before that time
/// are ranked.
#[must_use]
pub fn rank(
    mmu: &InfinityContextMmu,
    lexical: Option<&Connection>,
    ranker: Ranker,
    query: &str,
    asked: Option<(&Times, u64)>,
) -> Vec<ArtifactId> {
    // With a cutoff, search everything before filtering, so later artifacts cannot fill a bounded
    // candidate list and push an earlier answer out (review of #98).
    // HDC entries can repeat an artifact, so the entry count bounds the HDC list.
    let depth = if asked.is_some() {
        mmu.hdc_events
            .len()
            .max(mmu.cas.artifact_count())
            .max(DEPTH)
    } else {
        DEPTH
    };
    let visible = |id: &ArtifactId| {
        asked.is_none_or(|(times, at)| times.at.get(id).is_some_and(|&t| t <= at))
    };
    let legacy = || {
        dedup(
            mmu.hdc_index
                .query_associative(query, depth)
                .into_iter()
                .map(|h| h.artifact_id)
                .filter(visible),
        )
        .into_iter()
        // Each list goes into fusion at `DEPTH`, with or without a cutoff (review of #98).
        .take(DEPTH)
        .collect::<Vec<_>>()
    };
    let bm25 = || {
        lexical.map_or_else(Vec::new, |conn| {
            dedup(
                crate::lexical::search_words(conn, query, depth)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(id, _)| id)
                    .filter(|id| mmu.cas.get_meta(*id).is_some())
                    .filter(visible),
            )
            .into_iter()
            .take(DEPTH)
            .collect()
        })
    };
    let recent = || {
        asked.map_or_else(Vec::new, |(times, _)| {
            times
                .order
                .iter()
                .rev()
                .copied()
                .filter(visible)
                .take(DEPTH)
                .collect()
        })
    };
    let mut out = match ranker {
        Ranker::Legacy => legacy(),
        Ranker::Bm25 => bm25(),
        // Exactly the shipped fusion (review of #98): the first `FUSED_HDC_DEPTH` HDC hits that
        // existed and the first `FUSED_LEXICAL_DEPTH` lexical matches, through
        // `InfinityContextMmu::fused_ranking`.
        Ranker::Rrf => {
            let hits: Vec<_> = mmu
                .hdc_index
                .query_associative(query, depth)
                .into_iter()
                .filter(|h| visible(&h.artifact_id))
                .take(crate::mmu::FUSED_HDC_DEPTH)
                .collect();
            let lexical: Vec<ArtifactId> = bm25()
                .into_iter()
                .take(crate::mmu::FUSED_LEXICAL_DEPTH)
                .collect();
            InfinityContextMmu::fused_ranking(&hits, &lexical)
        }
        Ranker::Recent => recent(),
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
    times: Option<&Times>,
) -> Scores {
    let ranked: Vec<Vec<ArtifactId>> = cases
        .iter()
        .map(|c| {
            let asked = times.zip(c.ts.as_deref().and_then(parse_rfc3339_ms));
            rank(mmu, lexical, ranker, &c.query, asked)
        })
        .collect();
    score(ranker, cases, &ranked)
}

/// The cases with at least one relevant artifact in `mmu`'s CAS, and how many were dropped. A
/// case whose answer is not stored here (a fold handle, another workspace's artifact) cannot be
/// found by any ranker, so it is reported and not scored.
#[must_use]
pub fn answerable(mmu: &InfinityContextMmu, cases: Vec<Case>) -> (Vec<Case>, usize) {
    let total = cases.len();
    let kept: Vec<Case> = cases
        .into_iter()
        .filter(|c| {
            c.relevant
                .iter()
                .filter_map(|h| parse_handle(h))
                .any(|id| mmu.cas.get_meta(id).is_some())
        })
        .collect();
    let dropped = total - kept.len();
    (kept, dropped)
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
    let times = artifact_times(mmu);
    Ok(rankers
        .iter()
        .map(|&r| evaluate(mmu, Some(&conn), r, cases, Some(&times)))
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
        let mut case: Case = serde_json::from_str(line)
            .map_err(|e| IcxError::Io(format!("query set line {}: {e}", i + 1)))?;
        // An empty time means none; any other time must parse, since an unreadable one would
        // silently disable the cutoff (review of #98).
        if case.ts.as_deref().is_some_and(str::is_empty) {
            case.ts = None;
        }
        if let Some(ts) = case.ts.as_deref()
            && parse_rfc3339_ms(ts).is_none()
        {
            return Err(IcxError::Io(format!(
                "query set line {}: unreadable time {ts:?} (expected RFC 3339 UTC, ending in Z)",
                i + 1
            )));
        }
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
                ts: None,
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
            ts: None,
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
                evaluate(&a, Some(&la), r, &ca, None),
                evaluate(&b, Some(&lb), r, &cb, None)
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
        for r in [Ranker::Legacy, Ranker::Bm25, Ranker::Rrf] {
            let s = evaluate(&mmu, Some(&lex), r, &cases, None);
            assert!(s.recall_at_5 > 0.9, "{}: {s:?}", r.name());
        }
        // Without the lexical tables, bm25 finds nothing and rrf is the legacy ranking.
        assert_eq!(
            evaluate(&mmu, None, Ranker::Bm25, &cases, None).recall_at_10,
            0.0
        );
        let legacy = evaluate(&mmu, None, Ranker::Legacy, &cases, None);
        let rrf = evaluate(&mmu, None, Ranker::Rrf, &cases, None);
        assert_eq!(legacy.recall_at_10, rrf.recall_at_10);
        for r in ["legacy", "bm25", "rrf", "recent"] {
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
        assert_eq!(scores.len(), 4);
        assert!(scores.iter().all(|s| s.queries == 20));
    }

    #[test]
    fn unanswerable_cases_are_dropped_and_counted() {
        let (mmu, mut cases) = synthetic(50, 1_000, 5, 2);
        cases.push(Case {
            query: "nothing stored".into(),
            relevant: vec!["#icx:00000000000000aa".into()],
            split: "heldout".into(),
            ts: None,
        });
        let (kept, dropped) = answerable(&mmu, cases);
        assert_eq!((kept.len(), dropped), (5, 1));
    }

    #[test]
    fn times_parse_and_the_cutoff_hides_the_future() {
        assert_eq!(parse_rfc3339_ms("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_rfc3339_ms("1970-01-01T00:00:01.5Z"), Some(1_500));
        assert!(
            parse_rfc3339_ms("2024-02-29T00:00:00Z").is_some(),
            "leap day"
        );
        assert_eq!(
            parse_rfc3339_ms("2026-11-24T20:51:00.123Z"),
            Some(1_795_553_460_123)
        );
        assert_eq!(parse_rfc3339_ms("2026-11-24 20:51:00"), None);
        let mut times = Times::default();
        for (i, id) in [10u64, 20, 30].into_iter().enumerate() {
            times.at.insert(ArtifactId(id), 1_000 * (i as u64 + 1));
            times.order.push(ArtifactId(id));
        }
        let mmu = InfinityContextMmu::default();
        // Recent ranks only what existed at the time, newest first.
        assert_eq!(
            rank(&mmu, None, Ranker::Recent, "x", Some((&times, 2_000))),
            [ArtifactId(20), ArtifactId(10)]
        );
        assert_eq!(rank(&mmu, None, Ranker::Recent, "x", None), []);
    }

    #[test]
    fn old_answers_keeps_only_cases_with_enough_newer_artifacts() {
        let mut times = Times::default();
        for i in 0..10u64 {
            times.at.insert(ArtifactId(i), 1_000 * i);
            times.order.push(ArtifactId(i));
        }
        let case = |rel: u64, ts: &str| Case {
            query: String::new(),
            relevant: vec![ArtifactId(rel).to_string()],
            split: "heldout".into(),
            ts: Some(ts.into()),
        };
        // Asked at 9 s: artifact 2 has seven newer ones, artifact 8 has one.
        let cases = vec![
            case(2, "1970-01-01T00:00:09Z"),
            case(8, "1970-01-01T00:00:09Z"),
        ];
        assert_eq!(old_answers(&times, cases.clone(), 5).len(), 1);
        assert_eq!(old_answers(&times, cases.clone(), 0).len(), 2);
        // Artifacts after the question do not count.
        let early = vec![case(2, "1970-01-01T00:00:04Z")];
        assert_eq!(old_answers(&times, early, 5).len(), 0);
    }

    #[test]
    fn answers_produced_after_the_query_are_dropped() {
        let mut times = Times::default();
        for (i, id) in [1u64, 2].into_iter().enumerate() {
            times.at.insert(ArtifactId(id), 1_000 * (i as u64 + 1));
            times.order.push(ArtifactId(id));
        }
        let case = |rel: u64| Case {
            query: String::new(),
            relevant: vec![ArtifactId(rel).to_string()],
            split: "heldout".into(),
            ts: Some("1970-01-01T00:00:01.500Z".into()),
        };
        let (kept, dropped) = answer_existed(&times, vec![case(1), case(2)]);
        assert_eq!((kept.len(), dropped), (1, 1));
    }

    #[test]
    fn malformed_times_are_refused_and_empty_ones_mean_none() {
        for bad in [
            "2026-11-24T20:51:00bogusZ",
            "2026-11-24T20-51-00Z",
            "2026-13-24T20:51:00Z",
            "2026-11-24T20:51:00+00:00",
            "2026-11-24T20:51:00.Z",
            "2026-11-24T20:51:0é0Z",
            "2026-11-24T-1:-2:-3Z",
            "2026-11-24T+1:00:00Z",
            "2026-02-31T00:00:00Z",
            "2025-02-29T00:00:00Z",
        ] {
            assert_eq!(parse_rfc3339_ms(bad), None, "{bad}");
            let line = format!("{{\"query\":\"q\",\"relevant\":[],\"ts\":\"{bad}\"}}");
            assert!(read_cases(&line, "all").is_err(), "{bad}");
        }
        let empty = read_cases("{\"query\":\"q\",\"relevant\":[],\"ts\":\"\"}", "all").unwrap();
        assert_eq!(empty[0].ts, None);
    }

    #[test]
    fn a_later_answer_does_not_hide_an_old_one() {
        let mut times = Times::default();
        for i in 0..10u64 {
            times.at.insert(ArtifactId(i), 1_000 * i);
            times.order.push(ArtifactId(i));
        }
        // Asked at 9 s with answers 1 (old) and 9 (produced at 9 s, the last one): 1 counts.
        let case = Case {
            query: String::new(),
            relevant: vec![ArtifactId(1).to_string(), ArtifactId(9).to_string()],
            split: "heldout".into(),
            ts: Some("1970-01-01T00:00:08Z".into()),
        };
        assert_eq!(old_answers(&times, vec![case], 5).len(), 1);
    }

    #[test]
    fn a_dated_artifact_keeps_its_own_time() {
        let mut mmu = InfinityContextMmu::default();
        let a = mmu
            .ingest_payload(AgentKind::Codex, "log", "first\n", false)
            .artifact_id;
        let b = mmu
            .ingest_payload(AgentKind::Codex, "log", "second\n", false)
            .artifact_id;
        // Out-of-order clocks: the later artifact is dated earlier.
        mmu.graph.upsert_edge("x", "r", "1", false, 2_000, a, 1.0);
        mmu.graph.upsert_edge("y", "r", "2", false, 1_000, b, 1.0);
        let t = artifact_times(&mmu);
        assert!(
            t.at[&a] >= 2_000,
            "never moved into the past: {:?}",
            t.at[&a]
        );
        assert_eq!(t.at[&b], 1_000);
    }
}
