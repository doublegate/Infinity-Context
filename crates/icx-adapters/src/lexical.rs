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
//! Both tables are contentless (`content=''`): the text already lives in the CAS. Measured on a
//! 54 MB store holding 8.5 MB of artifact text, they take 3.0 MB and 3.7 MB; with content copies
//! they took 12.9 MB and 13.6 MB.
//!
//! The index is derived state and is not covered by the store seal, so it is treated as
//! untrusted: every id it returns must name an artifact in the sealed in-memory CAS before it is
//! used. It is rebuildable from the CAS at any time. It is built only while lexical ranking is
//! enabled (`ICX_RECALL_RANKING=rrf`), and older builds ignore the tables, so the store schema
//! version is unchanged and a downgrade keeps working.

use icx_core::{ArtifactId, IcxError};
use rusqlite::{Connection, params};

/// Version of the lexical tables, stored in `meta` under `lexical_index`.
pub const LEXICAL_VERSION: i64 = 1;

/// Text indexed per artifact, in bytes: the first 1 MiB. Longer artifacts are still found by
/// their beginning; their full text stays in the CAS.
pub const MAX_INDEXED_BYTES: usize = 1 << 20;

/// Artifacts indexed per save beyond the new ones, so turning the index on for a large store
/// costs a bounded amount per hook instead of one long first save.
pub const BACKFILL_BATCH: usize = 2_000;

/// Query terms used, so a long query cannot build an unbounded FTS5 expression.
const MAX_QUERY_TERMS: usize = 32;

/// Whether lexical ranking, and with it the index, is enabled (`ICX_RECALL_RANKING=rrf`).
#[must_use]
pub fn enabled() -> bool {
    std::env::var("ICX_RECALL_RANKING").is_ok_and(|v| v.trim().eq_ignore_ascii_case("rrf"))
}

fn db_err(e: impl std::fmt::Display) -> IcxError {
    IcxError::Io(format!("lexical index: {e}"))
}

/// Creates the tables if missing and records their version.
pub(crate) fn ensure_tables(conn: &Connection) -> Result<(), IcxError> {
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS lex_words USING fts5(body, parts, \
           tokenize = \"porter unicode61 remove_diacritics 2 tokenchars '_'\", content = '');
         CREATE VIRTUAL TABLE IF NOT EXISTS lex_trigrams USING fts5(body, \
           tokenize = 'trigram', detail = none, content = '');",
    )
    .map_err(db_err)?;
    conn.execute(
        "INSERT INTO meta (k, v) VALUES ('lexical_index', ?1) \
         ON CONFLICT(k) DO UPDATE SET v = excluded.v",
        params![LEXICAL_VERSION],
    )
    .map_err(db_err)?;
    Ok(())
}

/// Whether the lexical tables exist in this store.
pub(crate) fn tables_exist(conn: &Connection) -> Result<bool, IcxError> {
    let n: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name IN ('lex_words', 'lex_trigrams')",
            [],
            |r| r.get(0),
        )
        .map_err(db_err)?;
    Ok(n == 2)
}

/// Drops the tables (contentless rows cannot be deleted one by one); the next save with the
/// index enabled rebuilds them.
pub(crate) fn drop_tables(conn: &Connection) -> Result<(), IcxError> {
    conn.execute_batch(
        "DROP TABLE IF EXISTS lex_words; DROP TABLE IF EXISTS lex_trigrams;
         DELETE FROM meta WHERE k = 'lexical_index';",
    )
    .map_err(db_err)
}

/// Ids already in the index.
pub(crate) fn indexed_ids(
    conn: &Connection,
) -> Result<std::collections::BTreeSet<ArtifactId>, IcxError> {
    let mut stmt = conn
        .prepare("SELECT rowid FROM lex_words")
        .map_err(db_err)?;
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
pub(crate) fn insert(conn: &Connection, id: ArtifactId, text: &str) -> Result<(), IcxError> {
    let body = indexed_prefix(text);
    let parts = identifier_parts(body);
    conn.prepare_cached("INSERT INTO lex_words (rowid, body, parts) VALUES (?1, ?2, ?3)")
        .map_err(db_err)?
        .execute(params![id_to_sql(id), body, parts])
        .map_err(db_err)?;
    conn.prepare_cached("INSERT INTO lex_trigrams (rowid, body) VALUES (?1, ?2)")
        .map_err(db_err)?
        .execute(params![id_to_sql(id), body])
        .map_err(db_err)?;
    Ok(())
}

/// The parts of every compound identifier in `text`, lowercased and space-separated:
/// `parseHttpRequest` gives `parse http request`, `shell_words` gives `shell words`,
/// `icx-adapters` gives `icx adapters`. A plain word gives nothing (it is in the body already).
#[must_use]
pub fn identifier_parts(text: &str) -> String {
    let mut out = String::new();
    for word in text.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-')) {
        let parts = split_identifier(word);
        if parts.len() > 1 {
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

/// Splits one identifier at `_`, `-`, lower-to-upper and letter-digit boundaries, and before the
/// last capital of an acronym run (`HTTPServer` gives `HTTP`, `Server`).
fn split_identifier(word: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    for piece in word.split(['_', '-']).filter(|p| !p.is_empty()) {
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
/// joined by OR, at most [`MAX_QUERY_TERMS`]. Quoting makes every term a literal, so no query
/// text is read as FTS5 syntax. `None` when the query has no words.
#[must_use]
pub fn words_query(query: &str) -> Option<String> {
    let mut seen = std::collections::BTreeSet::new();
    let terms: Vec<String> = query
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
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
            "SELECT rowid, bm25(lex_words, 1.0, 0.5) AS s FROM lex_words \
             WHERE lex_words MATCH ?1 ORDER BY s, rowid LIMIT ?2",
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

/// Ids of artifacts holding every trigram of `needle` (at least 3 characters), at most `limit`.
/// A superset of the artifacts containing `needle`; the caller confirms each against its text.
pub fn trigram_candidates(
    conn: &Connection,
    needle: &str,
    limit: usize,
) -> Result<Vec<ArtifactId>, IcxError> {
    let chars: Vec<char> = needle.chars().collect();
    if chars.len() < 3 {
        return Ok(Vec::new());
    }
    let mut seen = std::collections::BTreeSet::new();
    let terms: Vec<String> = chars
        .windows(3)
        .map(|w| w.iter().collect::<String>())
        .filter(|t| seen.insert(t.clone()))
        .take(MAX_QUERY_TERMS)
        .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
        .collect();
    let mut stmt = conn
        .prepare_cached("SELECT rowid FROM lex_trigrams WHERE lex_trigrams MATCH ?1 LIMIT ?2")
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
        assert_eq!(split_identifier("icx-adapters"), ["icx", "adapters"]);
        assert_eq!(split_identifier("v1beta2"), ["v", "1", "beta", "2"]);
        assert_eq!(split_identifier("plain"), ["plain"]);
        assert_eq!(
            identifier_parts("call parseHttpRequest now"),
            "parse http request"
        );
        assert_eq!(identifier_parts("plain words only"), "");
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
}
