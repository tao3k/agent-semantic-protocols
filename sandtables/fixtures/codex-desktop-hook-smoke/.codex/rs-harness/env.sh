# Source from the fixture root to route raw shell/RTK reads through rs-harness.
rs_harness_guard_bin="$(pwd)/.codex/rs-harness/bin"
case ":${PATH:-}:" in
  *":$rs_harness_guard_bin:"*) ;;
  *) export PATH="$rs_harness_guard_bin:${PATH:-}" ;;
esac
