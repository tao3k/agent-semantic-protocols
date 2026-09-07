# Expected

ASP resolves the dependency package with Cargo metadata, emits `sourceRoot`, and returns an `external-api` selector or explicit API miss without requiring manual `.cargo/git/checkouts` discovery.
