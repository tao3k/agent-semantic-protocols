# Search Owner SourceIndex Missing DB Inputs

Fixture query:

```text
asp rust search owner crates/agent-semantic-hook/build.rs items --workspace . --view seeds
```

The scenario models the `ClientDbSourceIndexLookupState::MissingDb` result from
the DB Engine lookup adapter without opening a DB file.
