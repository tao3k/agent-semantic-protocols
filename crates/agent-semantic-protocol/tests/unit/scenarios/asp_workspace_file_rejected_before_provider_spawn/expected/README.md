# Expected

The Rust workspace-root validator rejects the file-valued `--workspace` before
provider resolution or provider spawn with a `--workspace requires a directory
project root` diagnostic and guidance to keep the file path as the
owner/selector while using a directory workspace such as `--workspace .`.

The fake `gslph` marker file is not created.
