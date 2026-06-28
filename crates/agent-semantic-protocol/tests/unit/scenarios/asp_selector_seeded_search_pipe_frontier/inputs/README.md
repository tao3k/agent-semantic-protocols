# Selector-Seeded Search Pipe Input

The fixed real case is:

```text
asp rust search pipe --selector rust://crates/agent-semantic-protocol/src/command/provider_process.rs#item/fn/provider_invocation_with_profile --query "runtime_profile_invocation RuntimeProfiles provider_command_prefix" --workspace . --view seeds
```

The selector and terms are already specific enough for the wrapper to render the
next query action without broad candidate collection.
