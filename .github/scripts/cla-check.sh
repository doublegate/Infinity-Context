#!/usr/bin/env bash
# Contributor License Agreement check (CLA.md), run by .github/workflows/cla.yml.
# `cla-check.sh --selftest` checks the decision logic offline.

set -euo pipefail

SIGN_TEXT='I have read the Infinity-Context CLA Document and I hereby sign the CLA'
BRANCH='cla-signatures'
FILE='signatures/version1/cla.json'
DOC_URL='https://github.com/doublegate/Infinity-Context/blob/main/CLA.md'
MARKER='<!-- infinity-context-cla-check -->'
MAINTAINER_ID='6858123' # github.com/doublegate

opener_exempt() {
    local id="${1%%:*}" login="${1#*:}"
    [ "$id" = "$MAINTAINER_ID" ] || { [ "$2" = "Bot" ] && [[ "$login" == *"[bot]" ]]; }
}

required_of() {
    local committers="$1" opener="$2" opener_type="$3"
    if opener_exempt "$opener" "$opener_type"; then
        printf '%s\n' "$committers" | sed -e '/^$/d' -e '/^@bot:/d' \
            | awk -F: -v m="$MAINTAINER_ID" -v o="${opener%%:*}" \
                '$1 != m && $1 != o && !($1 ~ /^[0-9]+$/ && $0 ~ /\[bot\]$/)'
    else
        printf '%s\n' "$opener"
        printf '%s\n' "$committers" | sed -e '/^$/d' -e 's/^@bot:/@unlinked:/' \
            | awk -F: -v m="$MAINTAINER_ID" '$1 != m'
    fi
}

unsigned_of() {
    local required="$1" signed="$2"
    printf '%s\n' "$required" | sed '/^$/d' | awk -v signed="$signed" '
        BEGIN { n = split(signed, ids, "\n"); for (i = 1; i <= n; i++) if (ids[i] != "") ok[ids[i]] = 1 }
        /^@unlinked:/ { if (!seen[$0]++) print; next }
        {
            id = $0; sub(/:.*/, "", id); login = $0; sub(/^[^:]*:/, "", login)
            if (!(id in ok) && !seen[id]++) print login
        }'
}

is_party() {
    local id="$1" committers="$2" opener="$3"
    [ "$id" = "${opener%%:*}" ] && return 0
    printf '%s\n' "$committers" | awk -F: -v id="$id" '$1 == id { found = 1 } END { exit !found }'
}

normalize_comment() {
    printf '%s' "$1" | tr -d '\r' | sed -e '/^[[:space:]]*$/d' \
        | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' -e '1s/^>[[:space:]]*//'
}

is_sign_comment() {
    [ "$(normalize_comment "$1")" = "$SIGN_TEXT" ]
}

is_recheck_comment() {
    [ "$(normalize_comment "$1")" = "recheck" ]
}

signed_of() {
    printf '%s' "$1" | jq -r '.signedContributors[]?.id // empty'
}

add_signature() {
    local doc="$1" login="$2" id="$3" comment_id="$4" pr="$5" repo_id="$6" now="$7"
    printf '%s' "$doc" | jq \
        --arg name "$login" --argjson id "$id" --argjson cid "$comment_id" \
        --argjson pr "$pr" --argjson rid "$repo_id" --arg at "$now" \
        '.signedContributors += [{name: $name, id: $id, comment_id: $cid,
          created_at: $at, repoId: $rid, pullRequestNo: $pr}]'
}

selftest() {
    local fails=0
    check() { if [ "$2" = "$3" ]; then echo "  ok    $1"; else echo "  FAIL  $1: got [$2] want [$3]"; fails=$((fails + 1)); fi; }
    req() { unsigned_of "$(required_of "$1" "$2" "$3")" "$4"; }
    local me="$MAINTAINER_ID:doublegate" al="11:alice" bo="12:bob" ma="13:mallory"
    check "an unsigned author is listed" "$(req "$al"$'\n'"$bo" "$me" User 11)" "bob"
    check "the maintainer's own pull request" "$(req "$me"$'\n'"$me" "$me" User '')" ""
    check "a bot's pull request" "$(req '@bot:dependabot[bot]' '49699333:dependabot[bot]' Bot '')" ""
    check "a bot's own linked commits" "$(req '49699333:dependabot[bot]' '49699333:dependabot[bot]' Bot '')" ""
    check "authors are listed once" "$(req "$bo"$'\n'"$bo"$'\n'"$bo" "$me" User '')" "bob"
    check "an unlinked author can never match" \
        "$(req '@unlinked:Bob Smith' "$me" User '')" "@unlinked:Bob Smith"
    check "everyone signed" "$(req "$al"$'\n'"$bo" "$me" User $'12\n11')" ""
    check "an outside opener must sign" "$(req "$me" "$ma" User '')" "mallory"
    check "a forged bot author in an outside pull request" \
        "$(req '@bot:x[bot]' "$ma" User 13)" "@unlinked:x[bot]"
    check "the maintainer's e-mail forged by a signed opener" "$(req "$me" "$ma" User 13)" ""
    check "a user account named like a bot is not exempt" \
        "$(req '@bot:x[bot]' '14:evil[bot]' User '')" $'evil[bot]\n@unlinked:x[bot]'
    check "a bot account without the suffix is not exempt" "$(req '' '15:notabot' Bot '')" "notabot"
    check "a login named like the maintainer is not the maintainer" \
        "$(req '' '16:doublegate' User '')" "doublegate"
    check "a renamed account keeps its signature" "$(req '11:alice-renamed' "$me" User 11)" ""
    check "a reused login does not inherit a signature" "$(req '17:alice' "$me" User 11)" "alice"
    check "a linked bot commit in the maintainer's pull request" \
        "$(req $'49699333:dependabot[bot]\n'"$al" "$me" User 11)" ""
    check "a linked bot commit in an outside pull request still counts" \
        "$(req '49699333:dependabot[bot]' "$ma" User 13)" "dependabot[bot]"
    is_party 13 "$al" "$ma" && r=yes || r=no
    check "the opener may sign" "$r" "yes"
    is_party 11 "$al"$'\n'"$bo" "$ma" && r=yes || r=no
    check "a commit author may sign" "$r" "yes"
    is_party 99 "$al" "$ma" && r=yes || r=no
    check "a bystander's signature is not recorded" "$r" "no"
    is_sign_comment "$SIGN_TEXT" && r=yes || r=no
    check "the signing text signs" "$r" "yes"
    is_sign_comment $'  '"$SIGN_TEXT"$'\r\n' && r=yes || r=no
    check "surrounding whitespace is ignored" "$r" "yes"
    is_sign_comment "$SIGN_TEXT and more" && r=yes || r=no
    check "anything more does not sign" "$r" "no"
    is_sign_comment "recheck" && r=yes || r=no
    check "recheck does not sign" "$r" "no"
    is_sign_comment "> $SIGN_TEXT"$'\n' && r=yes || r=no
    check "a pasted quotation signs" "$r" "yes"
    is_sign_comment "I do not sign. $SIGN_TEXT" && r=yes || r=no
    check "text before the signing text does not sign" "$r" "no"
    is_sign_comment $'\n\n> '"$SIGN_TEXT" && r=yes || r=no
    check "a quotation after blank lines signs" "$r" "yes"
    is_recheck_comment $'recheck\r\n' && r=yes || r=no
    check "recheck with a newline rechecks" "$r" "yes"
    is_recheck_comment "please do not recheck" && r=yes || r=no
    check "a sentence containing recheck does not" "$r" "no"
    local doc='{"signedContributors":[]}'
    doc="$(add_signature "$doc" 'alice' 7 99 48 123 '2026-11-16T00:00:00Z')"
    check "a signature is recorded by account ID" "$(signed_of "$doc")" "7"
    check "its fields" "$(printf '%s' "$doc" | jq -c '.signedContributors[0] | [.id,.comment_id,.pullRequestNo,.repoId]')" "[7,99,48,123]"
    doc="$(add_signature "$doc" 'x"; rm -rf /' 8 100 48 123 'now')"
    check "a hostile login stays data" "$(printf '%s' "$doc" | jq -r '.signedContributors[1].name')" 'x"; rm -rf /'
    if [ "$fails" -eq 0 ]; then echo "all checks passed"; else echo "$fails check(s) failed"; return 1; fi
}

if [ "${1:-}" = "--selftest" ]; then
    selftest
    exit $?
fi

: "${GH_TOKEN:?}" "${REPO:?}" "${PR:?}" "${EVENT:?}"
if [ "$EVENT" = "issue_comment" ] && ! is_sign_comment "${COMMENT_BODY:-}" \
    && ! is_recheck_comment "${COMMENT_BODY:-}"; then
    echo "CLA: comment ignored (neither the signing text nor recheck)"
    exit 0
fi
case "$PR" in '' | *[!0-9]*) echo "PR is not a number" >&2; exit 1 ;; esac
owner="${REPO%%/*}"
name="${REPO#*/}"

list_committers() {
# shellcheck disable=SC2016
gh api graphql --paginate \
    -F owner="$owner" -F name="$name" -F number="$PR" \
    -f query='query($owner: String!, $name: String!, $number: Int!, $endCursor: String) {
      repository(owner: $owner, name: $name) {
        pullRequest(number: $number) {
          commits(first: 100, after: $endCursor) {
            nodes { commit { author { name user { login databaseId } } } }
            pageInfo { hasNextPage endCursor }
          }
        }
      }
    }' \
    --jq '.data.repository.pullRequest.commits.nodes[].commit.author
          | if .user then "\(.user.databaseId):\(.user.login)" else (if ((.name // "") | endswith("[bot]")) then "@bot:" + .name
                            else "@unlinked:" + (if (.name // "") != "" then .name else "unknown" end) end) end'
}

head_of() { gh api "repos/$REPO/pulls/$PR" --jq .head.sha; }
head_sha=''
for attempt in 1 2 3; do
    before="$(head_of)"
    committers="$(list_committers)"
    if [ "$(head_of)" = "$before" ]; then
        head_sha="$before"
        break
    fi
    sleep 5
done
publish=yes
if [ -z "$head_sha" ]; then
    if [ "$EVENT" = "issue_comment" ] && is_sign_comment "${COMMENT_BODY:-}"; then
        publish=no
    else
        echo "CLA: the head kept moving; the run for the latest push checks it"
        exit 0
    fi
fi

pr_info="$(gh api "repos/$REPO/pulls/$PR")"
opener="$(printf '%s' "$pr_info" | jq -r '"\(.user.id):\(.user.login)"')"
opener_type="$(printf '%s' "$pr_info" | jq -r .user.type)"

read_signatures() {
    local out
    if out="$(gh api "repos/$REPO/contents/$FILE?ref=$BRANCH" 2>&1)"; then
        sig_sha="$(printf '%s' "$out" | jq -r .sha)"
        sig_doc="$(printf '%s' "$out" | jq -r .content | base64 -d)"
    elif printf '%s' "$out" | grep -q 'Not Found\|HTTP 404'; then
        sig_sha=''
        sig_doc='{"signedContributors":[]}'
    else
        echo "cannot read the signatures: $out" >&2
        exit 1
    fi
}
read_signatures

ensure_branch() {
    local out blob tree commit
    if out="$(gh api "repos/$REPO/branches/$BRANCH" 2>&1)"; then
        return 0
    elif ! printf '%s' "$out" | grep -q 'Not Found\|HTTP 404'; then
        echo "cannot read the $BRANCH branch: $out" >&2
        exit 1
    fi
    blob="$(gh api -X POST "repos/$REPO/git/blobs" -f content='{"signedContributors":[]}' -f encoding=utf-8 --jq .sha)"
    tree="$(jq -n --arg p "$FILE" --arg b "$blob" '{tree: [{path: $p, mode: "100644", type: "blob", sha: $b}]}' \
        | gh api -X POST "repos/$REPO/git/trees" --input - --jq .sha)"
    commit="$(gh api -X POST "repos/$REPO/git/commits" -f message='chore(cla): start the CLA signatures record' -f tree="$tree" --jq .sha)"
    gh api -X POST "repos/$REPO/git/refs" -f ref="refs/heads/$BRANCH" -f sha="$commit" >/dev/null
    read_signatures
}

if [ "$EVENT" = "issue_comment" ] && is_sign_comment "${COMMENT_BODY:-}"; then
    : "${COMMENT_USER:?}" "${COMMENT_USER_ID:?}" "${COMMENT_ID:?}"
    case "$COMMENT_USER_ID" in '' | *[!0-9]*) echo "COMMENT_USER_ID is not a number" >&2; exit 1 ;; esac
    if ! is_party "$COMMENT_USER_ID" "$committers" "$opener"; then
        echo "CLA: @$COMMENT_USER neither opened this pull request nor authored a commit in it; nothing recorded"
        exit 0
    fi
    [ -z "$sig_sha" ] && ensure_branch
    for attempt in 1 2 3; do
        if signed_of "$sig_doc" | grep -Fxq -- "$COMMENT_USER_ID"; then
            break
        fi
        repo_id="$(gh api "repos/$REPO" --jq .id)"
        new_doc="$(add_signature "$sig_doc" "$COMMENT_USER" "$COMMENT_USER_ID" "$COMMENT_ID" \
            "$PR" "$repo_id" "$(date -u +%Y-%m-%dT%H:%M:%SZ)")"
        args=(-X PUT "repos/$REPO/contents/$FILE"
            -f message="CLA: @$COMMENT_USER signs (pull request #$PR)"
            -f content="$(printf '%s\n' "$new_doc" | base64 -w0)"
            -f branch="$BRANCH")
        [ -n "$sig_sha" ] && args+=(-f sha="$sig_sha")
        if gh api "${args[@]}" >/dev/null; then
            sig_doc="$new_doc"
            break
        fi
        [ "$attempt" = 3 ] && { echo "could not record the signature" >&2; exit 1; }
        sleep 2
        read_signatures
    done
fi

if [ "$publish" = no ]; then
    echo "CLA: signature handled; the head kept moving, so the run for the latest push publishes the status"
    exit 0
fi

unsigned="$(unsigned_of "$(required_of "$committers" "$opener" "$opener_type")" "$(signed_of "$sig_doc")")"

if [ -z "$unsigned" ]; then
    state=success
    desc='Every author has signed the CLA'
    body="$MARKER"$'\n'"All authors of this pull request have signed the [Infinity-Context Contributor License Agreement]($DOC_URL). Thank you."
else
    state=failure
    desc='Waiting for CLA signatures'
    # shellcheck disable=SC2016
    list="$(printf '%s\n' "$unsigned" | sed -e 's/^@unlinked:\(.*\)/- \1 (a commit author with no GitHub account linked: add the commit e-mail address to a GitHub account, then comment `recheck`)/' -e '/^- /!s/^/- @/')"
    body="$MARKER"$'\n'"Thank you for your contribution. Before it can be merged, each author below needs to read the [Infinity-Context Contributor License Agreement]($DOC_URL) and sign it by posting this comment:"$'\n\n'"> $SIGN_TEXT"$'\n\n'"You sign once; it covers your later contributions. The CLA keeps your copyright with you and lets the project offer its code under both the GPL/AGPL and commercial licenses (CLA.md)."$'\n\n'"Not signed yet:"$'\n'"$list"
fi

gh api -X POST "repos/$REPO/statuses/$head_sha" \
    -f state="$state" -f context='CLA' -f description="$desc" -f target_url="$DOC_URL" >/dev/null

existing="$(gh api --paginate "repos/$REPO/issues/$PR/comments" \
    --jq '.[] | select(.user.login == "github-actions[bot]" and (.body | contains("<!-- infinity-context-cla-check -->"))) | .id' | sed -n 1p)"
if [ -n "$existing" ]; then
    gh api -X PATCH "repos/$REPO/issues/comments/$existing" -f body="$body" >/dev/null
elif [ -n "$unsigned" ]; then
    gh api -X POST "repos/$REPO/issues/$PR/comments" -f body="$body" >/dev/null
fi

echo "CLA: $state${unsigned:+ (unsigned: $(printf '%s' "$unsigned" | tr '\n' ' '))}"
