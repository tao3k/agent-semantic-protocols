#!/usr/bin/env bash
# Consume a qualified producer artifact for the committed built-in Orgize source.
set -euo pipefail
revision=$(git -C languages/orgize rev-parse HEAD)
case "$revision" in
  *[!0-9a-f]*|'') echo 'Orgize requires an immutable source SHA' >&2; exit 1 ;;
esac
[[ ${#revision} -eq 40 ]] || exit 1
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) platform=Linux-X64 ;;
  Darwin-arm64) platform=macOS-ARM64 ;;
  *) echo 'No qualified Orgize FFI artifact for this platform' >&2; exit 1 ;;
esac
artifact="orgize-ffi-$revision-$platform"
run=$(gh api "repos/tao3k/orgize/actions/runs?head_sha=$revision&status=success&per_page=100" \
  --jq '.workflow_runs | map(select(.name == "CI" and .conclusion == "success")) | sort_by(.id) | reverse | .[0].id // empty')
if [[ -z "$run" ]]; then
  echo "Orgize $revision has no successful producer CI run; ASP will not rebuild its Scheme parser" >&2
  exit 1
fi
output="$PWD/.data/orgize-ffi/$revision/$platform"
mkdir -p "$output"
gh run download "$run" --repo tao3k/orgize --name "$artifact" --dir "$output"
[[ -f "$output/bundle.json" ]] || { echo 'Missing producer FFI manifest' >&2; exit 1; }
# Orgize build-support performs target, source, runtime, header and archive admission.
printf 'ORGIZE_FFI_BUNDLE=%s\n' "$output" >> "$GITHUB_ENV"
