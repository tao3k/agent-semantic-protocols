# Workspace File Before Provider Spawn

The regression command keeps `build-std.ss` as the owner path but accidentally
passes the same file path as `--workspace`:

```sh
asp gerbil-scheme search owner build-std.ss items --query 'builded|pended|optimization|make|clan|building' --workspace build-std.ss --view seeds
```

The fixture includes a fake `gslph` marker provider. The provider must not run.
