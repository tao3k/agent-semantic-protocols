# Inputs

The Rust libtest fixture creates a temporary State Core home, a temporary Rust
project root, and eight child processes. Each child writes unique cache
generation rows and immediately reads them back through the Turso DB Engine
facade.
