#!/usr/bin/env bash
# Live silent-ignore detector for Claude Code output replacement (T-64-001).
#
# Runs one billed `claude -p` session per check in a throwaway project, with only icx's hooks and
# a scratch icx workspace (your user settings, hooks, plugins and icx store are not used), and
# fails if the model saw the raw tool output instead of icx's stub. Run it by hand after a Claude
# Code update; it is not part of CI.
#
# Usage: scripts/live-claude-replacement-check.sh [path/to/icx] [model]
set -euo pipefail

ICX_ARG=${1:-target/release/icx}
MODEL=${2:-haiku}
# Checked before realpath, which itself exits 1 (the "raw output" code) on a missing path.
[ -x "$ICX_ARG" ] || { echo "icx binary not found: $ICX_ARG" >&2; exit 2; }
ICX=$(realpath "$ICX_ARG")
command -v claude >/dev/null || { echo "claude not on PATH" >&2; exit 2; }
command -v jq >/dev/null || { echo "jq not on PATH" >&2; exit 2; }

ROOT=$(mktemp -d "${TMPDIR:-/tmp}/icx-live-check.XXXXXX")
RUNTIME=$(mktemp -d /tmp/icxlc.XXXXXX) # short path: the daemon socket must stay under 104 bytes
trap 'rm -rf "$ROOT" "$RUNTIME"' EXIT
chmod 700 "$RUNTIME"
mkdir -p "$ROOT/proj"
(cd "$ROOT/proj" && git init -q && seq 1 3000 > big.txt)

export ICX_DIR="$ROOT/ws" ICX_STATE_HOME="$ROOT/state" ICX_CONFIG_HOME="$ROOT/config"
export XDG_RUNTIME_DIR="$RUNTIME" ICX_NO_DAEMON=1
"$ICX" init --dir "$ICX_DIR" >/dev/null

# The hook command quotes the icx path for the shell, and jq escapes it for JSON, so a path with
# spaces, quotes or backslashes still works.
HOOK_CMD="$(printf '%q' "$ICX") hook --agent claude-code --event PostToolUse"
settings() { # $1: extra JSON object merged into the settings
    jq -n --arg cmd "$HOOK_CMD" --argjson extra "$1" \
        '{hooks: {PostToolUse: [{matcher: "", hooks: [{type: "command", command: $cmd, timeout: 30}]}]}} + $extra'
}
settings '{}' > "$ROOT/plain.json"
settings '{"env": {"CLAUDE_CODE_EXPERIMENTAL_CODE_MODE": "1"}}' > "$ROOT/codemode.json"

failures=0
errors=0
check() { # $1: label, $2: settings file, $3: allowed tools (comma-separated), $4: prompt
    local reply status=0
    local err="$ROOT/stderr-${1// /_}.txt"
    # `--allowedTools` is variadic: the `=` form keeps it from swallowing the prompt.
    reply=$(cd "$ROOT/proj" && claude -p --model "$MODEL" --setting-sources project \
        --settings "$2" --strict-mcp-config --no-session-persistence "--allowedTools=$3" "$4" \
        2>"$err") || status=$?
    if [ "$status" -ne 0 ]; then
        # Not a replacement result: authentication, a flag or the network failed.
        echo "ERROR $1: claude exited $status"
        sed 's/^/      /' "$err" | tail -5
        errors=$((errors + 1))
    # No pipes on the reply: under pipefail an early-closing reader would SIGPIPE the writer.
    elif [[ "$reply" == *ICX:STUB* ]]; then
        echo "PASS  $1"
    else
        echo "FAIL  $1: the model did not see the stub"
        sed -n '1,2s/^/      /p' <<<"$reply"
        failures=$((failures + 1))
    fi
}

ASK='Then reply with only the first line of the result exactly as you received it.'
check "Bash" "$ROOT/plain.json" Bash "Use the Bash tool to run exactly: seq 1 3000. $ASK"
check "Read" "$ROOT/plain.json" Read "Use the Read tool once to read big.txt. $ASK"
# Only REPL, the Code Mode tool, is allowed: a direct Bash call would pass without exercising the
# script path, so with Code Mode off this check fails rather than passing on Bash.
check "Code Mode script" "$ROOT/codemode.json" REPL \
    "Run the shell command seq 1 3000 once, using whatever tool you normally would. $ASK"

if [ "$errors" -gt 0 ]; then
    echo "$errors check(s) could not run; see the errors above" >&2
    exit 2
fi
if [ "$failures" -gt 0 ]; then
    echo "$failures check(s) failed: Claude Code may be ignoring updatedToolOutput" >&2
    exit 1
fi
version=$(claude --version 2>/dev/null || true)
echo "all checks passed (Claude Code ${version%%$'\n'*})"
