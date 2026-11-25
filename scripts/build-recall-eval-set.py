#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 DoubleGate
# SPDX-License-Identifier: GPL-3.0-or-later AND AGPL-3.0-or-later
"""Builds the real-session retrieval evaluation set (T-72-001) from Claude Code transcripts.

Usage:
  python3 -I scripts/build-recall-eval-set.py <transcript dir> [<transcript dir> ...] \
      --out eval/private/real-sessions.jsonl

Each case is a real need with a known answer: the agent dereferenced (`deref` or `slice`) an
icx handle it got from a stub, and the query is the agent's own text just before that call.
Handles that first appeared in an injected recall line are skipped, so the set does not favour
the ranker that surfaced them. Handles and long hex runs are removed from the query, so the
answer is never in it. Sessions are ordered by their first timestamp; the older half of the
cases is `train`, the newer half `heldout` (the held-out split is never used for tuning).

The output holds queries written in real sessions, so it must stay private: write it under
`eval/private/` (gitignored), and commit only aggregate scores. The script prints counts only.
"""

import argparse
import glob
import json
import os
import re
import sys

HANDLE = re.compile(r"#icx:[0-9a-f]{12,16}")
HEX_RUN = re.compile(r"\b[0-9a-f]{8,}\b")
RECALL_HANDLE = re.compile(r"HDC_RECALL[^\n]*?↦(#icx:[0-9a-f]{16})")
MAX_QUERY_CHARS = 300
MIN_QUERY_WORDS = 3


def clean_query(text: str) -> str:
    text = HANDLE.sub(" ", text)
    text = HEX_RUN.sub(" ", text)
    text = " ".join(text.split())
    return text[:MAX_QUERY_CHARS]


def is_user_prompt(entry: dict) -> bool:
    """A genuine user turn, not a tool result (which Claude Code also records as `user`)."""
    if entry.get("type") != "user":
        return False
    content = (entry.get("message") or {}).get("content")
    if isinstance(content, str):
        return True
    if isinstance(content, list):
        return not any(isinstance(c, dict) and c.get("type") == "tool_result" for c in content)
    return False


def cases_from(path: str) -> tuple[str, list[dict]]:
    first_ts = ""
    last_text = ""
    from_recall: set[str] = set()
    out: list[dict] = []
    seen: set[tuple[str, str]] = set()
    with open(path, encoding="utf-8", errors="replace") as f:
        for line in f:
            for h in RECALL_HANDLE.findall(line):
                from_recall.add(h)
            try:
                entry = json.loads(line)
            except json.JSONDecodeError:
                continue
            if not first_ts:
                first_ts = str(entry.get("timestamp") or "")
            if is_user_prompt(entry):
                last_text = ""
                continue
            if entry.get("type") != "assistant":
                continue
            content = (entry.get("message") or {}).get("content")
            if not isinstance(content, list):
                continue
            for block in content:
                if not isinstance(block, dict):
                    continue
                if block.get("type") == "text":
                    last_text = str(block.get("text") or "")
                elif block.get("type") == "tool_use" and str(block.get("name", "")).startswith(
                    "mcp__icx__"
                ):
                    args = block.get("input") or {}
                    handle = str(args.get("id") or "")
                    if args.get("op") not in ("deref", "slice") or not HANDLE.fullmatch(handle):
                        continue
                    if handle in from_recall:
                        continue
                    query = clean_query(last_text)
                    if len(query.split()) < MIN_QUERY_WORDS:
                        continue
                    key = (query, handle)
                    if key not in seen:
                        seen.add(key)
                        out.append({"query": query, "relevant": [handle]})
    return first_ts, out


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("dirs", nargs="+")
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    if "eval/private/" not in a.out.replace(os.sep, "/"):
        sys.exit("refusing to write outside eval/private/: the set holds private session text")
    sessions = []
    for d in a.dirs:
        for path in sorted(glob.glob(os.path.join(d, "*.jsonl"))):
            ts, cases = cases_from(path)
            if cases:
                sessions.append((ts, cases))
    sessions.sort(key=lambda s: s[0])
    total = sum(len(c) for _, c in sessions)
    done = 0
    rows = []
    for _, cases in sessions:
        split = "train" if done < total / 2 else "heldout"
        for c in cases:
            rows.append({**c, "split": split})
        done += len(cases)
    os.makedirs(os.path.dirname(a.out), exist_ok=True)
    with open(a.out, "w", encoding="utf-8") as f:
        for r in rows:
            f.write(json.dumps(r, ensure_ascii=False) + "\n")
    os.chmod(a.out, 0o600)
    n_train = sum(r["split"] == "train" for r in rows)
    print(
        json.dumps(
            {
                "sessions": len(sessions),
                "cases": len(rows),
                "train": n_train,
                "heldout": len(rows) - n_train,
            }
        )
    )


if __name__ == "__main__":
    main()
