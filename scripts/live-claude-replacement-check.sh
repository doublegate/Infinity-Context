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

ICX=$(realpath "${1:-target/release/icx}")
MODEL=${2:-haiku}
[ -x "$ICX" ] || { echo "icx binary not found: $ICX" >&2; exit 2; }
command -v claude >/dev/null || { echo "claude not on PATH" >&2; exit 2; }

ROOT=$(mktemp -d "${TMPDIR:-/tmp}/icx-live-check.XXXXXX")
RUNTIME=$(mktemp -d /tmp/icxlc.XXXXXX) # short path: the daemon socket must stay under 104 bytes
trap 'rm -rf "$ROOT" "$RUNTIME"' EXIT
chmod 700 "$RUNTIME"
mkdir -p "$ROOT/proj"
(cd "$ROOT/proj" && git init -q && seq 1 3000 > big.txt)

export ICX_DIR="$ROOT/ws" ICX_STATE_HOME="$ROOT/state" ICX_CONFIG_HOME="$ROOT/config"
export XDG_RUNTIME_DIR="$RUNTIME" ICX_NO_DAEMON=1
"$ICX" init --dir "$ICX_DIR" >/dev/null

settings() { # $1: extra JSON merged into the settings object
    printf '{"hooks":{"PostToolUse":[{"matcher":"","hooks":[{"type":"command","command":"%s hook --agent claude-code --event PostToolUse","timeout":30}]}]}%s}\n' "$ICX" "$1"
}
settings '' > "$ROOT/plain.json"
settings ',"env":{"CLAUDE_CODE_EXPERIMENTAL_CODE_MODE":"1"}' > "$ROOT/codemode.json"

failures=0
check() { # $1: label, $2: settings file, $3: allowed tool ("" for any), $4: prompt
    local reply
    local allowed=()
    # `--allowedTools` is variadic: the `=` form keeps it from swallowing the prompt.
    [ -n "$3" ] && allowed=("--allowedTools=$3")
    reply=$(cd "$ROOT/proj" && claude -p --model "$MODEL" --setting-sources project \
        --settings "$2" --strict-mcp-config --no-session-persistence "${allowed[@]}" "$4" 2>/dev/null) || true
    if printf '%s' "$reply" | grep -q 'ICX:STUB'; then
        echo "PASS  $1"
    else
        echo "FAIL  $1: the model did not see the stub"
        printf '      %s\n' "$(printf '%s' "$reply" | head -2)"
        failures=$((failures + 1))
    fi
}

ASK='Then reply with only the first line of the result exactly as you received it.'
check "Bash" "$ROOT/plain.json" Bash "Use the Bash tool to run exactly: seq 1 3000. $ASK"
check "Read" "$ROOT/plain.json" Read "Use the Read tool once to read big.txt. $ASK"
check "Code Mode script" "$ROOT/codemode.json" "" \
    "Run the shell command seq 1 3000 once, using whatever tool you normally would. $ASK"

if [ "$failures" -gt 0 ]; then
    echo "$failures check(s) failed: Claude Code may be ignoring updatedToolOutput" >&2
    exit 1
fi
echo "all checks passed (Claude Code $(claude --version 2>/dev/null | head -1))"
