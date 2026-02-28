param(
  [Parameter(Mandatory=$true)]
  [string] $Version
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true

if ([string]::IsNullOrWhiteSpace($Version)) {
  throw "VERSION is required (example: just bump 1.2.3)"
}

$toml = Get-Content .\Cargo.toml -Raw
$updated = [regex]::Replace($toml, '(?m)^version\s*=\s*"[^"]+"', ('version = "' + $Version + '"'), 1)
if ($toml -eq $updated) {
  throw "Failed to update version in Cargo.toml"
}
Set-Content -NoNewline -Encoding UTF8 -Path .\Cargo.toml -Value $updated

# Update Cargo.lock if it exists
$PSNativeCommandUseErrorActionPreference = $false
git ls-files --error-unmatch .\Cargo.lock 1>$null 2>$null
$hasLock = ($LASTEXITCODE -eq 0)
$PSNativeCommandUseErrorActionPreference = $true
if ($hasLock) {
  cargo update -p rust-switcher --precise $Version
}

git add .\Cargo.toml
if ($hasLock) { git add .\Cargo.lock }

git commit -m ("chore: bump version to " + $Version)