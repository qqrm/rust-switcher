$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true

# Preconditions
$branch = (git branch --show-current).Trim()
if ($branch -ne 'dev') { throw "publish must be run on branch 'dev' (current: $branch)" }

# Ensure we can talk to GitHub
gh auth status | Out-Null

# Find or create PR dev -> main
$PSNativeCommandUseErrorActionPreference = $false
$prNum = (gh pr list --base main --head dev --state open --json number --jq '.[0].number' 2>$null).Trim()
$PSNativeCommandUseErrorActionPreference = $true

if (-not $prNum) {
  $title = "chore: release dev -> main"
  $body  = "Automated release PR created by scripts/release/publish.ps1"
  gh pr create --base main --head dev --title $title --body $body | Out-Null

  $PSNativeCommandUseErrorActionPreference = $false
  $prNum = (gh pr list --base main --head dev --state open --json number --jq '.[0].number' 2>$null).Trim()
  $PSNativeCommandUseErrorActionPreference = $true
  if (-not $prNum) { throw "Failed to create or locate PR dev -> main." }
}

# Enable auto-merge (squash)
gh pr merge $prNum --auto --squash