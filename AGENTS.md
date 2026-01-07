# Agent instructions

## Required checks
Before finalizing changes, run the same checks as CI (Windows):

- `cargo fmt --all --check`
- `cargo clippy --target x86_64-pc-windows-msvc --all-targets -- -D warnings`
- `cargo test --target x86_64-pc-windows-msvc -- --nocapture`
- `cargo build --release --target x86_64-pc-windows-msvc`
