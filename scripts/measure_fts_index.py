#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 DoubleGate
# SPDX-License-Identifier: GPL-3.0-or-later
"""Size and build time of SQLite FTS5 indexes over a workspace's stored outputs (T-91-003).

Opens a workspace's `icx.sqlite3` read-only, reassembles each artifact's text from its chunks,
and builds one throwaway FTS5 index per tokenizer configuration, reporting index size against the
stored text and the insert time per artifact. It is the evidence step for choosing a `grep_cold`
index. The store's contents are not changed (SQLite may still touch the `-shm` file of a store in
WAL mode). The indexes hold stored, already redacted text, so they are written to a private
(0700) directory and deleted afterwards unless `--keep` is given.

Configurations ending in `-contentless` (`content=''`) store the index only, as an index over the
existing `chunks` table would; the others also keep a copy of the text in the FTS5 table. Insert
time is one bulk transaction over the whole store (backfill cost), not the latency of indexing
one output as it is stored; `optimize` and `VACUUM` are not timed.

Usage:
  python3 scripts/measure_fts_index.py ~/.local/state/icx/workspaces/<id>
  python3 scripts/measure_fts_index.py path/to/icx.sqlite3 --json results.json
"""

from __future__ import annotations

import argparse
import json
import sqlite3
import sys
import tempfile
import time
from pathlib import Path

CONFIGS = {
    "porter": "tokenize='porter unicode61 remove_diacritics 2'",
    "porter-detail-column": "tokenize='porter unicode61 remove_diacritics 2', detail=column",
    "porter-contentless": "tokenize='porter unicode61 remove_diacritics 2', content=''",
    "trigram": "tokenize='trigram'",
    "trigram-detail-none": "tokenize='trigram', detail=none",
    "trigram-contentless": "tokenize='trigram', content=''",
}


def db_id(chunk_id: int) -> int:
    """The store keeps a u64 `ArtifactId` bit-cast to SQLite's signed 64-bit integer."""
    return chunk_id - (1 << 64) if chunk_id >= 1 << 63 else chunk_id


def load_texts(db: Path) -> list[str]:
    src = sqlite3.connect(f"{db.resolve().as_uri()}?mode=ro", uri=True)
    try:
        texts = []
        for (meta,) in src.execute("SELECT meta FROM artifacts ORDER BY id"):
            parts = []
            for cid in json.loads(meta).get("chunk_ids", []):
                row = src.execute(
                    "SELECT bytes FROM chunks WHERE id = ?", (db_id(cid),)
                ).fetchone()
                if row is None:
                    raise SystemExit(
                        "an artifact references a missing chunk; store damaged?"
                    )
                parts.append(row[0])
            texts.append(b"".join(parts).decode("utf-8", "replace"))
        return texts
    finally:
        src.close()


def build(texts: list[str], path: Path, options: str) -> dict:
    path.unlink(missing_ok=True)
    out = sqlite3.connect(path)
    try:
        out.execute(f"CREATE VIRTUAL TABLE x USING fts5(body, {options})")
        start = time.perf_counter()
        out.executemany("INSERT INTO x(rowid, body) VALUES (?, ?)", enumerate(texts))
        out.commit()
        elapsed = time.perf_counter() - start
        out.execute("INSERT INTO x(x) VALUES('optimize')")
        out.commit()
        out.execute("VACUUM")
    finally:
        out.close()
    return {"bytes": path.stat().st_size, "insert_s": round(elapsed, 3)}


def measure(texts: list[str], total: int, out: Path, keep: bool) -> dict:
    results: dict = {}
    for name, options in CONFIGS.items():
        path = out / f"{name}.db"
        try:
            r = build(texts, path, options)
        finally:
            if not keep:
                path.unlink(missing_ok=True)
        r["ratio_to_text"] = round(r["bytes"] / total, 3)
        r["ms_per_artifact"] = round(r["insert_s"] / len(texts) * 1e3, 3)
        results[name] = r
        print(
            f"{name}: {r['bytes'] / 1e6:.1f} MB ({r['ratio_to_text']:.2f}x text), "
            f"insert {r['insert_s']:.1f} s ({r['ms_per_artifact']:.2f} ms/artifact)"
        )
    return results


def main() -> int:
    ap = argparse.ArgumentParser(
        description="Measure SQLite FTS5 index size over a workspace's stored outputs."
    )
    ap.add_argument("store", type=Path, help="workspace directory or its icx.sqlite3")
    ap.add_argument(
        "--out", type=Path, help="directory for the indexes (default: private temp)"
    )
    ap.add_argument("--json", type=Path, help="also write the results as JSON here")
    ap.add_argument("--keep", action="store_true", help="keep the index files")
    args = ap.parse_args()

    db = args.store / "icx.sqlite3" if args.store.is_dir() else args.store
    if not db.is_file():
        print(f"no store at {db}", file=sys.stderr)
        return 2
    # Never change an existing directory's mode; refuse one others can read.
    if (
        args.out
        and args.out.exists()
        and (not args.out.is_dir() or args.out.stat().st_mode & 0o077)
    ):
        print(
            f"{args.out} must be a directory only you can access (0700)",
            file=sys.stderr,
        )
        return 2
    texts = load_texts(db)
    total = sum(len(t.encode()) for t in texts)
    if total == 0:
        print("the store holds no text", file=sys.stderr)
        return 2

    results = {
        "artifacts": len(texts),
        "text_bytes": total,
        "sqlite_version": sqlite3.sqlite_version,
    }
    print(
        f"artifacts={len(texts)} text={total / 1e6:.1f} MB (SQLite {sqlite3.sqlite_version})"
    )
    if args.out:
        if not args.out.exists():
            args.out.mkdir(mode=0o700, parents=True)
        results.update(measure(texts, total, args.out, args.keep))
    elif args.keep:
        out = Path(tempfile.mkdtemp(prefix="icx-fts-"))  # mkdtemp creates it 0700
        results.update(measure(texts, total, out, keep=True))
        print(f"indexes kept in {out}")
    else:
        with tempfile.TemporaryDirectory(prefix="icx-fts-") as tmp:
            results.update(measure(texts, total, Path(tmp), keep=False))
    if args.json:
        args.json.write_text(json.dumps(results, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
