#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 DoubleGate
# SPDX-License-Identifier: GPL-3.0-or-later AND AGPL-3.0-or-later
"""Paired provider prompt-cache measurement for `icx proxy` (T-81-001 to T-81-003).

For each provider with a key in the environment, the same deterministic multi-turn
conversation (salted per arm and per run, so arms never share a cache entry) is sent per turn: once directly to the provider with no cache controls,
and once through a private `icx proxy` (fresh workspace and state directories, loopback port,
no other client). The provider's own usage counters are recorded for both arms, and the
proxy's `proxy-cache-usage.jsonl` ledger is copied into the results.

Keys come only from the environment (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`); they are never
printed or written. The run refuses to start when its worst-case cost estimate exceeds
`--budget` (US dollars), and `--dry-run` prints the plan without any network call.

Usage:
  python3 scripts/measure_provider_cache.py --dry-run
  set -a; . ~/.claude-secrets/provider-keys; set +a
  python3 scripts/measure_provider_cache.py --budget 5 --out /tmp/icx-cache-run.json
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

TURNS = 4
HISTORY_PAIRS = 4  # user/assistant pairs before the first measured turn
MESSAGE_CHARS = (
    1500  # under the proxy's 2 KiB folding threshold, so content is unchanged
)
MAX_OUTPUT_TOKENS = 8
ANTHROPIC_MODEL = "claude-haiku-5-5"
# Worst-case prices in US dollars per million tokens, used only for the refusal check:
# input at the cache-write rate (2x for a 1-hour write), output as listed.
ANTHROPIC_PRICE = {"input": 0.10 * 2, "output": 0.50}
OPENAI_PRICE_CEILING = {"input": 5.00 * 2, "output": 40.00}
OPENAI_CANDIDATES = ("gpt-5.6-nano", "gpt-5.6-mini", "gpt-5.6")


def filler(tag: str, n: int) -> str:
    """Deterministic, low-entropy text that is stable across turns."""
    words = (
        "the epoch controller keeps the prompt prefix stable while the tail slot moves "
        "forward each turn so the provider can reuse the cached prefix "
    )
    text = f"[{tag}] "
    while len(text) < n:
        text += words
    return text[:n]


def conversation(turn: int, arm: str = "") -> list[dict]:
    """Append-only, as a real conversation is: turn `t + 1` is turn `t` plus the reply to its
    last user message and one new user message. `arm` salts every message, so arms never
    share a provider cache entry."""

    def user(i: int) -> dict:
        text = filler(f"{arm}u{i}", MESSAGE_CHARS - 40) + " Reply with the word ok."
        return {"role": "user", "content": text}

    msgs = []
    for i in range(HISTORY_PAIRS + turn):
        msgs.append(user(i))
        msgs.append(
            {"role": "assistant", "content": filler(f"{arm}a{i}", MESSAGE_CHARS)}
        )
    msgs.append(user(HISTORY_PAIRS + turn))
    return msgs


def approx_tokens(msgs: list[dict]) -> int:
    return sum(len(m["content"]) for m in msgs) // 3 + 50


def plan(providers: list[str]) -> dict:
    per = []
    total = 0.0
    for p in providers:
        price = ANTHROPIC_PRICE if p == "anthropic" else OPENAI_PRICE_CEILING
        cost = 0.0
        tokens = 0
        arms = 3 if p == "anthropic" else 2
        for t in range(TURNS):
            n = approx_tokens(conversation(t))
            tokens += arms * n
            cost += (
                arms * (n * price["input"] + MAX_OUTPUT_TOKENS * price["output"]) / 1e6
            )
        per.append(
            {
                "provider": p,
                "requests": arms * TURNS,
                "input_tokens_est": tokens,
                "worst_case_usd": round(cost, 4),
            }
        )
        total += cost
    return {"turns": TURNS, "providers": per, "worst_case_total_usd": round(total, 4)}


def post(url: str, headers: dict, body: dict) -> dict:
    req = urllib.request.Request(
        url, data=json.dumps(body).encode(), headers=headers, method="POST"
    )
    try:
        with urllib.request.urlopen(req, timeout=120) as r:
            return json.loads(r.read().decode())
    except urllib.error.HTTPError as e:
        # The error body is the provider's; it carries no key.
        return {"http_error": e.code, "body": e.read().decode(errors="replace")[:500]}


def free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def start_proxy(icx: str, tmp: Path) -> tuple[subprocess.Popen, str, Path]:
    env = dict(os.environ)
    env.update(
        {
            "ICX_CONFIG_HOME": str(tmp / "config"),
            "ICX_STATE_HOME": str(tmp / "state"),
            "XDG_CONFIG_HOME": str(tmp / "xdg-config"),
            "XDG_STATE_HOME": str(tmp / "xdg-state"),
            "ICX_NO_DAEMON": "1",
        }
    )
    ws = tmp / "ws"
    subprocess.run(
        [icx, "init", "--dir", str(ws)],
        env=env,
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    port = free_port()
    proc = subprocess.Popen(
        [
            icx,
            "proxy",
            "--listen",
            f"127.0.0.1:{port}",
            "--dir",
            str(ws),
            "--insecure-no-auth",
        ],
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    for _ in range(50):
        if proc.poll() is not None:
            raise RuntimeError(f"icx proxy exited early with status {proc.returncode}")
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=0.2):
                return proc, f"http://127.0.0.1:{port}", ws
        except OSError:
            time.sleep(0.1)
    proc.kill()
    raise RuntimeError("icx proxy did not start listening within 5 s")


def anthropic_request(
    base: str, key: str, msgs: list[dict], client_cache: bool = False
) -> dict:
    headers = {
        "x-api-key": key,
        "anthropic-version": "2023-06-01",
        "content-type": "application/json",
    }
    body = {
        "model": ANTHROPIC_MODEL,
        "max_tokens": MAX_OUTPUT_TOKENS,
        "thinking": {"type": "disabled"},
        "messages": msgs,
    }
    if client_cache:
        # What a caching client (Claude Code) does: automatic caching at the top level.
        body["cache_control"] = {"type": "ephemeral"}
    return post(f"{base}/v1/messages", headers, body)


def openai_model(key: str) -> str | None:
    req = urllib.request.Request(
        "https://api.openai.com/v1/models", headers={"authorization": f"Bearer {key}"}
    )
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            ids = {m["id"] for m in json.loads(r.read().decode()).get("data", [])}
    except urllib.error.URLError:
        return None
    return next((m for m in OPENAI_CANDIDATES if m in ids), None)


def openai_request(base: str, key: str, model: str, msgs: list[dict]) -> dict:
    headers = {"authorization": f"Bearer {key}", "content-type": "application/json"}
    body = {
        "model": model,
        "max_completion_tokens": MAX_OUTPUT_TOKENS,
        "messages": msgs,
    }
    return post(f"{base}/v1/chat/completions", headers, body)


def usage_of(resp: dict) -> dict:
    u = resp.get("usage") or {}
    keep = {
        k: u.get(k)
        for k in (
            "input_tokens",
            "cache_read_input_tokens",
            "cache_creation_input_tokens",
            "prompt_tokens",
        )
    }
    if "prompt_tokens_details" in u:
        keep["prompt_tokens_details"] = u["prompt_tokens_details"]
    if "http_error" in resp:
        keep["http_error"] = resp["http_error"]
        keep["error_body"] = resp.get("body")
    return keep


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--budget", type=float, default=5.0)
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument(
        "--icx", default=str(Path(__file__).resolve().parents[1] / "target/release/icx")
    )
    ap.add_argument("--out", default="")
    args = ap.parse_args()

    keys = {
        "anthropic": os.environ.get("ANTHROPIC_API_KEY", "").strip(),
        "openai": os.environ.get("OPENAI_API_KEY", "").strip(),
    }
    providers = [p for p in ("anthropic", "openai") if keys[p] or args.dry_run]
    manifest = plan(providers)
    manifest["models"] = {
        "anthropic": ANTHROPIC_MODEL,
        "openai": list(OPENAI_CANDIDATES),
    }
    manifest["budget_usd"] = args.budget
    print(json.dumps({"plan": manifest}, indent=2))
    if args.dry_run:
        return 0
    if not providers:
        print("no provider key in the environment; nothing to do", file=sys.stderr)
        return 2
    if manifest["worst_case_total_usd"] > args.budget:
        print(
            "worst-case estimate exceeds the budget; refusing to run", file=sys.stderr
        )
        return 3

    run_id = os.urandom(4).hex()
    results: dict = {"plan": manifest, "runs": {}, "run_id": run_id}
    tmp = Path(tempfile.mkdtemp(prefix="icx-cache-run-"))
    try:
        proc, proxy_base, ws = start_proxy(args.icx, tmp)
    except (OSError, RuntimeError, subprocess.CalledProcessError):
        shutil.rmtree(tmp, ignore_errors=True)
        raise
    try:
        for p in providers:
            model = ANTHROPIC_MODEL
            if p == "openai":
                model = openai_model(keys[p])
                if model is None:
                    results["runs"][p] = {
                        "skipped": "no GPT-5.6 model visible to this key"
                    }
                    continue
            rows = []
            for t in range(TURNS):
                row: dict = {"turn": t}
                direct = (
                    "https://api.anthropic.com"
                    if p == "anthropic"
                    else "https://api.openai.com"
                )
                # Anthropic caches only on request, so a client that asks for caching is the fair
                # baseline (`direct_client_cache`); OpenAI caches automatically, so its `direct`
                # arm already is.
                arms = [("direct", direct, False), ("proxy", proxy_base, False)]
                if p == "anthropic":
                    arms.append(("direct_client_cache", direct, True))
                for arm, base, client_cache in arms:
                    # A per-run nonce plus the arm name: no arm reads another's cache entry,
                    # nor an entry left by an earlier run.
                    msgs = conversation(t, f"{run_id}:{arm}:")
                    if p == "anthropic":
                        resp = anthropic_request(base, keys[p], msgs, client_cache)
                    else:
                        resp = openai_request(base, keys[p], model, msgs)
                    row[arm] = usage_of(resp)
                rows.append(row)
            results["runs"][p] = {"model": model, "turns": rows}
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
    ledger = ws / "proxy-cache-usage.jsonl"
    results["proxy_ledger"] = (
        [json.loads(x) for x in ledger.read_text().splitlines() if x]
        if ledger.is_file()
        else []
    )
    shutil.rmtree(tmp, ignore_errors=True)
    out = json.dumps(results, indent=2)
    if args.out:
        Path(args.out).write_text(out + "\n")
        print(f"results written to {args.out}")
    else:
        print(out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
