#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

set -euo pipefail

if (( $# == 0 )); then
  echo "usage: just devenv <command> [args...]" >&2
  exit 2
fi

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
repo_root="${ASP_DEVENV_EXEC_ROOT:-$(cd -- "${script_dir}/.." && pwd -P)}"
if [[ ! -d "${repo_root}" ]]; then
  echo "[devenv-profile-exec] repository root does not exist: ${repo_root}" >&2
  exit 2
fi
repo_root="$(cd -- "${repo_root}" && pwd -P)"

if [[ -n "${DEVENV_PROFILE:-}" && -n "${DEVENV_ROOT:-}" &&
      "${ASP_DEVENV_PROFILE_NATIVE_READY:-}" == "${DEVENV_PROFILE}" ]]; then
  active_root="$(cd -- "${DEVENV_ROOT}" 2>/dev/null && pwd -P || true)"
  if [[ "${active_root}" == "${repo_root}" && -d "${DEVENV_PROFILE}/bin" ]]; then
    case ":${PATH}:" in
      *":${DEVENV_PROFILE}/bin:"*) ;;
      *) export PATH="${DEVENV_PROFILE}/bin:${PATH}" ;;
    esac
    cd -- "${repo_root}"
    exec "$@"
  fi
fi

state_root="${ASP_DEVENV_EXEC_STATE:-${repo_root}/.devenv/state}"
cache_dir="${state_root}/asp-profile-exec"
cache_path="${cache_dir}/profile.v1"
refresh_lock="${cache_dir}/refresh.lock"
build_top="${cache_dir}/build-top"

hash_stream() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum | awk '{print "sha256:" $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 | awk '{print "sha256:" $1}'
  else
    openssl dgst -sha256 | awk '{print "sha256:" $NF}'
  fi
}

hash_input_path() {
  local input_path="$1"
  printf 'path:%s\0' "${input_path}"
  if [[ -f "${input_path}" ]]; then
    printf 'file\0'
    command cat -- "${input_path}"
    printf '\0'
  elif [[ -d "${input_path}" ]]; then
    printf 'directory\0'
  else
    printf 'missing\0'
  fi
}

input_digest() {
  local dotfile_path="${1:-}" relative_path input_path discovered
  local -a static_inputs=(
    ".envrc"
    "devenv.nix"
    "devenv.lock"
    "devenv.yaml"
    "devenv.local.nix"
    "devenv.local.yaml"
    "flake.nix"
    "flake.lock"
  )
  {
    for relative_path in "${static_inputs[@]}"; do
      hash_input_path "${repo_root}/${relative_path}"
    done
    if [[ -n "${dotfile_path}" && -f "${dotfile_path}/input-paths.txt" ]]; then
      hash_input_path "${dotfile_path}/input-paths.txt"
      while IFS= read -r discovered; do
        [[ -n "${discovered}" ]] || continue
        hash_input_path "${discovered}"
      done < "${dotfile_path}/input-paths.txt"
    else
      printf 'dynamic-inputs:unavailable\0'
    fi
  } | hash_stream
}

profile_is_usable() {
  local candidate="$1"
  [[ "${candidate}" == /* && -d "${candidate}/bin" ]] || return 1
  if [[ "${ASP_DEVENV_EXEC_ALLOW_NON_STORE_PROFILE:-0}" != "1" ]]; then
    [[ "${candidate}" == /nix/store/* ]] || return 1
    [[ -x "${candidate}/bin/bash" && -f "${candidate}/setup" ]] || return 1
  fi
}

cached_line() {
  local line_number="$1"
  [[ -f "${cache_path}" && ! -L "${cache_path}" ]] || return 1
  sed -n "${line_number}p" "${cache_path}"
}

write_cache() {
  local digest="$1" profile="$2" dotfile_path="$3" temporary
  mkdir -p -- "${cache_dir}"
  chmod 700 -- "${cache_dir}"
  temporary="${cache_path}.tmp.$$"
  (umask 077; printf '%s\n%s\n%s\n' "${digest}" "${profile}" "${dotfile_path}" > "${temporary}")
  mv -f -- "${temporary}" "${cache_path}"
}

refresh_cache() {
  local payload profile dotfile_path digest
  payload="$(
    direnv exec "${repo_root}" /bin/bash --noprofile --norc -c \
      'test -n "${DEVENV_PROFILE:-}"; printf "%s\n%s\n" "$DEVENV_PROFILE" "${DEVENV_DOTFILE:-}"'
  )" || {
    echo "[devenv-profile-exec] direnv did not produce DEVENV_PROFILE" >&2
    return 1
  }
  profile="$(printf '%s\n' "${payload}" | sed -n '1p')"
  dotfile_path="$(printf '%s\n' "${payload}" | sed -n '2p')"
  profile_is_usable "${profile}" || {
    echo "[devenv-profile-exec] rejected unusable DEVENV_PROFILE" >&2
    return 1
  }
  digest="$(input_digest "${dotfile_path}")"
  write_cache "${digest}" "${profile}" "${dotfile_path}"
}

mkdir -p -- "${cache_dir}"
chmod 700 -- "${cache_dir}"
mkdir -p -- "${build_top}"
chmod 700 -- "${build_top}"

cached_profile="$(cached_line 2 || true)"
cached_dotfile="$(cached_line 3 || true)"
cached_digest="$(cached_line 1 || true)"
current_digest="$(input_digest "${cached_dotfile}")"

if [[ "${cached_digest}" != "${current_digest}" ]] || ! profile_is_usable "${cached_profile}"; then
  if mkdir -- "${refresh_lock}" 2>/dev/null; then
    trap 'rmdir -- "${refresh_lock}" 2>/dev/null || true' EXIT
    cached_profile="$(cached_line 2 || true)"
    cached_dotfile="$(cached_line 3 || true)"
    cached_digest="$(cached_line 1 || true)"
    current_digest="$(input_digest "${cached_dotfile}")"
    if [[ "${cached_digest}" != "${current_digest}" ]] || ! profile_is_usable "${cached_profile}"; then
      refresh_cache
    fi
    rmdir -- "${refresh_lock}"
    trap - EXIT
  else
    for _ in {1..1200}; do
      cached_profile="$(cached_line 2 || true)"
      cached_dotfile="$(cached_line 3 || true)"
      cached_digest="$(cached_line 1 || true)"
      current_digest="$(input_digest "${cached_dotfile}")"
      if [[ "${cached_digest}" == "${current_digest}" ]] && profile_is_usable "${cached_profile}"; then
        break
      fi
      sleep 0.05
    done
  fi
fi

profile="$(cached_line 2)"
profile_is_usable "${profile}" || {
  echo "[devenv-profile-exec] no usable cached DEVENV_PROFILE" >&2
  exit 75
}

export DEVENV_PROFILE="${profile}"
export DEVENV_ROOT="${repo_root}"
export DEVENV_STATE="${state_root}"
export ASP_DEVENV_PROFILE_NATIVE_READY="${profile}"
export NIX_BUILD_TOP="${build_top}"

cd -- "${repo_root}"
# Cargo's Darwin toolchain may emit -liconv while the active profile keeps
# libiconv outside the default linker search path.  Export the profile's
# concrete library directory before either execution mode so the normal fast
# path and the full shell path have identical linking semantics.
for iconv_lib in /nix/store/*-libiconv-*/lib; do
  if [[ -f "${iconv_lib}/libiconv.dylib" ]]; then
    export RUSTFLAGS="${RUSTFLAGS-} -L${iconv_lib}"
    break
  fi
done
if [[ "${ASP_DEVENV_EXEC_ALLOW_NON_STORE_PROFILE:-0}" == "1" && "${profile}" != /nix/store/* ]]; then
  export PATH="${profile}/bin:${PATH}"
  exec /bin/bash --noprofile --norc -c 'exec "$@"' asp-devenv "$@"
fi

export NIX_STORE="${profile%/*}"
exec "${profile}/bin/bash" --noprofile --norc -c '
  set -e
  set +u
  export outputs=out
  export out="$DEVENV_PROFILE"
  export buildInputs="$DEVENV_PROFILE"
  source "$DEVENV_PROFILE/setup"
  unset outputs out buildInputs
  unset NIX_ENFORCE_PURITY
  for iconv_lib in /nix/store/*-libiconv-*/lib; do
    if [[ -f "${iconv_lib}/libiconv.dylib" ]]; then
      export RUSTFLAGS="${RUSTFLAGS-} -L${iconv_lib}"
      break
    fi
  done
  export IN_NIX_SHELL=impure
  exec "$@"
' asp-devenv "$@"
