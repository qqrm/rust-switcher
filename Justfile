set dotenv-load := true
set shell := ["pwsh", "-NoLogo", "-NoProfile", "-Command"]

# Show all available recipes.
default: help

# List all recipes with descriptions.
help:
	@just --list

# -----------------------------
# Quality gates
# -----------------------------

# Check formatting.
fmt:
	cargo fmt --check

# Run clippy with CI flags.
clippy:
	cargo clippy --all-targets --all-features --locked -- -D warnings

# Run the test suite.
test:
	cargo test --locked

# Run all quality checks.
check: fmt clippy test

# -----------------------------
# Release helpers
# -----------------------------

# Bump Cargo.toml (and Cargo.lock) version and commit.
bump VERSION:
	@pwsh -NoLogo -NoProfile -File scripts\release\bump.ps1 {{VERSION}}

# Create and auto-merge a release PR from dev to main.
publish:
	@pwsh -NoLogo -NoProfile -File scripts\release\publish.ps1
