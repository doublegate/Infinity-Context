//! Lexical index over stored artifacts (v1.5.0, T-71-001): SQLite FTS5 tables in the workspace
//! store, one for BM25 over words and one for trigram substring candidates.
//!
//! - `lex_words` uses the `porter unicode61` tokenizer with `_` as a token character, so
//!   `snake_case` identifiers stay whole; a second column holds the parts of every compound
//!   identifier (`camelCase`, `snake_case`, `kebab-case`), so a query for a part finds the whole.
//!   Paths split at `/` and `.` in both.
//! - `lex_trigrams` uses the `trigram` tokenizer with `detail=none`: it finds every artifact that
//!   holds all of a needle's trigrams, a candidate set that is never too small. Each candidate is
//!   then confirmed against the artifact's own text, so a substring hit is exact.
//!
//! Both tables are contentless (`content=''`): the text already lives in the CAS. Rows are
//! numbered compactly and mapped to artifact ids through `lex_ids`, because FTS5 stores rowid gaps
//! as varints and random 64-bit ids made every posting about 9 bytes. Measured sizes are in
//! `docs/numbers.md`.
//!
//! The index is derived state and is not covered by the store seal, so it is treated as
//! untrusted: every id it returns must name an artifact in the sealed in-memory CAS before it is
//! used. It is rebuildable from the CAS at any time. It is built only while lexical ranking is
//! enabled (`ICX_RECALL_RANKING=rrf`), and older builds ignore the tables, so the store schema
//! version is unchanged and a downgrade keeps working.

use icx_core::{ArtifactId, IcxError};
use rusqlite::{Connection, params};

/// Version of the lexical tables, stored in `meta` under `lexical_index`.
/// Version 2 numbers rows compactly through `lex_ids` (review of #97: random 64-bit rowids made
/// the trigram postings about five times larger).
pub const LEXICAL_VERSION: i64 = 2;

/// Text indexed per artifact, in bytes: the first 1 MiB. Longer artifacts are still found by
/// their beginning; their full text stays in the CAS.
pub const MAX_INDEXED_BYTES: usize = 1 << 20;

/// Artifacts indexed per save beyond the new ones, so turning the index on for a large store
/// costs a bounded amount per hook instead of one long first save.
pub const BACKFILL_BATCH: usize = 2_000;

/// Query terms used, so a long query cannot build an unbounded FTS5 expression.
pub const MAX_QUERY_TERMS: usize = 32;

/// Whether lexical ranking, and with it the index, is enabled (`ICX_RECALL_RANKING=rrf`).
#[must_use]
pub fn enabled() -> bool {
    std::env::var("ICX_RECALL_RANKING").is_ok_and(|v| v.trim().eq_ignore_ascii_case("rrf"))
}

fn db_err(e: impl std::fmt::Display) -> IcxError {
    IcxError::Io(format!("lexical index: {e}"))
}

/// The `lexical_index` version recorded in `meta`, if any.
/// A value that is not an integer reads as `None`, so the tables are rebuilt (review of #97).
fn stored_version(conn: &Connection) -> Result<Option<i64>, IcxError> {
    conn.query_row("SELECT v FROM meta WHERE k = 'lexical_index'", [], |r| {
        r.get::<_, i64>(0)
    })
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows | rusqlite::Error::InvalidColumnType(..) => Ok(None),
        other => Err(db_err(other)),
    })
}

/// Number of the three lexical tables present (`lex_ids`, `lex_words`, `lex_trigrams`).
fn table_count(conn: &Connection) -> Result<i64, IcxError> {
    conn.query_row(
        "SELECT count(*) FROM sqlite_master \
         WHERE type = 'table' AND name IN ('lex_ids', 'lex_words', 'lex_trigrams')",
        [],
        |r| r.get(0),
    )
    .map_err(db_err)
}

/// Creates the tables if missing and records their version. Tables of another version, or only
/// one of the two, are dropped first and rebuilt (review of #97). Returns `true` when the tables
/// were (re)created empty, so the caller's record of indexed ids is stale.
pub fn ensure_tables(conn: &Connection) -> Result<bool, IcxError> {
    let count = table_count(conn)?;
    let current = count == 3 && stored_version(conn)? == Some(LEXICAL_VERSION);
    if current {
        return Ok(false);
    }
    if count > 0 {
        drop_tables(conn)?;
    }
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS lex_ids (n INTEGER PRIMARY KEY, id INTEGER NOT NULL UNIQUE);
         CREATE VIRTUAL TABLE IF NOT EXISTS lex_words USING fts5(body, parts, \
           tokenize = \"porter unicode61 remove_diacritics 2 tokenchars '_'\", content = '');
         CREATE VIRTUAL TABLE IF NOT EXISTS lex_trigrams USING fts5(body, \
           tokenize = 'trigram case_sensitive 1', detail = none, content = '');",
    )
    .map_err(db_err)?;
    conn.execute(
        "INSERT INTO meta (k, v) VALUES ('lexical_index', ?1) \
         ON CONFLICT(k) DO UPDATE SET v = excluded.v",
        params![LEXICAL_VERSION],
    )
    .map_err(db_err)?;
    Ok(true)
}

/// Whether this build can query the lexical tables: all three exist, at this build's version. A store
/// written by a newer build is not read (review of #97).
pub(crate) fn tables_exist(conn: &Connection) -> Result<bool, IcxError> {
    Ok(table_count(conn)? == 3 && stored_version(conn)? == Some(LEXICAL_VERSION))
}

/// Drops the tables (contentless rows cannot be deleted one by one); the next save with the
/// index enabled rebuilds them.
pub(crate) fn drop_tables(conn: &Connection) -> Result<(), IcxError> {
    conn.execute_batch(
        "DROP TABLE IF EXISTS lex_words; DROP TABLE IF EXISTS lex_trigrams;
         DROP TABLE IF EXISTS lex_ids;
         DELETE FROM meta WHERE k = 'lexical_index';",
    )
    .map_err(db_err)
}

/// Ids already in the index.
pub(crate) fn indexed_ids(
    conn: &Connection,
) -> Result<std::collections::BTreeSet<ArtifactId>, IcxError> {
    let mut stmt = conn.prepare("SELECT id FROM lex_ids").map_err(db_err)?;
    let rows = stmt.query_map([], |r| r.get::<_, i64>(0)).map_err(db_err)?;
    let mut out = std::collections::BTreeSet::new();
    for r in rows {
        out.insert(id_from_sql(r.map_err(db_err)?));
    }
    Ok(out)
}

const fn id_to_sql(id: ArtifactId) -> i64 {
    i64::from_ne_bytes(id.0.to_ne_bytes())
}
const fn id_from_sql(v: i64) -> ArtifactId {
    ArtifactId(u64::from_ne_bytes(v.to_ne_bytes()))
}

/// The indexed prefix of `text`: at most [`MAX_INDEXED_BYTES`], cut at a character boundary.
fn indexed_prefix(text: &str) -> &str {
    &text[..text.floor_char_boundary(MAX_INDEXED_BYTES)]
}

/// Adds one artifact to both tables.
pub fn insert(conn: &Connection, id: ArtifactId, text: &str) -> Result<(), IcxError> {
    let body = indexed_prefix(text);
    let parts = identifier_parts(body);
    // FTS5 stores rowid gaps as varints, so rows are numbered compactly here and mapped to the
    // (random 64-bit) artifact id through `lex_ids`.
    conn.prepare_cached("INSERT INTO lex_ids (id) VALUES (?1)")
        .map_err(db_err)?
        .execute(params![id_to_sql(id)])
        .map_err(db_err)?;
    let n = conn.last_insert_rowid();
    conn.prepare_cached("INSERT INTO lex_words (rowid, body, parts) VALUES (?1, ?2, ?3)")
        .map_err(db_err)?
        .execute(params![n, body, parts])
        .map_err(db_err)?;
    conn.prepare_cached("INSERT INTO lex_trigrams (rowid, body) VALUES (?1, ?2)")
        .map_err(db_err)?
        .execute(params![n, body])
        .map_err(db_err)?;
    Ok(())
}

/// The parts of every compound identifier in `text`, lowercased and space-separated:
/// `parseHttpRequest` gives `parse http request`, `shell_words` gives `shell words`. A plain
/// word gives nothing, and neither does `kebab-case`, which the tokenizer already splits in the
/// body (review of #97).
#[must_use]
pub fn identifier_parts(text: &str) -> String {
    let mut out = String::new();
    for word in text.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
        let parts = split_identifier(word);
        // `__init__` or `_private` is one part, but not the word as indexed (review of #97).
        if parts.len() > 1 || parts.first().is_some_and(|p| *p != word) {
            for p in parts {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(&p.to_lowercase());
            }
        }
    }
    out
}

/// Splits one identifier at `_`, lower-to-upper and letter-digit boundaries, and before the last
/// capital of an acronym run (`HTTPServer` gives `HTTP`, `Server`).
fn split_identifier(word: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    for piece in word.split('_').filter(|p| !p.is_empty()) {
        let chars: Vec<(usize, char)> = piece.char_indices().collect();
        let mut start = 0;
        for i in 1..chars.len() {
            let (pos, c) = chars[i];
            let prev = chars[i - 1].1;
            let next = chars.get(i + 1).map(|&(_, n)| n);
            let boundary = (prev.is_lowercase() && c.is_uppercase())
                || (prev.is_alphabetic() && c.is_numeric())
                || (prev.is_numeric() && c.is_alphabetic())
                || (prev.is_uppercase()
                    && c.is_uppercase()
                    && next.is_some_and(char::is_lowercase));
            if boundary {
                parts.push(&piece[start..pos]);
                start = pos;
            }
        }
        parts.push(&piece[start..]);
    }
    parts
}

/// The FTS5 query for `query`: each distinct word (letters, digits and `_`) as a quoted term,
/// and the parts of each compound identifier as well (so `parseHttpRequest` also finds
/// `parse_http_request`), joined by OR, at most [`MAX_QUERY_TERMS`]. Quoting makes every term a
/// literal, so no query text is read as FTS5 syntax. `None` when the query has no words.
#[must_use]
pub fn words_query(query: &str) -> Option<String> {
    let words: Vec<&str> = query
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .collect();
    // Every whole word first, then identifier parts in the remaining budget, so long compound
    // identifiers cannot crowd out later words (review of #97).
    let parts = words.iter().flat_map(|w| {
        let p = split_identifier(w);
        if p.len() > 1 || p.first().is_some_and(|x| x != w) {
            p
        } else {
            Vec::new()
        }
    });
    let mut seen = std::collections::BTreeSet::new();
    let terms: Vec<String> = words
        .iter()
        .copied()
        .chain(parts)
        .map(str::to_lowercase)
        .filter(|w| seen.insert(w.clone()))
        .take(MAX_QUERY_TERMS)
        .map(|w| format!("\"{}\"", w.replace('"', "\"\"")))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" OR "))
}

/// The `limit` best BM25 matches for `query`, best first, as `(id, score)` with higher better.
/// Ids are not yet checked against the CAS; the caller drops any it does not hold.
pub fn search_words(
    conn: &Connection,
    query: &str,
    limit: usize,
) -> Result<Vec<(ArtifactId, f64)>, IcxError> {
    let Some(expr) = words_query(query) else {
        return Ok(Vec::new());
    };
    // bm25() is lower for better matches; the body counts twice as much as identifier parts.
    let mut stmt = conn
        .prepare_cached(
            "SELECT l.id, bm25(lex_words, 1.0, 0.5) AS s FROM lex_words \
             JOIN lex_ids l ON l.n = lex_words.rowid \
             WHERE lex_words MATCH ?1 ORDER BY s, l.id LIMIT ?2",
        )
        .map_err(db_err)?;
    let rows = stmt
        .query_map(
            params![expr, i64::try_from(limit).unwrap_or(i64::MAX)],
            |r| Ok((id_from_sql(r.get(0)?), -r.get::<_, f64>(1)?)),
        )
        .map_err(db_err)?;
    rows.collect::<Result<_, _>>().map_err(db_err)
}

/// Ids of artifacts holding every sampled trigram of `needle` (at least 3 characters), at most
/// `limit`. A superset of the artifacts containing `needle`; the caller confirms each against
/// its text. A needle with more than [`MAX_QUERY_TERMS`] distinct trigrams is sampled evenly
/// along its length, start and end included, so a long shared prefix (a path, a URL) does not
/// decide the candidates alone (review of #97). A needle holding NUL matches nothing.
pub fn trigram_candidates(
    conn: &Connection,
    needle: &str,
    limit: usize,
) -> Result<Vec<ArtifactId>, IcxError> {
    let chars: Vec<char> = needle.chars().collect();
    if chars.len() < 3 || limit == 0 || chars.contains(&'\0') {
        return Ok(Vec::new());
    }
    let mut seen = std::collections::BTreeSet::new();
    let distinct: Vec<String> = chars
        .windows(3)
        .map(|w| w.iter().collect::<String>())
        .filter(|t| seen.insert(t.clone()))
        .collect();
    let picked: Vec<&String> = if distinct.len() <= MAX_QUERY_TERMS {
        distinct.iter().collect()
    } else {
        let last = distinct.len() - 1;
        (0..MAX_QUERY_TERMS)
            .map(|i| &distinct[i * last / (MAX_QUERY_TERMS - 1)])
            .collect()
    };
    let terms: Vec<String> = picked
        .iter()
        .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
        .collect();
    let mut stmt = conn
        .prepare_cached(
            "SELECT l.id FROM lex_trigrams JOIN lex_ids l ON l.n = lex_trigrams.rowid \
             WHERE lex_trigrams MATCH ?1 ORDER BY l.id LIMIT ?2",
        )
        .map_err(db_err)?;
    let rows = stmt
        .query_map(
            params![
                terms.join(" AND "),
                i64::try_from(limit).unwrap_or(i64::MAX)
            ],
            |r| r.get::<_, i64>(0),
        )
        .map_err(db_err)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(id_from_sql(r.map_err(db_err)?));
    }
    Ok(out)
}

/// The usual reciprocal-rank-fusion constant (Cormack, Clarke and Büttcher, SIGIR 2009).
pub const RRF_K: f64 = 60.0;

/// Reciprocal rank fusion of ranked lists (T-71-002): each item scores the sum of
/// `1 / (k + rank)` over the lists it appears in, ranks counted from 1, and a repeat within a
/// list counts only at its best rank. Equal scores are ordered by artifact id, so the result is
/// deterministic.
#[must_use]
pub fn rrf(lists: &[Vec<ArtifactId>], k: f64) -> Vec<ArtifactId> {
    let mut score: std::collections::BTreeMap<ArtifactId, f64> = std::collections::BTreeMap::new();
    for list in lists {
        let mut seen = std::collections::BTreeSet::new();
        // Ranks count distinct items only, so a repeat never pushes later items down (review of
        // #98).
        let mut rank = 0usize;
        for id in list {
            if seen.insert(*id) {
                rank += 1;
                *score.entry(*id).or_insert(0.0) += 1.0 / (k + rank as f64);
            }
        }
    }
    let mut out: Vec<(ArtifactId, f64)> = score.into_iter().collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    out.into_iter().map(|(id, _)| id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE meta (k TEXT PRIMARY KEY, v BLOB NOT NULL);")
            .unwrap();
        ensure_tables(&c).unwrap();
        c
    }

    #[test]
    fn identifiers_split_into_their_parts() {
        assert_eq!(
            split_identifier("parseHttpRequest"),
            ["parse", "Http", "Request"]
        );
        assert_eq!(split_identifier("HTTPServer"), ["HTTP", "Server"]);
        assert_eq!(split_identifier("shell_words"), ["shell", "words"]);
        assert_eq!(split_identifier("icx-adapters"), ["icx-adapters"]); // no `-` splitting here
        assert_eq!(identifier_parts("icx-adapters"), "");
        assert_eq!(split_identifier("v1beta2"), ["v", "1", "beta", "2"]);
        assert_eq!(split_identifier("plain"), ["plain"]);
        assert_eq!(
            identifier_parts("call parseHttpRequest now"),
            "parse http request"
        );
        assert_eq!(identifier_parts("plain words only"), "");
        assert_eq!(identifier_parts("def __init__(self_)"), "init self");
    }

    #[test]
    fn words_query_quotes_every_term() {
        assert_eq!(
            words_query("Shell  words shell"),
            Some("\"shell\" OR \"words\"".into())
        );
        // FTS5 operators and quotes in the query are only ever terms.
        assert_eq!(
            words_query("a AND b\" NEAR(c"),
            Some("\"a\" OR \"and\" OR \"b\" OR \"near\" OR \"c\"".into())
        );
        assert_eq!(words_query("!!! ..."), None);
        assert_eq!(
            words_query("parseHttp later"),
            Some("\"parsehttp\" OR \"later\" OR \"parse\" OR \"http\"".into())
        );
        let long = (0..100)
            .map(|i| format!("w{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(
            words_query(&long).unwrap().matches(" OR ").count(),
            MAX_QUERY_TERMS - 1
        );
    }

    #[test]
    fn bm25_finds_words_and_identifier_parts() {
        let c = mem();
        insert(
            &c,
            ArtifactId(1),
            "fn parseHttpRequest(buf) in src/net/http.rs",
        )
        .unwrap();
        insert(&c, ArtifactId(2), "test result: ok. 12 passed; 0 failed").unwrap();
        insert(&c, ArtifactId(u64::MAX), "let shell_words = split(cmd);").unwrap();
        let ids = |q: &str| -> Vec<u64> {
            search_words(&c, q, 10)
                .unwrap()
                .into_iter()
                .map(|(id, _)| id.0)
                .collect()
        };
        assert_eq!(ids("request parsing"), [1]); // porter: parsing ~ parse; a part of the identifier
        assert_eq!(ids("http.rs"), [1]); // a path's pieces
        assert_eq!(ids("passed"), [2]);
        assert_eq!(ids("shell_words"), [u64::MAX]); // whole snake_case identifier, high id bit
        assert_eq!(ids("words"), [u64::MAX]); // one of its parts
        assert_eq!(ids("nothing here"), [] as [u64; 0]);
        // Scores are ordered best first.
        let r = search_words(&c, "http request passed", 10).unwrap();
        assert!(r.windows(2).all(|w| w[0].1 >= w[1].1));
    }

    #[test]
    fn trigrams_give_a_superset_for_substrings() {
        let c = mem();
        insert(&c, ArtifactId(1), "error: E0502 cannot borrow").unwrap();
        insert(&c, ArtifactId(2), "warning: unused import").unwrap();
        assert_eq!(
            trigram_candidates(&c, "E0502", 10).unwrap(),
            [ArtifactId(1)]
        );
        assert_eq!(
            trigram_candidates(&c, "0502 can", 10).unwrap(),
            [ArtifactId(1)]
        );
        // Case-sensitive, like the substring check that confirms each candidate.
        assert_eq!(trigram_candidates(&c, "e0502", 10).unwrap(), []);
        assert_eq!(trigram_candidates(&c, "zzz9", 10).unwrap(), []);
        assert_eq!(trigram_candidates(&c, "E0", 10).unwrap(), []); // too short
    }

    #[test]
    fn only_a_prefix_of_huge_artifacts_is_indexed_and_tables_rebuild() {
        let c = mem();
        let big = format!("{}tailword", "é".repeat(MAX_INDEXED_BYTES));
        insert(&c, ArtifactId(7), &big).unwrap();
        assert_eq!(search_words(&c, "tailword", 5).unwrap(), []);
        assert_eq!(indexed_ids(&c).unwrap().len(), 1);
        assert!(tables_exist(&c).unwrap());
        drop_tables(&c).unwrap();
        assert!(!tables_exist(&c).unwrap());
        ensure_tables(&c).unwrap();
        assert_eq!(indexed_ids(&c).unwrap().len(), 0);
    }

    #[test]
    fn long_needles_are_sampled_along_their_length_and_nul_matches_nothing() {
        let c = mem();
        let shared = "https://example.invalid/a/very/long/shared/prefix/";
        insert(&c, ArtifactId(1), &format!("{shared}first-target")).unwrap();
        insert(&c, ArtifactId(2), &format!("{shared}second-other")).unwrap();
        // Past the term budget, but the end of the needle still decides.
        assert_eq!(
            trigram_candidates(&c, &format!("{shared}first-target"), 10).unwrap(),
            [ArtifactId(1)]
        );
        assert_eq!(trigram_candidates(&c, "pre\0fix", 10).unwrap(), []);
        assert_eq!(trigram_candidates(&c, "prefix", 0).unwrap(), []);
    }

    #[test]
    fn tables_of_another_version_or_half_a_pair_are_rebuilt() {
        let c = mem();
        insert(&c, ArtifactId(1), "kept until a rebuild").unwrap();
        assert!(!ensure_tables(&c).unwrap(), "current tables are kept");
        c.execute("UPDATE meta SET v = 99 WHERE k = 'lexical_index'", [])
            .unwrap();
        assert!(!tables_exist(&c).unwrap(), "a newer version is not read");
        assert!(ensure_tables(&c).unwrap(), "rebuilt for this version");
        assert_eq!(indexed_ids(&c).unwrap().len(), 0);
        c.execute_batch("DROP TABLE lex_trigrams").unwrap();
        assert!(ensure_tables(&c).unwrap(), "a partial set is rebuilt");
        assert!(tables_exist(&c).unwrap());
    }

    #[test]
    fn rrf_rewards_agreement_and_breaks_ties_by_id() {
        let a = ArtifactId(1);
        let b = ArtifactId(2);
        let c = ArtifactId(3);
        // `b` is second in both lists, so it beats items that are first in only one.
        assert_eq!(rrf(&[vec![a, b], vec![c, b]], RRF_K), [b, a, c]);
        // A repeat within a list counts once and does not push later items down: `c` is second
        // in the first list despite the repeats, so it ties `b` (first in the other list after a
        // repeat is skipped) and wins the tie by id only when its score is equal.
        assert_eq!(rrf(&[vec![a, a, a], vec![b]], RRF_K), [a, b]);
        assert_eq!(rrf(&[vec![a, a, c], vec![b, c]], RRF_K), [c, a, b]);
        // Equal scores: by id.
        assert_eq!(rrf(&[vec![c], vec![a]], RRF_K), [a, c]);
        assert_eq!(rrf(&[], RRF_K), []);
    }
}
