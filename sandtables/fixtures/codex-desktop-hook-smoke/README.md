# Codex Desktop Hook Smoke Fixture

This fixture is intentionally tiny so a failed Codex Desktop hook smoke cannot
leak real Rust source.  Its `.codex/config.toml` denies any PreToolUse call.

Manual Desktop smoke:

1. Run `functions.exec_command` in this directory with `rtk read src/lib.rs`.
2. Run `multi_tool_use.parallel` with a nested `functions.exec_command` using
   the same command.
3. If the hook is active, the tool output should contain
   `[codex-desktop-smoke] denied`.  If it prints `smoke_marker`, Desktop did
   not dispatch the project hook.

The automated sandtable scenario replays the Codex Desktop payload shapes
directly through `rs-harness agent hook`, then runs the fixture-local guard
shims to verify `rtk`/shell reads deny Rust source even without hook dispatch.
