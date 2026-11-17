#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 DoubleGate
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Contributor License Agreement check (CLA.md), run by .github/workflows/cla.yml.
# `cla-check.sh --selftest` checks the decision logic offline.

set -euo pipefail

SIGN_TEXT='I have read the Infinity-Context CLA Document and I hereby sign the CLA'
ALT_SIGN_TEXT='I have read the Infinity-Context CLA Document (CLA.md) and I hereby sign and agree to the CLA'
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
                '$1 != m && $1 != o && $0 != "@unlinked:Lucas Parobek" && $0 != "@unlinked:doublegate" && !($1 ~ /^[0-9]+$/ && $0 ~ /\[bot\]$/)'
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
    printf '%s' "$1" | tr -d '\r`' | sed -e '/^[[:space:]]*$/d' \
        | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' -e '1s/^>[[:space:]]*//' -e 's/\.$//'
}

is_sign_comment() {
    local norm
    norm="$(normalize_comment "$1")"
    [ "$norm" = "$SIGN_TEXT" ] || [ "$norm" = "$ALT_SIGN_TEXT" ]
}

is_sign_body() {
    is_sign_comment "$1" && return 0
    local line
    while IFS= read -r line; do
        [ -z "$line" ] && continue
        if is_sign_comment "$line"; then
            return 0
        fi
    done < <(printf '%s\n' "$1" | tr -d '\r')
    return 1
}

format_author_node() {
    jq -r '
      select(. != null)
      | if .user then "\(.user.databaseId):\(.user.login)"
        else (if ((.name // "") | endswith("[bot]")) then "@bot:" + .name
              else "@unlinked:" + (if (.name // "") != "" then .name else "unknown" end) end)
        end'
}

parse_object_authors() {
    jq -ce '
      .data.repository.object
      | select(. != null)
      | (if (.authors.nodes // null) != null and ((.authors.nodes | length) > 0)
         then .authors.nodes[]
         elif .author != null then .author
         else error("missing commit authors")
         end)' | format_author_node
}

parse_committers() {
    jq -ce '
      .data.repository.pullRequest.commits.nodes[].commit
      | (if (.authors.nodes // null) != null and ((.authors.nodes | length) > 0)
         then .authors.nodes[]
         elif .author != null then .author
         else error("missing commit authors")
         end)' | format_author_node
}

commit_authors_by_oid() {
    local c_oid="$1" raw
    # shellcheck disable=SC2016
    raw="$(gh api graphql --paginate \
        -F owner="${owner:-}" -F name="${name:-}" -F oid="$c_oid" \
        -f query='query($owner: String!, $name: String!, $oid: GitObjectID!, $endCursor: String) {
          repository(owner: $owner, name: $name) {
            object(oid: $oid) {
              ... on Commit {
                author { name user { login databaseId } }
                authors(first: 100, after: $endCursor) {
                  nodes { name user { login databaseId } }
                  pageInfo { hasNextPage endCursor }
                }
              }
            }
          }
        }')" || return 1
    [ -n "$raw" ] || return 1
    printf '%s\n' "$raw" | parse_object_authors || return 1
}

list_committers_range() {
    local b_sha="$1" h_sha="$2" shas c_sha c_out
    shas="$(gh api --paginate "repos/${REPO:-}/compare/${b_sha}...${h_sha}" --jq '.commits[].sha')" || return 1
    while IFS= read -r c_sha; do
        [ -z "$c_sha" ] && continue
        c_out="$(commit_authors_by_oid "$c_sha")" || return 1
        printf '%s\n' "$c_out"
    done <<< "$shas"
}

collect_shared_sha_prs() {
    local h_sha="$1" raw
    raw="$(gh api --paginate "repos/${REPO:-}/commits/$h_sha/pulls")" || return 1
    printf '%s\n' "$raw" | jq -r --arg h "$h_sha" \
        '.[] | select(.state == "open" and .head.sha == $h) | [(.number // ""), (.base.sha // ""), "\(.user.id):\(.user.login)", (.user.type // "User")] | @tsv' || return 1
}

collect_valid_signers() {
    local pr_json="$1" comments_json="$2" committers="$3" opener="$4"
    local uid ulogin cbody cid cbody_b64
    uid="$(printf '%s\n' "$pr_json" | jq -r '.user.id // ""')"
    ulogin="$(printf '%s\n' "$pr_json" | jq -r '.user.login // ""')"
    cid="$(printf '%s\n' "$pr_json" | jq -r '.id // 0')"
    cbody="$(printf '%s\n' "$pr_json" | jq -r '.body // ""')"
    if [ -n "$uid" ] && [[ "$uid" =~ ^[0-9]+$ ]] && is_sign_body "$cbody" && is_party "$uid" "$committers" "$opener"; then
        printf '%s\t%s\t%s\n' "$uid" "$ulogin" "$cid"
    fi
    local cm_rows
    cm_rows="$(printf '%s\n' "$comments_json" | jq -r '.[] | [(.user.id // ""), (.user.login // ""), (.id // 0), ((.body // "") | @base64)] | @tsv')" || return 1
    while IFS=$'\t' read -r uid ulogin cid cbody_b64; do
        [ -z "$uid" ] && continue
        case "$uid" in '' | *[!0-9]*) continue ;; esac
        cbody="$(printf '%s' "$cbody_b64" | base64 -d)"
        if is_sign_body "$cbody" && is_party "$uid" "$committers" "$opener"; then
            printf '%s\t%s\t%s\n' "$uid" "$ulogin" "$cid"
        fi
    done <<< "$cm_rows"
}

LOCK_FILE='signatures/version1/cla-lock.json'
LOCK_TTL_SECS=900
lock_held=no
my_lock_blob_sha=''
status_published=no

release_cla_lock() {
    if [ "$lock_held" = yes ] && [ -n "$my_lock_blob_sha" ]; then
        local unlocked_b64
        unlocked_b64="$(printf '%s\n' '{"owner":"","expires_at":0}' | base64 | tr -d '\n')"
        # Atomic compare-and-swap via Contents API: succeeds ONLY if the current blob SHA equals my_lock_blob_sha.
        gh api -X PUT "repos/${REPO:-}/contents/$LOCK_FILE" \
            -f message="chore(cla): release lock" \
            -f content="$unlocked_b64" \
            -f branch="$BRANCH" \
            -f sha="$my_lock_blob_sha" >/dev/null 2>&1 || true
        lock_held=no
    fi
}

acquire_cla_lock() {
    local attempt max_attempts="${CLA_LOCK_MAX_ATTEMPTS:-180}" sleep_s="${CLA_LOCK_SLEEP_SECS:-2}"
    local out cur_blob_sha cur_doc cur_owner cur_exp now_s new_exp payload_b64 put_out
    for attempt in $(seq 1 "$max_attempts"); do
        now_s="$(date -u +%s)"
        new_exp=$((now_s + LOCK_TTL_SECS))
        payload_b64="$(jq -n --arg o "run-${GITHUB_RUN_ID:-$$}-$attempt" --argjson e "$new_exp" '{owner:$o,expires_at:$e}' | base64 | tr -d '\n')"
        if out="$(gh api "repos/${REPO:-}/contents/$LOCK_FILE?ref=$BRANCH" 2>&1)"; then
            cur_blob_sha="$(printf '%s' "$out" | jq -r .sha)"
            cur_doc="$(printf '%s' "$out" | jq -r .content | base64 -d)"
            cur_owner="$(printf '%s' "$cur_doc" | jq -r '.owner // ""')"
            cur_exp="$(printf '%s' "$cur_doc" | jq -r '.expires_at // 0')"
            if [ -z "$cur_owner" ] || [ "$cur_exp" -le "$now_s" ]; then
                # Atomic compare-and-swap via -f sha="$cur_blob_sha": if another waiter updates first, GitHub returns 409 Conflict.
                if put_out="$(gh api -X PUT "repos/${REPO:-}/contents/$LOCK_FILE" \
                    -f message="chore(cla): acquire lock" \
                    -f content="$payload_b64" \
                    -f branch="$BRANCH" \
                    -f sha="$cur_blob_sha" 2>/dev/null)"; then
                    my_lock_blob_sha="$(printf '%s' "$put_out" | jq -r .content.sha)"
                    lock_held=yes
                    return 0
                fi
            fi
        elif printf '%s' "$out" | grep -q 'Not Found\|HTTP 404'; then
            if put_out="$(gh api -X PUT "repos/${REPO:-}/contents/$LOCK_FILE" \
                -f message="chore(cla): initialize lock" \
                -f content="$payload_b64" \
                -f branch="$BRANCH" 2>/dev/null)"; then
                my_lock_blob_sha="$(printf '%s' "$put_out" | jq -r .content.sha)"
                lock_held=yes
                return 0
            fi
        fi
        [ "$attempt" -lt "$max_attempts" ] && sleep "$sleep_s"
    done
    echo "CLA: could not acquire serialization lock $LOCK_FILE" >&2
    return 1
}

publish_lookup_failure() {
    local h_sha="$1"
    [ -z "$h_sha" ] && return 1
    gh api -X POST "repos/${REPO:-}/statuses/$h_sha" \
        -f state="failure" -f context="CLA" \
        -f description="CLA check error (author/PR lookup failed); comment recheck to retry" \
        -f target_url="$DOC_URL" >/dev/null
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
    is_sign_comment "I have read the Infinity-Context CLA Document (\`CLA.md\`) and I hereby sign and agree to the CLA." && r=yes || r=no
    check "the CLA.md documented statement with backticks and period signs" "$r" "yes"
    local gql_sample='{"data":{"repository":{"pullRequest":{"commits":{"nodes":[{"commit":{"authors":{"nodes":[{"name":"Alice","user":{"login":"alice","databaseId":11}},{"name":"Bob","user":{"login":"bob","databaseId":12}}]}}}]}}}}}'
    check "co-authors from Commit.authors.nodes are extracted" "$(printf '%s' "$gql_sample" | parse_committers)" $'11:alice\n12:bob'
    local pr_sample='{"id":500,"user":{"id":11,"login":"alice"},"body":"## Summary\nAdds feature X.\n\n> I have read the Infinity-Context CLA Document (`CLA.md`) and I hereby sign and agree to the CLA."}'
    local cm_sample='[{"id":601,"user":{"id":12,"login":"bob"},"body":"I have read the Infinity-Context CLA Document and I hereby sign the CLA"},{"id":602,"user":{"id":99,"login":"bystander"},"body":"I have read the Infinity-Context CLA Document and I hereby sign the CLA"}]'
    check "PR body and comments recover valid party signers only" \
        "$(collect_valid_signers "$pr_sample" "$cm_sample" $'11:alice\n12:bob' '11:alice')" \
        $'11\talice\t500\n12\tbob\t601'
    # Test production collector list_committers_range: multi-page Commit.authors pagination, compare failure propagation, and mid-loop author failure propagation
    REPO="doublegate/Infinity-Context" owner="doublegate" name="Infinity-Context"
    gh() {
        if [[ "$*" == *"compare/base_fail...head"* ]]; then return 1; fi
        if [[ "$*" == *"compare/base_ok...head"* ]]; then printf 'sha1\nsha2\n'; return 0; fi
        if [[ "$*" == *"oid=sha1"* ]]; then
            # Two concatenated GraphQL pages for sha1 (page 1 has Alice + hasNextPage:true, page 2 has Bob + hasNextPage:false)
            printf '%s\n%s\n' \
                '{"data":{"repository":{"object":{"authors":{"nodes":[{"name":"Alice","user":{"login":"alice","databaseId":11}}],"pageInfo":{"hasNextPage":true,"endCursor":"c1"}}}}}}' \
                '{"data":{"repository":{"object":{"authors":{"nodes":[{"name":"Bob","user":{"login":"bob","databaseId":12}}],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}}}'
            return 0
        fi
        if [[ "$*" == *"oid=sha2"* ]] && [ "${FAIL_SHA2:-0}" = "1" ]; then return 1; fi
        if [[ "$*" == *"oid=sha2"* ]]; then
            printf '%s\n' '{"data":{"repository":{"object":{"authors":{"nodes":[{"name":"Carol","user":{"login":"carol","databaseId":13}}]}}}}}'
            return 0
        fi
        if [[ "$*" == *"commits/head_fail/pulls"* ]]; then return 1; fi
        if [[ "$*" == *"statuses/head_err"* ]]; then
            printf 'status_published=failure\n'
            return 0
        fi
        return 0
    }
    check "production collector extracts multi-page paginated authors and co-authors across commits" \
        "$(list_committers_range base_ok head)" $'11:alice\n12:bob\n13:carol'
    list_committers_range base_fail head >/dev/null 2>&1 && r=ok || r=failed
    check "compare API failure propagates and fails closed" "$r" "failed"
    FAIL_SHA2=1 list_committers_range base_ok head >/dev/null 2>&1 && r=ok || r=failed
    check "per-commit author lookup failure propagates and fails closed" "$r" "failed"
    collect_shared_sha_prs head_fail >/dev/null 2>&1 && r=ok || r=failed
    check "shared-SHA open PR lookup failure propagates and fails closed" "$r" "failed"
    local published_state=""
    gh() {
        if [[ "$*" == *"statuses/head_err"* ]] && [[ "$*" == *"state=failure"* ]]; then
            published_state="failure"
            return 0
        fi
        return 1
    }
    publish_lookup_failure head_err
    check "lookup failure helper publishes failure status to overwrite stale success" "$published_state" "failure"
    # Test atomic Contents-API CAS lock: overlapping owners, expired takeover, and CAS-fenced release
    local current_lock_sha="blob_owner_a" current_lock_owner="owner_a"
    gh() {
        if [[ "$*" == *"-X PUT"*"cla-lock.json"* ]]; then
            if [[ "$*" == *"sha=$current_lock_sha"* ]]; then
                current_lock_sha="blob_updated"
                printf '{"content":{"sha":"%s"}}\n' "$current_lock_sha"
                return 0
            fi
            return 1 # 409 Conflict when sha does not match
        fi
        if [[ "$*" == *"cla-lock.json?ref="* ]]; then
            local b64
            b64="$(jq -n --arg o "$current_lock_owner" --argjson e "$LOCK_EXP_TEST" '{owner:$o,expires_at:$e}' | base64 | tr -d '\n')"
            printf '{"sha":"%s","content":"%s"}\n' "$current_lock_sha" "$b64"
            return 0
        fi
        return 0
    }
    # 1. If Owner B already replaced Owner A's lock (current_lock_sha="blob_owner_b"), Owner A's release with stale sha="blob_owner_a" is rejected by CAS (409)
    current_lock_sha="blob_owner_b" lock_held=yes my_lock_blob_sha="blob_owner_a"
    release_cla_lock
    check "release_cla_lock CAS precondition prevents overwriting a newer owner's lock" "$current_lock_sha" "blob_owner_b"
    # 2. While Owner A's lease is active (expires_at = now + 900), Owner B cannot evict it
    LOCK_EXP_TEST=$(($(date -u +%s) + 900)) current_lock_sha="blob_owner_a" current_lock_owner="owner_a"
    CLA_LOCK_MAX_ATTEMPTS=2 CLA_LOCK_SLEEP_SECS=0 acquire_cla_lock >/dev/null 2>&1 && r=acquired || r=blocked
    check "acquire_cla_lock does not evict a live lock holder and fails closed on exhaustion" "$r" "blocked"
    # 3. Once Owner A's lease expires (expires_at = 100), Owner B atomically CAS-acquires the lock and gets the new blob SHA
    LOCK_EXP_TEST=100 current_lock_sha="blob_owner_a" current_lock_owner="owner_a"
    CLA_LOCK_MAX_ATTEMPTS=1 CLA_LOCK_SLEEP_SECS=0 acquire_cla_lock >/dev/null 2>&1 && r="$my_lock_blob_sha" || r=blocked
    check "acquire_cla_lock atomically CAS-takes over an expired lock" "$r" "blob_updated"
    unset -f gh
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
if [ "$EVENT" = "issue_comment" ] && ! is_sign_body "${COMMENT_BODY:-}" \
    && ! is_recheck_comment "${COMMENT_BODY:-}"; then
    echo "CLA: comment ignored (neither the signing text nor recheck)"
    exit 0
fi
case "$PR" in '' | *[!0-9]*) echo "PR is not a number" >&2; exit 1 ;; esac
owner="${REPO%%/*}"
name="${REPO#*/}"

on_exit() {
    local rc="$1"
    if [ "$rc" -ne 0 ] && [ "${status_published:-no}" != yes ] && [ -n "${head_sha:-}" ]; then
        publish_lookup_failure "$head_sha" || true
    fi
    if [ "$lock_held" = yes ]; then
        release_cla_lock
    fi
}
trap 'on_exit $?' EXIT

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
        return 1
    fi
}

ensure_branch() {
    local out blob tree commit ref_out
    if out="$(gh api "repos/$REPO/branches/$BRANCH" 2>&1)"; then
        read_signatures
        return 0
    elif ! printf '%s' "$out" | grep -q 'Not Found\|HTTP 404'; then
        echo "cannot read the $BRANCH branch: $out" >&2
        return 1
    fi
    blob="$(gh api -X POST "repos/$REPO/git/blobs" -f content='{"signedContributors":[]}' -f encoding=utf-8 --jq .sha)" || return 1
    tree="$(jq -n --arg p "$FILE" --arg b "$blob" '{tree: [{path: $p, mode: "100644", type: "blob", sha: $b}]}' \
        | gh api -X POST "repos/$REPO/git/trees" --input - --jq .sha)" || return 1
    commit="$(gh api -X POST "repos/$REPO/git/commits" -f message='chore(cla): start the CLA signatures record' -f tree="$tree" --jq .sha)" || return 1
    if ! ref_out="$(gh api -X POST "repos/$REPO/git/refs" -f ref="refs/heads/$BRANCH" -f sha="$commit" 2>&1)"; then
        if ! printf '%s' "$ref_out" | grep -q 'Reference already exists'; then
            echo "cannot create the $BRANCH branch: $ref_out" >&2
            return 1
        fi
    fi
    read_signatures
}

record_signer() {
    local s_uid="$1" s_login="$2" s_cid="$3" s_pr="${4:-$PR}" s_comms="${5:-$committers}" s_opener="${6:-$opener}"
    case "$s_uid" in '' | *[!0-9]*) return 0 ;; esac
    if ! is_party "$s_uid" "$s_comms" "$s_opener"; then
        echo "CLA: @$s_login neither opened pull request #$s_pr nor authored/co-authored a commit in it; nothing recorded"
        return 0
    fi
    read_signatures
    if signed_of "$sig_doc" | grep -Fxq -- "$s_uid"; then
        return 0
    fi
    [ -z "$sig_sha" ] && ensure_branch
    repo_id="$(gh api "repos/$REPO" --jq .id)" || return 1
    for attempt in $(seq 1 15); do
        if signed_of "$sig_doc" | grep -Fxq -- "$s_uid"; then
            break
        fi
        new_doc="$(add_signature "$sig_doc" "$s_login" "$s_uid" "$s_cid" \
            "$s_pr" "$repo_id" "$(date -u +%Y-%m-%dT%H:%M:%SZ)")"
        args=(-X PUT "repos/$REPO/contents/$FILE"
            -f message="CLA: @$s_login signs (pull request #$s_pr)"
            -f content="$(printf '%s\n' "$new_doc" | base64 | tr -d '\n')"
            -f branch="$BRANCH")
        [ -n "$sig_sha" ] && args+=(-f sha="$sig_sha")
        if gh api "${args[@]}" >/dev/null 2>&1; then
            read_signatures
            break
        fi
        [ "$attempt" = 15 ] && { echo "could not record the signature for @$s_login" >&2; return 1; }
        sleep 2
        read_signatures
    done
}

# Resolve head_sha early so on_exit can publish a failure status if pre-lock or lock acquisition fails.
pre_info="$(gh api "repos/$REPO/pulls/$PR")"
head_sha="$(printf '%s' "$pre_info" | jq -r .head.sha)"

# Ensure the cla-signatures branch exists before acquiring the CAS lock file on it.
read_signatures
[ -z "$sig_sha" ] && ensure_branch

# If triggered by a signing comment, persist the signature via CAS on cla.json even before waiting on cla-lock.json
# so a signature is never lost even if lock acquisition later times out.
if [ "$EVENT" = "issue_comment" ] && is_sign_body "${COMMENT_BODY:-}"; then
    : "${COMMENT_USER:?}" "${COMMENT_USER_ID:?}" "${COMMENT_ID:?}"
    pre_base="$(printf '%s' "$pre_info" | jq -r .base.sha)"
    pre_head="$head_sha"
    pre_opener="$(printf '%s' "$pre_info" | jq -r '"\(.user.id):\(.user.login)"')"
    pre_comms="$(list_committers_range "$pre_base" "$pre_head" "$PR")"
    record_signer "$COMMENT_USER_ID" "$COMMENT_USER" "$COMMENT_ID" "$PR" "$pre_comms" "$pre_opener"
fi

if ! acquire_cla_lock; then
    echo "CLA: could not acquire lock for PR #$PR" >&2
    exit 1
fi

evaluate_pr_head() {
    local eval_pr="$1" initial_head_sha="${2:-}" eval_attempt
    local pr_info pr_state base_sha eval_head_sha opener opener_type committers
    local all_pr_comments valid_signers_rows rec_uid rec_login rec_cid
    local shared_prs_rows combined_required o_pr o_base o_opener o_type o_comms o_pr_info o_pr_comments o_signers s_uid s_login s_cid
    local latest_pr_info latest_base_sha latest_head_sha unsigned state desc body list existing target_pr

    status_published=no
    [ -n "$initial_head_sha" ] && head_sha="$initial_head_sha"

    for eval_attempt in 1 2 3; do
        pr_info="$(gh api "repos/$REPO/pulls/$eval_pr")"
        pr_state="$(printf '%s' "$pr_info" | jq -r .state)"
        eval_head_sha="$(printf '%s' "$pr_info" | jq -r .head.sha)"
        head_sha="$eval_head_sha"
        if [ "$pr_state" != "open" ]; then
            shared_prs_rows="$(collect_shared_sha_prs "$eval_head_sha")"
            eval_pr="$(printf '%s\n' "$shared_prs_rows" | awk -F'\t' -v p="$eval_pr" '$1 != "" && $1 != p { print $1; exit }')"
            if [ -z "$eval_pr" ]; then
                echo "CLA: closed PR has no surviving open PRs sharing $eval_head_sha"
                return 0
            fi
            pr_info="$(gh api "repos/$REPO/pulls/$eval_pr")"
        fi
        base_sha="$(printf '%s' "$pr_info" | jq -r .base.sha)"
        eval_head_sha="$(printf '%s' "$pr_info" | jq -r .head.sha)"
        head_sha="$eval_head_sha"
        opener="$(printf '%s' "$pr_info" | jq -r '"\(.user.id):\(.user.login)"')"
        opener_type="$(printf '%s' "$pr_info" | jq -r .user.type)"
        committers="$(list_committers_range "$base_sha" "$eval_head_sha" "$eval_pr")"

        read_signatures

        if [ "$EVENT" = "issue_comment" ] && [ "$eval_pr" = "$PR" ] && is_sign_body "${COMMENT_BODY:-}"; then
            : "${COMMENT_USER:?}" "${COMMENT_USER_ID:?}" "${COMMENT_ID:?}"
            record_signer "$COMMENT_USER_ID" "$COMMENT_USER" "$COMMENT_ID" "$eval_pr" "$committers" "$opener"
        fi

        all_pr_comments="$(gh api --paginate "repos/$REPO/issues/$eval_pr/comments" | jq -s 'add // []')"
        valid_signers_rows="$(collect_valid_signers "$pr_info" "$all_pr_comments" "$committers" "$opener")"
        while IFS=$'\t' read -r rec_uid rec_login rec_cid; do
            [ -z "$rec_uid" ] && continue
            record_signer "$rec_uid" "$rec_login" "$rec_cid" "$eval_pr" "$committers" "$opener"
        done <<< "$valid_signers_rows"

        shared_prs_rows="$(collect_shared_sha_prs "$eval_head_sha")"
        combined_required="$(required_of "$committers" "$opener" "$opener_type")"
        while IFS=$'\t' read -r o_pr o_base o_opener o_type; do
            [ -z "$o_pr" ] || [ "$o_pr" = "$eval_pr" ] && continue
            o_comms="$(list_committers_range "$o_base" "$eval_head_sha" "$o_pr")"
            o_pr_info="$(gh api "repos/$REPO/pulls/$o_pr")"
            o_pr_comments="$(gh api --paginate "repos/$REPO/issues/$o_pr/comments" | jq -s 'add // []')"
            o_signers="$(collect_valid_signers "$o_pr_info" "$o_pr_comments" "$o_comms" "$o_opener")"
            while IFS=$'\t' read -r s_uid s_login s_cid; do
                [ -z "$s_uid" ] && continue
                record_signer "$s_uid" "$s_login" "$s_cid" "$o_pr" "$o_comms" "$o_opener"
            done <<< "$o_signers"
            combined_required+=$'\n'"$(required_of "$o_comms" "$o_opener" "$o_type")"
        done <<< "$shared_prs_rows"

        latest_pr_info="$(gh api "repos/$REPO/pulls/$eval_pr")"
        latest_base_sha="$(printf '%s' "$latest_pr_info" | jq -r .base.sha)"
        latest_head_sha="$(printf '%s' "$latest_pr_info" | jq -r .head.sha)"
        if [ "$latest_base_sha" = "$base_sha" ] && [ "$latest_head_sha" = "$eval_head_sha" ]; then
            break
        fi
        if [ "$eval_attempt" = 3 ]; then
            echo "CLA: PR #$eval_pr base or head kept moving across 3 attempts; failing closed" >&2
            head_sha="$latest_head_sha"
            publish_lookup_failure "$latest_head_sha" || true
            status_published=yes
            return 1
        fi
        sleep 2
    done

    read_signatures
    unsigned="$(unsigned_of "$combined_required" "$(signed_of "$sig_doc")")"

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

    gh api -X POST "repos/$REPO/statuses/$eval_head_sha" \
        -f state="$state" -f context='CLA' -f description="$desc" -f target_url="$DOC_URL" >/dev/null
    status_published=yes

    while IFS= read -r target_pr; do
        [ -z "$target_pr" ] && continue
        existing="$(gh api --paginate "repos/$REPO/issues/$target_pr/comments" \
            --jq '.[] | select(.user.login == "github-actions[bot]" and (.body | contains("<!-- infinity-context-cla-check -->"))) | .id' | sed -n 1p)"
        if [ -n "$existing" ]; then
            gh api -X PATCH "repos/$REPO/issues/comments/$existing" -f body="$body" >/dev/null
        elif [ -n "$unsigned" ]; then
            gh api -X POST "repos/$REPO/issues/$target_pr/comments" -f body="$body" >/dev/null
        fi
    done < <(printf '%s\n%s\n' "$eval_pr" "$(printf '%s\n' "$shared_prs_rows" | awk -F'\t' '$1 != "" { print $1 }')" | awk '!seen[$0]++')

    echo "CLA (PR #$eval_pr @ $eval_head_sha): $state${unsigned:+ (unsigned: $(printf '%s' "$unsigned" | tr '\n' ' '))}"
}

evaluate_pr_head "$PR" "$head_sha"

if [ -n "${BEFORE_SHA:-}" ] && [ "${BEFORE_SHA}" != "0000000000000000000000000000000000000000" ] && [ "${BEFORE_SHA}" != "${head_sha:-}" ]; then
    status_published=no
    head_sha="$BEFORE_SHA"
    old_shared_rows="$(collect_shared_sha_prs "$BEFORE_SHA")"
    surviving_old_pr="$(printf '%s\n' "$old_shared_rows" | awk -F'\t' -v p="$PR" '$1 != "" && $1 != p { print $1; exit }')"
    if [ -n "$surviving_old_pr" ]; then
        evaluate_pr_head "$surviving_old_pr" "$BEFORE_SHA"
    else
        status_published=yes
    fi
fi
