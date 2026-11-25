#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 DoubleGate
# SPDX-License-Identifier: GPL-3.0-or-later AND AGPL-3.0-or-later
"""Measures the v1.5.0 lexical index (T-71-001) on a copy of a workspace store.

Usage: python3 -I scripts/measure-lexical-index.py <path/to/icx.sqlite3> <scratch dir>

The store is opened read-only. Each artifact's text is rebuilt from its chunks, then indexed into
fresh tables in the scratch directory, with the same definitions as `crates/icx-adapters/src/
lexical.rs`. Prints sizes, insert time per artifact, and query times. Nothing from the store is
printed, so the output is safe to publish.
"""

import json
import os
import sqlite3
import statistics
import sys
import time

TABLES = {
    "words": "CREATE VIRTUAL TABLE x USING fts5(body, parts, "
    "tokenize = \"porter unicode61 remove_diacritics 2 tokenchars '_'\", content = '')",
    "trigrams": "CREATE VIRTUAL TABLE x USING fts5(body, tokenize = 'trigram', "
    "detail = none, content = '')",
}
WORD_QUERIES = [
    '"cargo" OR "test" OR "failed"',
    '"error" OR "borrow"',
    '"session" OR "hook"',
]
NEEDLES = ["error", "install", "workspace"]


def texts(store: str) -> list[str]:
    src = sqlite3.connect(f"file:{store}?mode=ro", uri=True)
    chunks = dict(src.execute("SELECT id, bytes FROM chunks"))
    out = []
    for (meta,) in src.execute("SELECT meta FROM artifacts"):
        ids = json.loads(meta)["chunk_ids"]
        raw = b"".join(chunks.get(c if c < 2**63 else c - 2**64, b"") for c in ids)
        out.append(raw[: 1 << 20].decode("utf-8", "replace"))
    return out


def median_ms(run, n: int = 50) -> tuple[float, float]:
    xs = []
    for _ in range(n):
        t0 = time.perf_counter()
        run()
        xs.append((time.perf_counter() - t0) * 1e3)
    return statistics.median(xs), max(xs)


def main() -> None:
    store, out = sys.argv[1], sys.argv[2]
    os.makedirs(out, exist_ok=True)
    docs = texts(store)
    print(
        f"artifacts={len(docs)} text={sum(len(t.encode()) for t in docs) / 1e6:.1f} MB"
    )
    for name, ddl in TABLES.items():
        path = os.path.join(out, f"lex-{name}.db")
        if os.path.exists(path):
            os.remove(path)
        db = sqlite3.connect(path)
        db.execute(ddl)
        t0 = time.perf_counter()
        if name == "words":
            db.executemany(
                "INSERT INTO x (rowid, body, parts) VALUES (?, ?, '')", enumerate(docs)
            )
        else:
            db.executemany("INSERT INTO x (rowid, body) VALUES (?, ?)", enumerate(docs))
        db.commit()
        per = (time.perf_counter() - t0) / max(len(docs), 1) * 1e3
        db.execute("INSERT INTO x (x) VALUES ('optimize')")
        db.commit()
        db.execute("VACUUM")
        print(
            f"{name}: {os.path.getsize(path) / 1e6:.1f} MB, insert {per:.2f} ms/artifact"
        )
        if name == "words":
            for q in WORD_QUERIES:
                sql = "SELECT rowid, bm25(x, 1.0, 0.5) s FROM x WHERE x MATCH ? ORDER BY s LIMIT 20"
                m, worst = median_ms(
                    lambda q=q, db=db, sql=sql: db.execute(sql, (q,)).fetchall()
                )
                print(
                    f"  bm25 {len(q.split(' OR '))} terms: median {m:.2f} ms, worst {worst:.2f} ms"
                )
        else:
            for needle in NEEDLES:
                q = " AND ".join(
                    f'"{needle[i : i + 3]}"' for i in range(len(needle) - 2)
                )
                sql = "SELECT rowid FROM x WHERE x MATCH ? LIMIT 160"
                m, worst = median_ms(
                    lambda q=q, db=db, sql=sql: db.execute(sql, (q,)).fetchall()
                )
                print(
                    f"  trigram {len(needle)} chars: median {m:.2f} ms, worst {worst:.2f} ms"
                )
        db.close()


if __name__ == "__main__":
    main()
