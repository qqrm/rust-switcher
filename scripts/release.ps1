param(
  [Parameter(Mandatory = $false)]
  [string]$Version = ''
)

$ErrorActionPreference = 'Stop'

$script = Join-Path $PSScriptRoot 'release\release.ps1'

if (-not (Test-Path $script)) {
  throw "Expected release entrypoint not found: $script"
}

& pwsh -ExecutionPolicy Bypass -NoLogo -NoProfile -File $script $Version
