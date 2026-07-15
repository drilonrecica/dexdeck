#!/usr/bin/env bash

set -euo pipefail

if [[ "$#" -ne 2 ]]; then
  echo "usage: check-dco.sh <base-commit> <head-commit>" >&2
  exit 2
fi

base_commit="$1"
head_commit="$2"
failed=0
commits="$(git rev-list --no-merges "${base_commit}..${head_commit}")"

if [[ -n "$commits" ]]; then
  while IFS= read -r commit; do
    author_name="$(git show -s --format='%an' "$commit")"
    author_email="$(git show -s --format='%ae' "$commit")"
    expected="Signed-off-by: ${author_name} <${author_email}>"

    if ! git show -s --format='%B' "$commit" | grep --fixed-strings --ignore-case --line-regexp --quiet "$expected"; then
      echo "${commit}: missing ${expected}" >&2
      failed=1
    fi
  done <<< "$commits"
fi

exit "$failed"
