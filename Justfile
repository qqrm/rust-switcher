set dotenv-load := true
set shell := ["pwsh", "-NoLogo", "-NoProfile", "-Command"]

default:
  @just --list

# -----------------------------
# Quality gates
# -----------------------------

fmt:
  cargo fmt --check

clippy:
  cargo clippy --all-targets --all-features --locked -- -D warnings

test:
  cargo test --locked

check: fmt clippy test

# -----------------------------
# Release helpers
# -----------------------------

bump VERSION:
  $ErrorActionPreference = 'Stop'
  $version = '{{VERSION}}'
  if (-not $version) {
    throw 'VERSION is required (example: just bump 1.2.3)'
  }
  $toml = Get-Content Cargo.toml -Raw
  $updated = [regex]::Replace($toml, '(?m)^version\s*=\s*"[^"]+"', "version = \"$version\"", 1)
  if ($toml -eq $updated) {
    throw 'Failed to update version in Cargo.toml'
  }
  Set-Content -Path Cargo.toml -Value $updated
  if (git ls-files --error-unmatch Cargo.lock) {
    cargo update -p rust-switcher --precise $version
  }
  git add Cargo.toml
  if (git ls-files --error-unmatch Cargo.lock) {
    git add Cargo.lock
  }
  git commit -m "chore: bump version to $version"

publish:
  $ErrorActionPreference = 'Stop'
  $branch = git rev-parse --abbrev-ref HEAD
  if ($branch -ne 'dev') {
    throw "Must be on dev branch (current: $branch)"
  }
  if (git status --porcelain) {
    throw 'Working tree must be clean'
  }
  $version = rg -m 1 '^version\s*=\s*"(?<ver>[^"]+)"' Cargo.toml --replace '$ver'
  if (-not $version) {
    throw 'Unable to read version from Cargo.toml'
  }
  $prUrl = gh pr create --base main --head dev --title "Release v$version" --body "Release v$version" --json url -q '.url'
  gh pr merge $prUrl --auto --squash
