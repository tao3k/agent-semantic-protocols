#!/usr/bin/env bash
set -euo pipefail

readonly TURSO_VERSION="0.7.0"
readonly RELEASE_BASE="https://github.com/tursodatabase/turso/releases/download/v${TURSO_VERSION}"

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64)
    asset="turso_cli-aarch64-apple-darwin.tar.xz"
    ;;
  Darwin-x86_64)
    asset="turso_cli-x86_64-apple-darwin.tar.xz"
    ;;
  Linux-aarch64|Linux-arm64)
    asset="turso_cli-aarch64-unknown-linux-gnu.tar.xz"
    ;;
  Linux-x86_64)
    asset="turso_cli-x86_64-unknown-linux-gnu.tar.xz"
    ;;
  *)
    echo "unsupported platform for pinned Turso sync fixture: $(uname -s)-$(uname -m)" >&2
    exit 2
    ;;
esac

fixture_root="$(mktemp -d "${TMPDIR:-/tmp}/asp-turso-sync-v0.7.XXXXXX")"
trap 'rm -rf "$fixture_root"' EXIT

curl --fail --location --silent --show-error \
  "${RELEASE_BASE}/${asset}" \
  --output "${fixture_root}/${asset}"
curl --fail --location --silent --show-error \
  "${RELEASE_BASE}/${asset}.sha256" \
  --output "${fixture_root}/${asset}.sha256"
(
  cd "$fixture_root"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -c "${asset}.sha256"
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "${asset}.sha256"
  else
    echo "neither shasum nor sha256sum is available" >&2
    exit 2
  fi
  tar -xJf "$asset"
)

readonly archive_root="${asset%.tar.xz}"
readonly server_bin="${fixture_root}/${archive_root}/tursodb"
test "$("$server_bin" --version)" = "Turso ${TURSO_VERSION}"

TURSO_SYNC_SERVER_BIN="$server_bin" \
  direnv exec . cargo test -p agent-semantic-client-db \
    pinned_v0_7_sync_server_push_pull_checkpoint_and_stats \
    -- --ignored --nocapture

TURSO_SYNC_SERVER_BIN="$server_bin" \
  direnv exec . cargo bench -p agent-semantic-client-db \
    --bench turso_sync_server_e2e
