#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 DoubleGate
# SPDX-License-Identifier: GPL-3.0-or-later AND AGPL-3.0-or-later
"""Builds the real-session retrieval evaluation set (T-72-001) from Claude Code transcripts.

Usage:
  python3 -I scripts/build-recall-eval-set.py <transcript dir> [<transcript dir> ...] \
      --kind <note|prompt> --out eval/private/<name>.jsonl

Each case is a real need with a known answer: the agent dereferenced (`deref` or `slice`) an
icx handle it got from a stub. The query is, by `--kind`:
- `note`: the agent's own text just before that call;
- `prompt`: the user's prompt that started the turn (all handles dereferenced before the next
  prompt are relevant). This is what automatic recall searches with, so it is the set that
  decides a default ranking.
Handles that first appeared in an injected recall line are skipped, so the set does not favour
the ranker that surfaced them. Handles and long hex runs are removed from the query, so the
answer is never in it. Cases are ordered by the time of the dereference; the older half is
`train`, the newer half `heldout` (the held-out split is never used for tuning). Subagent
transcripts (`<session>/subagents/*.jsonl`) are read too.

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
        return not any(
            isinstance(c, dict) and c.get("type") == "tool_result" for c in content
        )
    return False


NOISE = re.compile(
    r"<system-reminder>.*?</system-reminder>|\u27ea\s*ICX:[^\u27eb]*\u27eb", re.DOTALL
)


def prompt_text(entry: dict) -> str:
    content = (entry.get("message") or {}).get("content")
    if isinstance(content, list):
        content = " ".join(
            str(c.get("text") or "")
            for c in content
            if isinstance(c, dict) and c.get("type") == "text"
        )
    return NOISE.sub(" ", str(content or ""))


def cases_from(path: str, kind: str) -> tuple[str, list[dict]]:
    first_ts = ""
    last_text = ""
    prompt = ""
    turn: dict[str, list[str]] = {}
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
            ts = str(entry.get("timestamp") or "")
            if not first_ts:
                first_ts = ts
            if is_user_prompt(entry):
                last_text = ""
                prompt = clean_query(prompt_text(entry))
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
                elif block.get("type") == "tool_use" and str(
                    block.get("name", "")
                ).startswith("mcp__icx__"):
                    args = block.get("input") or {}
                    handle = str(args.get("id") or "")
                    if args.get("op") not in ("deref", "slice") or not HANDLE.fullmatch(
                        handle
                    ):
                        continue
                    if handle in from_recall:
                        continue
                    if kind == "prompt":
                        if len(prompt.split()) >= MIN_QUERY_WORDS:
                            hs = turn.setdefault(prompt, {"ts": ts, "relevant": []})
                            if handle not in hs["relevant"]:
                                hs["relevant"].append(handle)
                        continue
                    query = clean_query(last_text)
                    if len(query.split()) < MIN_QUERY_WORDS:
                        continue
                    key = (query, handle)
                    if key not in seen:
                        seen.add(key)
                        out.append({"query": query, "relevant": [handle], "ts": ts})
    if kind == "prompt":
        out = [
            {"query": q, "relevant": v["relevant"], "ts": v["ts"]}
            for q, v in turn.items()
        ]
    return first_ts, out


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("dirs", nargs="+")
    ap.add_argument("--out", required=True)
    ap.add_argument("--kind", choices=["note", "prompt"], default="note")
    a = ap.parse_args()
    if "eval/private/" not in a.out.replace(os.sep, "/"):
        sys.exit(
            "refusing to write outside eval/private/: the set holds private session text"
        )
    sessions = 0
    cases: list[dict] = []
    for d in a.dirs:
        paths = glob.glob(os.path.join(d, "*.jsonl")) + glob.glob(
            os.path.join(d, "*", "subagents", "*.jsonl")
        )
        for path in sorted(paths):
            _, found = cases_from(path, a.kind)
            if found:
                sessions += 1
                cases.extend(found)
    cases.sort(key=lambda c: c["ts"])
    half = len(cases) // 2
    rows = [
        {
            "query": c["query"],
            "relevant": c["relevant"],
            "split": "train" if i < half else "heldout",
            "ts": c["ts"],
        }
        for i, c in enumerate(cases)
    ]
    os.makedirs(os.path.dirname(a.out), exist_ok=True)
    with open(a.out, "w", encoding="utf-8") as f:
        f.writelines(json.dumps(r, ensure_ascii=False) + "\n" for r in rows)
    os.chmod(a.out, 0o600)
    n_train = sum(r["split"] == "train" for r in rows)
    print(
        json.dumps(
            {
                "transcripts": sessions,
                "cases": len(rows),
                "train": n_train,
                "heldout": len(rows) - n_train,
            }
        )
    )


if __name__ == "__main__":
    main()
