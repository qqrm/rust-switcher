set dotenv-load := true
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
# Release helpers
# -----------------------------

# Dry-run: shows next version only.
next bump="patch":
  pwsh -NoLogo -NoProfile -File ./scripts/release.ps1 -Bump {{bump}} -DryRun

# Main entry: bump + checks + commit + push + PR
release bump="patch":
  pwsh -NoLogo -NoProfile -File ./scripts/release.ps1 -Bump {{bump}} -Branch dev -Main main -Remote origin -Package rust-switcher -RunChecks -Commit -Push -CreatePr

# Aliases (no nesting, always explicit bump)
release-patch:
  pwsh -NoLogo -NoProfile -File ./scripts/release.ps1 -Bump patch -Branch dev -Main main -Remote origin -Package rust-switcher -RunChecks -Commit -Push -CreatePr

release-minor:
  pwsh -NoLogo -NoProfile -File ./scripts/release.ps1 -Bump minor -Branch dev -Main main -Remote origin -Package rust-switcher -RunChecks -Commit -Push -CreatePr

release-major:
  pwsh -NoLogo -NoProfile -File ./scripts/release.ps1 -Bump major -Branch dev -Main main -Remote origin -Package rust-switcher -RunChecks -Commit -Push -CreatePr

# Full auto: also tries merge + tag (may fail due to branch protection)
release-full bump="patch":
  pwsh -NoLogo -NoProfile -File ./scripts/release.ps1 -Bump {{bump}} -Branch dev -Main main -Remote origin -Package rust-switcher -RunChecks -Commit -Push -CreatePr -MergePr -Tag -TagOnMain

# Full auto aliases (recommended)
release-full-patch:
  pwsh -NoLogo -NoProfile -File ./scripts/release.ps1 -Bump patch -Branch dev -Main main -Remote origin -Package rust-switcher -RunChecks -Commit -Push -CreatePr -MergePr -Tag -TagOnMain

release-full-minor:
  pwsh -NoLogo -NoProfile -File ./scripts/release.ps1 -Bump minor -Branch dev -Main main -Remote origin -Package rust-switcher -RunChecks -Commit -Push -CreatePr -MergePr -Tag -TagOnMain

release-full-major:
  pwsh -NoLogo -NoProfile -File ./scripts/release.ps1 -Bump major -Branch dev -Main main -Remote origin -Package rust-switcher -RunChecks -Commit -Push -CreatePr -MergePr -Tag -TagOnMain
