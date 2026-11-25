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
- `prompt`: the user's prompt that started the turn, timed at the prompt (when automatic recall
  runs); every handle dereferenced before the next prompt is relevant. Outputs produced during the
  turn did not exist yet, so the harness drops cases whose answer came later.
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
import pathlib
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
    prompt_ts = ""
    # One case per user turn (review of #98: keyed by turn, not by prompt text, and timed at the
    # prompt, when automatic recall runs).
    turns: list[dict] = []
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
                prompt_ts = ts
                turns.append({"query": prompt, "relevant": [], "ts": prompt_ts})
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
                        if turns and handle not in turns[-1]["relevant"]:
                            turns[-1]["relevant"].append(handle)
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
            t
            for t in turns
            if t["relevant"] and len(t["query"].split()) >= MIN_QUERY_WORDS
        ]
    return first_ts, out


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("dirs", nargs="+")
    ap.add_argument("--out", required=True)
    ap.add_argument("--kind", choices=["note", "prompt"], default="note")
    a = ap.parse_args()
    # The output must resolve inside this repository's gitignored eval/private/ (review of
    # #98: a substring test let `../` or another `eval/private/` through).
    private = (
        pathlib.Path(__file__).resolve().parent.parent / "eval" / "private"
    ).resolve()
    out_path = pathlib.Path(a.out).resolve()
    if not out_path.is_relative_to(private) or out_path == private:
        sys.exit(
            f"refusing to write outside {private}: the set holds private session text"
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
            # No time is written as no time (review of #98), never as "".
            **({"ts": c["ts"]} if c["ts"] else {}),
        }
        for i, c in enumerate(cases)
    ]
    out_path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    # Created 0600, never readable by others even briefly (review of #98).
    fd = os.open(out_path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    os.fchmod(fd, 0o600)
    with os.fdopen(fd, "w", encoding="utf-8") as f:
        f.writelines(json.dumps(r, ensure_ascii=False) + "\n" for r in rows)
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
