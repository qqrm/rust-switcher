param(
  [Parameter(Mandatory = $true)]
  [ValidateSet("dev", "release", "test", "dushnota")]
  [string] $Mode
)

$ErrorActionPreference = "Stop"

function ExitIfFailed() {
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

function ClippyCommonArgs() {
  @(
    "--", 
    "-D", "warnings",
    "-D", "clippy::all",
    "-D", "clippy::style",
    "-D", "clippy::complexity",
    "-D", "clippy::perf",
    "-D", "clippy::cargo",
    "-D", "clippy::unwrap_used",
    "-D", "clippy::expect_used",
    "-D", "clippy::todo",
    "-D", "clippy::unimplemented",
    "-D", "clippy::dbg_macro",
    "-A", "clippy::multiple_crate_versions"
  )
}

function ClippyDushnotaArgs() {
  @(
    "--",
    "-D", "warnings",
    "-D", "clippy::all",
    "-D", "clippy::style",
    "-D", "clippy::complexity",
    "-D", "clippy::perf",
    "-D", "clippy::pedantic",
    "-D", "clippy::cargo",
    "-D", "clippy::unwrap_used",
    "-D", "clippy::expect_used",
    "-D", "clippy::panic",
    "-D", "clippy::todo",
    "-D", "clippy::unimplemented",
    "-D", "clippy::dbg_macro",
    "-A", "clippy::multiple_crate_versions"
  )
}

cargo +nightly fmt --check
ExitIfFailed

if ($Mode -eq "dushnota") {
  $clippyArgs = ClippyDushnotaArgs
} else {
  $clippyArgs = ClippyCommonArgs
}

cargo +nightly clippy --all-targets --all-features @clippyArgs
ExitIfFailed

switch ($Mode) {
  "dev" {
    cargo +nightly run --features debug-tracing
    ExitIfFailed
  }
  "release" {
    cargo +nightly run --release --locked
    ExitIfFailed
  }
  "test" {
    cargo test
    ExitIfFailed
  }
  "dushnota" {
    exit 0
  }
}
