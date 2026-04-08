param(
  [Parameter(Mandatory = $false)]
  [string]$Version = ''
)

. "$PSScriptRoot\common.ps1"

Assert-Tool git
Assert-Tool cargo

$repoRoot = Get-RepoRoot
Push-Location $repoRoot
try {
  Assert-CleanWorktree

  $appCur  = Get-CargoPackageVersion 'rust-switcher'
  $coreCur = Get-CargoPackageVersion 'rust-switcher-core'

  $target = if ([string]::IsNullOrWhiteSpace($Version)) {
    $base = Max-SemVer $appCur $coreCur
    Bump-Patch $base
  } else {
    Assert-SemVer $Version
  }

  $changed = $false
  $changed = (Update-CargoTomlPackageVersion -Path 'Cargo.toml' -NewVersion $target) -or $changed
  $changed = (Update-CargoTomlPackageVersion -Path 'crates/rust-switcher-core/Cargo.toml' -NewVersion $target) -or $changed
  $changed = (Update-DependencyVersionInCargoToml -Path 'Cargo.toml' -DependencyName 'rust-switcher-core' -NewVersion $target) -or $changed

  if (-not $changed) {
    Write-Host "No version changes required (already $target)."
    Write-Output $target
    return
  }

  # Update Cargo.lock deterministically for the new workspace versions.
  $null = Invoke-Checked cargo @('generate-lockfile') -Quiet

  $null = Invoke-Checked git @('add', 'Cargo.toml', 'Cargo.lock', 'crates/rust-switcher-core/Cargo.toml') -Quiet
  $msg = "chore: bump version to $target"

  $commit = Invoke-Checked git @('commit', '-m', $msg) -AllowFailure -Quiet
  if ($commit.ExitCode -ne 0) {
    if ($commit.Output -match 'nothing to commit') {
      Write-Host "Nothing to commit (already at $target)."
    } else {
      throw "git commit failed: $($commit.Output)"
    }
  }

  Write-Output $target
}
finally {
  Pop-Location
}
