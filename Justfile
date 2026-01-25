set dotenv-load := true

# Use PowerShell for consistent behavior on Windows.
set shell := ["pwsh", "-NoLogo", "-NoProfile", "-Command"]

default:
  @just --list

# -----------------------------
# Quality gates
# -----------------------------

fmt:
  cargo fmt --all -- --check

clippy:
  cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

test:
  cargo test --workspace --all-features --all-targets --locked

check: fmt clippy test

# -----------------------------
# Version bump / release helpers
# -----------------------------

# Dry-run shows the next version without changing files.
next bump="patch":
  pwsh -File ./scripts/release.ps1 -Bump {{bump}} -DryRun

# Bump version, run checks, commit, push dev, and open PR dev -> main.
release bump="patch":
  pwsh -File ./scripts/release.ps1 -Bump {{bump}} -Branch dev -Main main -Remote origin -Package rust-switcher -RunChecks -Commit -Push -CreatePr

release-patch:
  just release bump=patch

release-minor:
  just release bump=minor

release-major:
  just release bump=major

# Same as `release`, but also tries to merge the PR and create/push a tag on main.
# This only works if your repo/branch protection rules allow it.
release-full bump="patch":
  pwsh -File ./scripts/release.ps1 -Bump {{bump}} -Branch dev -Main main -Remote origin -Package rust-switcher -RunChecks -Commit -Push -CreatePr -MergePr -Tag -TagOnMain
