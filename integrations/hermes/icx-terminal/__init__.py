# ruff: noqa: N999 - the directory is the Hermes plugin name, `icx-terminal`
"""Infinity-Context (icx) shell-output stubbing for Hermes Agent (T-65-009).

Written against Hermes v0.23.3 (``caafe5389ae``); ``plugin.yaml`` pins ``>=0.23.3,<0.24``.
A general plugin, separate from the ``icx`` context engine because only a general plugin can
register ``transform_terminal_output``. It is off until you list it:

    plugins:
      enabled: [icx-terminal]

Hermes calls the hook for each terminal result and uses the first string a plugin returns in
place of the output, then applies its output limit (``tools/terminal_tool_result.py``). This
plugin sends a result of at least ``ICX_HERMES_STUB_MIN_BYTES`` (default 2048) bytes to
``icx hook --agent hermes --event transform_terminal_output``. When icx stores it, the plugin
returns icx's stub; the stub's ``#icx:`` handle dereferences to the full output through the
``icx`` MCP tool. A failed command (non-zero exit) keeps its output, so the model sees the error.

It fails open: if icx is missing, slow (``ICX_HERMES_TIMEOUT`` seconds, default 10, below
Hermes's own plugin timeout) or errors, the plugin returns None and Hermes keeps the raw output.
``ICX_HERMES_DISABLE=1`` turns it off.
"""

from __future__ import annotations

import json
import logging
import math
import os
import shutil
import subprocess
from typing import Any

logger = logging.getLogger(__name__)

_MAX_PAYLOAD_BYTES = 64 * 1024 * 1024
_STUB_PREFIX = "⟪ICX:STUB│#icx:"


def _is_program(path: str) -> bool:
    return os.path.isfile(path) and os.access(path, os.X_OK)


def _icx_bin() -> str | None:
    explicit = os.environ.get("ICX_BIN")
    if explicit:
        # A bare name is looked up on PATH; a directory is never a program.
        if "/" not in explicit:
            return shutil.which(explicit)
        return explicit if _is_program(explicit) else None
    found = shutil.which("icx")
    if found:
        return found
    fallback = os.path.expanduser("~/.local/bin/icx")
    return fallback if _is_program(fallback) else None


def _env_number(name: str, default: float, floor: float) -> float:
    # A non-finite value (`inf`, `nan`) falls back to the default.
    try:
        value = float(os.environ.get(name, default))
    except ValueError:
        return default
    return max(floor, value) if math.isfinite(value) else default


def _split_simple_commands(command: str) -> list[list[str]]:
    """One pass over `command`, as `crates/icx-adapters/src/shell.rs` reads it: single quotes are
    literal; double quotes allow backslash escapes of `"`, `\\`, `$` and `` ` ``; a backslash
    outside quotes escapes the next character, and before a newline joins the lines; `&&`, `||`,
    `;`, `|`, `&` and a newline outside quotes end a simple command. `#` is an ordinary character
    (handles are `#icx:...`)."""
    segments: list[list[str]] = [[]]
    cur: list[str] = []
    in_word = False
    quote: str | None = None
    i = 0
    n = len(command)

    def end_word() -> None:
        nonlocal in_word
        if in_word:
            segments[-1].append("".join(cur))
            cur.clear()
            in_word = False

    while i < n:
        c = command[i]
        i += 1
        if quote == "'":
            if c == "'":
                quote = None
            else:
                cur.append(c)
        elif quote == '"':
            if c == '"':
                quote = None
            elif c == "\\" and i < n and command[i] in '"\\$`':
                cur.append(command[i])
                i += 1
            elif c == "\\" and command[i : i + 1] == "\n":
                i += 1
            elif c == "\\" and command[i : i + 2] == "\r\n":
                i += 2
            else:
                cur.append(c)
        elif c == "\\":
            if command[i : i + 1] == "\n":
                i += 1
                continue
            if command[i : i + 2] == "\r\n":
                i += 2
                continue
            if i < n:
                cur.append(command[i])
                i += 1
            in_word = True
        elif c in "\"'":
            quote = c
            in_word = True
        elif c in ";|&\n":
            end_word()
            if c in "|&" and i < n and command[i] == c:
                i += 1
            segments.append([])
        elif c.isspace():
            end_word()
        else:
            cur.append(c)
            in_word = True
    end_word()
    return [seg for seg in segments if seg]


def _segments(command: str, depth: int = 0) -> list[list[str]]:
    """The simple commands of `command` (see `_split_simple_commands`); a shell wrapper's `-c`
    script is read as commands too, at most 8 levels deep."""
    expanded: list[list[str]] = []
    for seg in _split_simple_commands(command):
        i = _program_index(seg)
        prog = seg[i].rsplit("/", 1)[-1] if i is not None else ""
        if depth < 8 and prog in ("sh", "bash", "zsh", "dash", "fish"):
            flag = next(
                (
                    k
                    for k in range(i + 1, len(seg))
                    if seg[k].startswith("-")
                    and "c" in seg[k][1:]
                    and not seg[k].startswith("--")
                ),
                None,
            )
            if flag is not None and flag + 1 < len(seg):
                expanded.extend(_segments(seg[flag + 1], depth + 1))
                continue
        expanded.append(seg)
    return expanded


def _program_index(words: list[str]) -> int | None:
    """The program word: past `KEY=value` assignments and `env` with its options."""
    i = 0
    while i < len(words):
        w = words[i]
        name, eq, _ = w.partition("=")
        if eq and name and name.replace("_", "").isalnum():
            i += 1
        elif w == "env" or w.endswith("/env"):
            i += 1
            while i < len(words) and words[i].startswith("-"):
                i += 2 if words[i] in ("-u", "--unset", "-C", "--chdir", "-S") else 1
        else:
            return i
    return None


def _is_own_command(command: str) -> bool:
    """icx's own commands (`icx deref ...`, also inside `cd x && ...`, a pipe or `sh -c`), whose
    output must never be stubbed again (review of #91)."""
    for seg in _segments(command):
        i = _program_index(seg)
        if i is not None and seg[i].rsplit("/", 1)[-1] in ("icx", "icx-hook"):
            return True
    return False


def transform_terminal_output(
    command: Any = "",
    output: Any = "",
    returncode: Any = None,
    task_id: Any = "",
    env_type: Any = "",
    **_: Any,
) -> str | None:
    """Returns icx's stub for a large successful result, or None to keep the output."""
    try:
        if os.environ.get("ICX_HERMES_DISABLE") == "1" or not isinstance(output, str):
            return None
        # Only a known success is stubbed: a failure or an unknown status keeps its output.
        if isinstance(returncode, bool) or returncode != 0:
            return None
        if _is_own_command(str(command)) or output.lstrip().startswith("⟪ICX:"):
            return None
        size = len(output.encode("utf-8", errors="replace"))
        if size < _env_number("ICX_HERMES_STUB_MIN_BYTES", 2048, 0):
            return None
        binary = _icx_bin()
        if binary is None:
            return None
        payload = json.dumps(
            {
                "command": str(command),
                "output": output,
                "returncode": returncode,
                "task_id": str(task_id),
                "env_type": str(env_type),
            },
            ensure_ascii=False,
            # Strict: output icx cannot store byte for byte (a lone surrogate) is not stubbed,
            # so the raw output stays and nothing lossy is stored (review of #92).
        ).encode("utf-8")
        if len(payload) > _MAX_PAYLOAD_BYTES:
            return None
        env = dict(os.environ)
        env.setdefault("ICX_DAEMON_AUTOSTART", "1")
        proc = subprocess.run(
            [
                binary,
                "hook",
                "--agent",
                "hermes",
                "--event",
                "transform_terminal_output",
            ],
            input=payload,
            capture_output=True,
            timeout=_env_number("ICX_HERMES_TIMEOUT", 10, 1),
            env=env,
            check=False,
        )
        if proc.returncode != 0:
            return None
        reply = json.loads(proc.stdout or b"{}")
        stub = reply.get("output") if isinstance(reply, dict) else None
        # Only a well-formed, smaller stub replaces the output.
        if (
            isinstance(stub, str)
            and stub.startswith(_STUB_PREFIX)
            and len(stub) < len(output)
        ):
            return stub
        return None
    except Exception as exc:  # noqa: BLE001 - never break a terminal call over icx
        logger.debug("icx transform_terminal_output skipped: %s", exc)
        return None


def register(ctx: Any) -> None:
    ctx.register_hook("transform_terminal_output", transform_terminal_output)
