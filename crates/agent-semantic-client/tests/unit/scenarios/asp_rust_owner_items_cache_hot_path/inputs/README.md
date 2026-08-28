# ASP rust owner-items cache hot path input
The scenario creates `crate/src/lib.rs`, installs a rust-harness owner-items provider script, warms the owner-items cache once, and then runs:
```sh
asp rust search owner crate/src/lib.rs items --query dynamic_owner_item_index --workspace . --view seeds
```
