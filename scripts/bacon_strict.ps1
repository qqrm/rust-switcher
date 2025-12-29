param(
  [Parameter(Mandatory = $true)]
  [ValidateSet("dev", "release", "test", "dushnota")]
  [string] $Mode
)

$ErrorActionPreference = "Stop"

function ExitIfFailed() {
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

function RunCargoForwarded([string[]] $Args) {
  & cargo @Args 2>&1 | Out-Host
  ExitIfFailed
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

function ClippyDushnotaExtraArgs() {
  @(
    "-D", "clippy::pedantic",
    "-D", "clippy::panic"
  )
}

RunCargoForwarded @("+nightly", "fmt", "--check")

$clippyArgs = @(ClippyCommonArgs)
if ($Mode -eq "dushnota") {
  $clippyArgs += @(ClippyDushnotaExtraArgs)
}

& cargo +nightly clippy --all-targets --all-features @clippyArgs 2>&1 | Out-Host
ExitIfFailed

switch ($Mode) {
  "dev" {
    RunCargoForwarded @("+nightly", "run", "--features", "debug-tracing")
  }
  "release" {
    RunCargoForwarded @("+nightly", "run", "--release", "--locked")
  }
  "test" {
    RunCargoForwarded @("+nightly", "test")
  }
  "dushnota" {
    exit 0
  }
}
