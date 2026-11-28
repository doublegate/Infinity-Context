#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 DoubleGate
# SPDX-License-Identifier: GPL-3.0-or-later
#
# redactor-differential.sh -- compare the redactor in this working tree against a base ref.
#
#   scripts/redactor-differential.sh [--cases FILE.json] [--n LINES] [--out DIR] [BASE_REF]
#
# Runs the `redact_dev_harness` fragment fuzz (fixed seed, so both trees see the same lines) in
# this tree and in a temporary detached worktree of BASE_REF (default: main), and reports:
#   new-leaks.txt    lines whose placeholder survives here but not on BASE_REF (look at these)
#   fixed-leaks.txt  lines that leak on BASE_REF but not here
# With --cases, it also runs the case file on both trees and prints every case whose tag differs.
#
# A surviving placeholder is a leak only after a sensitive key, so read new-leaks.txt by hand; it
# is a review aid, not a gate. This tree's harness is copied over the base worktree's, so both
# sides always generate the same lines and apply the same leak test.
#
# Environment:
#   CARGO_WRAP  command prefix for cargo, e.g. "agent-workstation resource --wait 1800 build --"
#   BASE_TARGET_DIR  cargo target directory for the base tree (default: target/redactor-diff-base
#               in this repository, kept between runs so the base is not rebuilt from scratch)
#   CARGO_BUILD_JOBS is passed through as usual.
set -euo pipefail
export LC_ALL=C  # byte order for sort, comm and join

usage() { sed -n '6,13p' "$0" >&2; exit 2; }
cases="" n=200000 out="" base="main" positional=0
while [ $# -gt 0 ]; do
  case "$1" in
    --cases) [ $# -ge 2 ] || usage; cases=$(realpath "$2"); shift 2 ;;
    --n) [ $# -ge 2 ] || usage; n=$2; shift 2 ;;
    --out) [ $# -ge 2 ] || usage; out=$2; shift 2 ;;
    -h|--help) usage ;;
    -*) usage ;;
    *) [ "$positional" -eq 0 ] || usage; base=$1; positional=1; shift ;;
  esac
done
case "$n" in ''|*[!0-9]*) usage ;; esac

repo=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
harness=crates/icx-codec/tests/redact_dev_harness.rs
[ -f "$repo/$harness" ] || { echo "missing $harness in $repo" >&2; exit 1; }
git -C "$repo" rev-parse --verify --quiet "$base^{commit}" > /dev/null \
  || { echo "unknown base ref: $base" >&2; exit 1; }
out=${out:-$(mktemp -d "${TMPDIR:-/tmp}/icx-redactor-diff.XXXXXX")}
mkdir -p "$out"
wt=$(mktemp -d "${TMPDIR:-/tmp}/icx-redactor-base.XXXXXX")
cleanup() {
  git -C "$repo" worktree remove --force "$wt" > /dev/null 2>&1 \
    || { rm -rf "$wt"; git -C "$repo" worktree prune; }
}
trap cleanup EXIT
git -C "$repo" worktree add --detach --quiet "$wt" "$base"
mkdir -p "$(dirname "$wt/$harness")"
cp "$repo/$harness" "$wt/$harness"

read -r -a wrap <<< "${CARGO_WRAP:-}"
run() { # tree label test [env...]
  local tree=$1 label=$2 test=$3; shift 3
  local target=()
  [ "$label" = base ] && target=(CARGO_TARGET_DIR="${BASE_TARGET_DIR:-$repo/target/redactor-diff-base}")
  (cd "$tree" && env ${target[@]+"${target[@]}"} "$@" ${wrap[@]+"${wrap[@]}"} cargo test -q --release -p icx-codec \
     --test redact_dev_harness -- --ignored --nocapture --exact "$test") > "$out/$label.$test.log" 2>&1 \
    || { echo "cargo test failed in $tree; see $out/$label.$test.log" >&2; exit 1; }
}

for side in head base; do
  tree=$repo; [ "$side" = base ] && tree=$wt
  run "$tree" "$side" fuzz FUZZ_N="$n" LEAKS=1
  grep '^L ' "$out/$side.fuzz.log" | sort -u > "$out/$side.leaks.txt" || true
  if [ -n "$cases" ]; then run "$tree" "$side" cases CASES="$cases" ALL=1; fi
done

comm -23 "$out/head.leaks.txt" "$out/base.leaks.txt" > "$out/new-leaks.txt"
comm -13 "$out/head.leaks.txt" "$out/base.leaks.txt" > "$out/fixed-leaks.txt"
echo "base: $base ($(git -C "$repo" rev-parse --short "$base")), fuzz lines: $n, results: $out"
grep -h '^fuzz:' "$out/head.fuzz.log" | sed 's/^/  head  /'
grep -h '^fuzz:' "$out/base.fuzz.log" | sed 's/^/  base  /'
echo "  new leaks:   $(wc -l < "$out/new-leaks.txt")  ($out/new-leaks.txt)"
echo "  fixed leaks: $(wc -l < "$out/fixed-leaks.txt")  ($out/fixed-leaks.txt)"

if [ -n "$cases" ]; then
  tags() { grep -E '^#[0-9]+ ' "$1" | awk '{print $1, $2}' | sort -k1,1; }
  echo "cases whose tag differs (case head base):"
  join <(tags "$out/head.cases.log") <(tags "$out/base.cases.log") | awk '$2 != $3' | sort -t'#' -k2 -n
fi
