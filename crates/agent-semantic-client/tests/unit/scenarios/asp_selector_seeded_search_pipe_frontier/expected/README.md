# Expected Frontier

The scenario must render:

- `source=selector`
- `ranker=selector-seed`
- `selectorSeed=rust://...provider_invocation_with_profile`
- `ownerSeed=crates/agent-semantic-client/src/command/provider_process.rs`
- `symbolSeed=provider_invocation_with_profile`
- `actionFrontier=A1.query-code,A2.owner-items`
- `recommendedNext=A1.query-code`
- one `nextCommand=asp rust query --selector ... --workspace . --projection source`

The output must not contain `&&`, and the configured provider marker must not be
needed by the scenario renderer.

The scenario performance gate must also record:

- `providerProcessCount=0`
- `nativeFinderProcessCount=0`
- `renderDuration`
- `stdoutBytes`
