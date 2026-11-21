#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 DoubleGate
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Assembles the asset set for a GitHub release (used by .github/workflows/release.yml).
#
# Input:  dist/ holds this run's archives (*.tar.gz) and THIRD-PARTY-NOTICES.txt; $TAG names the
#         release; `gh` is authenticated for the repository.
# Output: final/ holds the complete set as it will exist on the release, with SHA256SUMS;
#         upload/ holds only what must be uploaded. Writes exists, replace_sums and new_tarballs
#         to $GITHUB_OUTPUT.
#
# Files already attached are never replaced: they are downloaded and hashed as attached, because
# a rebuild is not byte-identical. SHA256SUMS is regenerated over the full set; an attached
# SHA256SUMS may be replaced only when every one of its lines survives unchanged.
set -euo pipefail

: "${TAG:?TAG must be set}"
out="${GITHUB_OUTPUT:-/dev/stdout}"
mkdir -p final upload attached

existing=""
# `gh release view` also finds draft releases (the tag-lookup API does not). Only its
# "release not found" answer means the release is absent; any other failure stops the run.
if existing="$(gh release view "$TAG" --json assets --jq '.assets[].name' 2>attached/view.err)"; then
  echo "exists=true" >>"$out"
elif grep -qi 'release not found' attached/view.err; then
  existing=""
  echo "exists=false" >>"$out"
else
  cat attached/view.err >&2
  echo "ERROR: could not look up release $TAG" >&2
  exit 1
fi

is_attached() { grep -Fxq -- "$1" <<<"$existing"; }

shopt -s nullglob
for f in dist/*.tar.gz dist/THIRD-PARTY-NOTICES.txt; do
  n="$(basename "$f")"
  if is_attached "$n"; then
    gh release download "$TAG" --pattern "$n" --dir final
    echo "already attached, kept as is: $n"
  else
    cp "$f" final/
    cp "$f" upload/
  fi
done
# Archives attached earlier but not built in this run stay covered by SHA256SUMS.
while IFS= read -r n; do
  case "$n" in
    *.tar.gz)
      if [ ! -e "final/$n" ]; then
        gh release download "$TAG" --pattern "$n" --dir final
        echo "attached, not rebuilt this run: $n"
      fi
      ;;
  esac
done <<<"$existing"

(
  cd final
  sums=(*.tar.gz)
  [ -e THIRD-PARTY-NOTICES.txt ] && sums+=(THIRD-PARTY-NOTICES.txt)
  sha256sum -- "${sums[@]}" | sort -k2 >SHA256SUMS
  cat SHA256SUMS
)

replace=false
if is_attached SHA256SUMS; then
  gh release download "$TAG" --pattern SHA256SUMS --dir attached
  while IFS= read -r line; do
    [ -n "$line" ] || continue
    if ! grep -Fxq -- "$line" final/SHA256SUMS; then
      echo "ERROR: attached SHA256SUMS line would change or disappear: $line" >&2
      exit 1
    fi
  done <attached/SHA256SUMS
  if cmp -s attached/SHA256SUMS final/SHA256SUMS; then
    echo "attached SHA256SUMS already matches the asset set"
  else
    echo "attached SHA256SUMS is extended with the new files"
    cp final/SHA256SUMS upload/
    replace=true
  fi
else
  cp final/SHA256SUMS upload/
fi
echo "replace_sums=$replace" >>"$out"

uploads=(upload/*.tar.gz)
if [ "${#uploads[@]}" -gt 0 ]; then
  echo "new_tarballs=true" >>"$out"
else
  echo "new_tarballs=false" >>"$out"
fi
ls -l upload
