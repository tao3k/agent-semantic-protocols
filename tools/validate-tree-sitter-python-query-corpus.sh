#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
grammar_root="$repo_root/languages/python-lang-project-harness/tree-sitter/tree-sitter-python"
profile="$grammar_root/grammar-profile.json"
validator="tools/validate-tree-sitter-python-query-corpus.sh"

test "$(jq -r '.aspWorkspace.queryCorpusValidator' "$profile")" = "$validator"
test "$(jq -r '.queryCorpus.validator' "$profile")" = "$validator"

case_count=0
while IFS=$'\t' read -r catalog capture; do
  query_file="$grammar_root/queries/$catalog.scm"
  test -f "$query_file"
  grep -F "@$capture" "$query_file" >/dev/null
done < <(jq -r '.catalogs[] | .id as $id | .captures[] | [$id, .] | @tsv' "$profile")

for corpus_file in "$grammar_root"/query-corpus/*.txt; do
  test -f "$corpus_file"
  current_catalog=""
  while IFS= read -r line; do
    case "$line" in
      catalog:\ *)
        current_catalog="${line#catalog: }"
        ;;
      capture\ *)
        test -n "$current_catalog"
        capture="${line#capture }"
        capture="${capture%% node=*}"
        jq -e --arg id "$current_catalog" --arg capture "$capture" \
          '.catalogs[] | select(.id == $id) | .captures | index($capture)' \
          "$profile" >/dev/null
        case_count=$((case_count + 1))
        ;;
    esac
  done < "$corpus_file"
done

test "$case_count" -gt 0
printf 'tree-sitter Python query corpus is valid\n'
