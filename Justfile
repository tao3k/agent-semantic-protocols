set shell := ["bash", "-cu"]

repo := "."
rust_harness_project := "languages/rust-lang-project-harness"
typescript_harness_project := "languages/typescript-lang-project-harness"
python_harness_project := "languages/python-lang-project-harness"

default:
    @just --list

# Install all agent tools and Codex hook config. Optional: just agent-hooks-install ~/.local/bin
agent-hooks-install bin_dir="":
    @bin_dir="{{bin_dir}}"; \
      if [ -z "${bin_dir}" ]; then bin_dir="${SEMANTIC_AGENT_BIN_DIR:-/opt/homebrew/bin}"; fi; \
      just agent-tools-install-global "${bin_dir}"; \
      just agent-hooks-install-current "${bin_dir}"; \
      just agent-hooks-doctor "${bin_dir}"

agent-hooks-install-current bin_dir="":
    @just agent-hooks-install-root "{{bin_dir}}"

agent-hooks-doctor bin_dir="":
    @just agent-hooks-doctor-root "{{bin_dir}}"

agent-hooks-install-root bin_dir="":
    @bin_dir="{{bin_dir}}"; \
      if [ -z "${bin_dir}" ]; then bin_dir="${SEMANTIC_AGENT_BIN_DIR:-}"; fi; \
      if [ -n "${bin_dir}" ]; then hook_bin="${bin_dir}/semantic-agent-hook"; else hook_bin="semantic-agent-hook"; fi; \
      "${hook_bin}" install --client codex {{repo}}

agent-hooks-doctor-root bin_dir="":
    @bin_dir="{{bin_dir}}"; \
      if [ -z "${bin_dir}" ]; then bin_dir="${SEMANTIC_AGENT_BIN_DIR:-}"; fi; \
      if [ -n "${bin_dir}" ]; then hook_bin="${bin_dir}/semantic-agent-hook"; else hook_bin="semantic-agent-hook"; fi; \
      "${hook_bin}" doctor --client codex {{repo}}

# Replay the root classifier directly without launching Codex.
agent-hooks-smoke-hook:
    @printf '%s' '{"tool_name":"functions.exec_command","tool_input":{"cmd":"sed -n '\''1,8p'\'' languages/typescript-lang-project-harness/tests/unit/cli.test.ts"}}' \
      | semantic-agent-hook hook --client codex pre-tool --profiles .codex/semantic-agent-hook/profiles.json --emit decision \
      | python3 -c 'import json,sys; d=json.load(sys.stdin); assert d["decision"]=="deny", d; assert d["reasonKind"] in {"bulk-source-dump","direct-source-read"}, d; print("[agent-hooks-smoke-hook] blocked", d["reasonKind"])'

# Launch Codex CLI and verify the real PreToolUse runtime blocks a TS source dump.
agent-hooks-smoke-codex:
    @out="$(mktemp)"; \
      codex_bin="$(command -v codex || true)"; \
      if [ -z "${codex_bin}" ] && [ -x /Applications/Codex.app/Contents/Resources/codex ]; then codex_bin=/Applications/Codex.app/Contents/Resources/codex; fi; \
      if [ -z "${codex_bin}" ]; then echo "codex binary not found on PATH"; rm -f "${out}"; exit 127; fi; \
      "${codex_bin}" exec --json --dangerously-bypass-approvals-and-sandbox --dangerously-bypass-hook-trust -C "$PWD" \
        "Run exactly this shell command and do nothing else: sed -n '1,8p' languages/typescript-lang-project-harness/tests/unit/cli.test.ts" >"${out}" 2>&1 || true; \
      if rg -q "Command blocked by PreToolUse hook: bulk-source-dump denied|permissionDecision.*deny" "${out}"; then \
        echo "[agent-hooks-smoke-codex] blocked"; \
      else \
        cat "${out}"; \
        rm -f "${out}"; \
        exit 1; \
      fi; \
      rm -f "${out}"

# Install semantic-agent-hook, rs-harness, ts-harness, and py-harness.
agent-tools-install-global bin_dir="":
    @bin_dir="{{bin_dir}}"; \
      if [ -z "${bin_dir}" ]; then bin_dir="${SEMANTIC_AGENT_BIN_DIR:-/opt/homebrew/bin}"; fi; \
      just agent-tools-install-hook "${bin_dir}"; \
      just agent-tools-install-rs "${bin_dir}"; \
      just agent-tools-install-ts "${bin_dir}"; \
      just agent-tools-install-py "${bin_dir}"; \
      echo "[agent-tools-install-global] installed semantic-agent-hook, rs-harness, ts-harness, and py-harness into ${bin_dir}"

# Install only the root semantic-agent-hook binary.
agent-tools-install-hook bin_dir="":
    @bin_dir="{{bin_dir}}"; \
      if [ -z "${bin_dir}" ]; then bin_dir="${SEMANTIC_AGENT_BIN_DIR:-/opt/homebrew/bin}"; fi; \
      mkdir -p "${bin_dir}"; \
      cargo build --release --manifest-path Cargo.toml --package semantic-agent-hook --bin semantic-agent-hook; \
      install -m 755 target/release/semantic-agent-hook "${bin_dir}/semantic-agent-hook"; \
      test -x "${bin_dir}/semantic-agent-hook"

# Install only the Rust provider binary.
agent-tools-install-rust bin_dir="":
    @just agent-tools-install-rs "{{bin_dir}}"

agent-tools-install-rs bin_dir="":
    @bin_dir="{{bin_dir}}"; \
      if [ -z "${bin_dir}" ]; then bin_dir="${SEMANTIC_AGENT_BIN_DIR:-/opt/homebrew/bin}"; fi; \
      mkdir -p "${bin_dir}"; \
      cargo build --release --manifest-path {{rust_harness_project}}/Cargo.toml --features cli --bin rs-harness; \
      install -m 755 {{rust_harness_project}}/target/release/rs-harness "${bin_dir}/rs-harness"; \
      "${bin_dir}/rs-harness" --help >/dev/null

# Install only the TypeScript provider binary.
agent-tools-install-typescript bin_dir="":
    @just agent-tools-install-ts "{{bin_dir}}"

agent-tools-install-ts bin_dir="":
    @bin_dir="{{bin_dir}}"; \
      if [ -z "${bin_dir}" ]; then bin_dir="${SEMANTIC_AGENT_BIN_DIR:-/opt/homebrew/bin}"; fi; \
      mkdir -p "${bin_dir}"; \
      npm --prefix {{typescript_harness_project}} run -s build >/dev/null; \
      rm -f "${bin_dir}/ts-harness"; \
      printf '#!/usr/bin/env bash\nexec node "%s/%s/dist/src/cli/main.js" "$@"\n' "$PWD" "{{typescript_harness_project}}" > "${bin_dir}/ts-harness"; \
      chmod 755 "${bin_dir}/ts-harness"; \
      "${bin_dir}/ts-harness" --help >/dev/null

# Install only the Python provider binary.
agent-tools-install-python bin_dir="":
    @just agent-tools-install-py "{{bin_dir}}"

agent-tools-install-py bin_dir="":
    @bin_dir="{{bin_dir}}"; \
      if [ -z "${bin_dir}" ]; then bin_dir="${SEMANTIC_AGENT_BIN_DIR:-/opt/homebrew/bin}"; fi; \
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
