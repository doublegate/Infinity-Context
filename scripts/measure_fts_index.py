#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 DoubleGate
# SPDX-License-Identifier: GPL-3.0-or-later
"""Size and build time of SQLite FTS5 indexes over a workspace's stored outputs (T-91-003).

Reads a workspace's `icx.sqlite3` read-only, reassembles each artifact's text from its chunks,
and builds one throwaway FTS5 index per tokenizer configuration in an output directory, reporting
index size against the stored text and the insert time per artifact. It is the evidence step for
choosing a `grep_cold` index: nothing in the workspace is modified, and the indexes it writes
hold stored (already redacted) text, so keep the output directory private and delete it after.

Usage:
  python3 scripts/measure_fts_index.py ~/.local/state/icx/workspaces/<id>
  python3 scripts/measure_fts_index.py path/to/icx.sqlite3 --out /tmp/fts --json results.json
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
    "trigram": "tokenize='trigram'",
    "trigram-detail-none": "tokenize='trigram', detail=none",
}


def db_id(chunk_id: int) -> int:
    """The store keeps a u64 `ArtifactId` bit-cast to SQLite's signed 64-bit integer."""
    return chunk_id - (1 << 64) if chunk_id >= 1 << 63 else chunk_id


def load_texts(db: Path) -> list[str]:
    src = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    try:
        chunks = dict(src.execute("SELECT id, bytes FROM chunks"))
        texts = []
        for (meta,) in src.execute("SELECT meta FROM artifacts ORDER BY id"):
            ids = json.loads(meta).get("chunk_ids", [])
            missing = [c for c in ids if db_id(c) not in chunks]
            if missing:
                raise SystemExit(
                    f"artifact references {len(missing)} missing chunk(s); store damaged?"
                )
            body = b"".join(chunks[db_id(c)] for c in ids)
            texts.append(body.decode("utf-8", "replace"))
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


def main() -> int:
    ap = argparse.ArgumentParser(
        description="Measure SQLite FTS5 index size over a workspace's stored outputs."
    )
    ap.add_argument("store", type=Path, help="workspace directory or its icx.sqlite3")
    ap.add_argument(
        "--out", type=Path, help="directory for the throwaway indexes (default: temp)"
    )
    ap.add_argument("--json", type=Path, help="also write the results as JSON here")
    ap.add_argument("--keep", action="store_true", help="keep the index files")
    args = ap.parse_args()

    db = args.store / "icx.sqlite3" if args.store.is_dir() else args.store
    if not db.is_file():
        print(f"no store at {db}", file=sys.stderr)
        return 2
    texts = load_texts(db)
    total = sum(len(t.encode()) for t in texts)
    if not texts:
        print("the store holds no artifacts", file=sys.stderr)
        return 2
    out = args.out or Path(tempfile.mkdtemp(prefix="icx-fts-"))
    out.mkdir(parents=True, exist_ok=True)

    results = {
        "artifacts": len(texts),
        "text_bytes": total,
        "sqlite_version": sqlite3.sqlite_version,
    }
    print(
        f"artifacts={len(texts)} text={total / 1e6:.1f} MB (SQLite {sqlite3.sqlite_version})"
    )
    for name, options in CONFIGS.items():
        path = out / f"{name}.db"
        r = build(texts, path, options)
        r["ratio_to_text"] = round(r["bytes"] / total, 3)
        r["ms_per_artifact"] = round(r["insert_s"] / len(texts) * 1e3, 3)
        results[name] = r
        print(
            f"{name}: {r['bytes'] / 1e6:.1f} MB ({r['ratio_to_text']:.2f}x text), "
            f"insert {r['insert_s']:.1f} s ({r['ms_per_artifact']:.2f} ms/artifact)"
        )
        if not args.keep:
            path.unlink()
    if args.json:
        args.json.write_text(json.dumps(results, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
