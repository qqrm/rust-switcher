set dotenv-load := true
set shell := ["pwsh", "-NoLogo", "-NoProfile", "-Command"]

default: help

help:
	@just --list

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

test:
	cargo test --workspace --all-features --all-targets --locked

check: fmt clippy test

# bump VERSION?
# - If VERSION omitted: bump patch automatically
# - If VERSION provided: set exact semver
bump VERSION='':
	@pwsh -NoLogo -NoProfile -File scripts\release\bump.ps1 {{VERSION}}

# github-release VERSION?
# Build and upload Windows artifacts to a GitHub Release for vX.Y.Z.
github-release VERSION='':
	@pwsh -NoLogo -NoProfile -File scripts\release\github_release.ps1 {{VERSION}}

# crates-release
# Publish crates to crates.io in dependency order: core -> app
crates-release:
	@pwsh -NoLogo -NoProfile -File scripts\release\cargo_publish.ps1

# release VERSION?
# One-command production release from dev:
# checks -> bump -> push -> immutable tag -> GH Release + assets -> crates.io publish
release VERSION='':
	@pwsh -NoLogo -NoProfile -File scripts\release\release.ps1 {{VERSION}}
