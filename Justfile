set shell := ["bash", "-cu"]

repo := "."
rust_harness_project := "languages/rust-lang-project-harness"
typescript_harness_project := "languages/typescript-lang-project-harness"
python_harness_project := "languages/python-lang-project-harness"

default:
    @just --list

# Build/install all language provider binaries globally, then install the root hook
# config into {{repo}}. Override the global bin path with SEMANTIC_AGENT_BIN_DIR.
agent-hooks-install: agent-tools-install-global agent-hooks-install-current agent-hooks-doctor

agent-hooks-install-current: agent-hooks-install-root

agent-hooks-doctor: agent-hooks-doctor-root

agent-hooks-install-root:
    if command -v semantic-agent-hook >/dev/null 2>&1; then \
      semantic-agent-hook install --client codex {{repo}}; \
    else \
      cargo run --manifest-path Cargo.toml --quiet --package semantic-agent-hook -- install --client codex {{repo}}; \
    fi

agent-hooks-doctor-root:
    if command -v semantic-agent-hook >/dev/null 2>&1; then \
      semantic-agent-hook doctor --client codex {{repo}}; \
    else \
      cargo run --manifest-path Cargo.toml --quiet --package semantic-agent-hook -- doctor --client codex {{repo}}; \
    fi

agent-tools-install-global: agent-tools-install-hook agent-tools-install-rs agent-tools-install-ts agent-tools-install-py
    @bin_dir="${SEMANTIC_AGENT_BIN_DIR:-/opt/homebrew/bin}"; \
      echo "[agent-tools-install-global] installed semantic-agent-hook, rs-harness, ts-harness, and py-harness into ${bin_dir}"

agent-tools-install-hook:
    bin_dir="${SEMANTIC_AGENT_BIN_DIR:-/opt/homebrew/bin}"; \
      mkdir -p "${bin_dir}"; \
      cargo build --release --manifest-path Cargo.toml --package semantic-agent-hook --bin semantic-agent-hook; \
      install -m 755 target/release/semantic-agent-hook "${bin_dir}/semantic-agent-hook"; \
      test -x "${bin_dir}/semantic-agent-hook"

agent-tools-install-rs:
    bin_dir="${SEMANTIC_AGENT_BIN_DIR:-/opt/homebrew/bin}"; \
      mkdir -p "${bin_dir}"; \
      cargo build --release --manifest-path {{rust_harness_project}}/Cargo.toml --features cli --bin rs-harness; \
      install -m 755 {{rust_harness_project}}/target/release/rs-harness "${bin_dir}/rs-harness"; \
      "${bin_dir}/rs-harness" --help >/dev/null

agent-tools-install-ts:
    bin_dir="${SEMANTIC_AGENT_BIN_DIR:-/opt/homebrew/bin}"; \
      mkdir -p "${bin_dir}"; \
      npm --prefix {{typescript_harness_project}} run -s build >/dev/null; \
      rm -f "${bin_dir}/ts-harness"; \
      printf '#!/usr/bin/env bash\nexec node "%s/%s/dist/src/cli/main.js" "$@"\n' "$PWD" "{{typescript_harness_project}}" > "${bin_dir}/ts-harness"; \
      chmod 755 "${bin_dir}/ts-harness"; \
      "${bin_dir}/ts-harness" --help >/dev/null

agent-tools-install-py:
    bin_dir="${SEMANTIC_AGENT_BIN_DIR:-/opt/homebrew/bin}"; \
      mkdir -p "${bin_dir}"; \
      uv tool install --force --editable {{python_harness_project}}; \
      py_bin="$(uv tool dir --bin)/py-harness"; \
      ln -sfn "${py_bin}" "${bin_dir}/py-harness"; \
      "${bin_dir}/py-harness" --help >/dev/null

agent-hooks-doctor-providers: agent-hooks-doctor-rs agent-hooks-doctor-ts agent-hooks-doctor-py

agent-hooks-doctor-rs:
    rs-harness agent doctor {{repo}}

agent-hooks-doctor-ts:
    ts-harness agent doctor {{repo}}

agent-hooks-doctor-py:
    py-harness agent doctor {{repo}}

check-sandtables:
    uv run semantic-sandtable

check-python-policy:
    uv run --project {{python_harness_project}} --frozen py-harness check --full {{repo}}

report-python-policy:
    uv run --project {{python_harness_project}} --frozen py-harness check --full {{repo}} || true
