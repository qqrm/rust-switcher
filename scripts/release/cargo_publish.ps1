. "$PSScriptRoot\common.ps1"

Assert-Tool cargo
Assert-Tool git

$repoRoot = Get-RepoRoot
Push-Location $repoRoot
try {
  function Publish-IfNeeded {
    param([Parameter(Mandatory = $true)][string]$Package)

    Write-Host "`n>> cargo publish -p $Package --locked"
    $r = Invoke-Checked cargo @('publish', '-p', $Package, '--locked') -AllowFailure
    if ($r.ExitCode -eq 0) {
      return
    }

    $t = $r.Output
    if ($t -match 'already uploaded|is already uploaded|already exists|is already in the index|has already been published|already published') {
      Write-Host "cargo publish -p $Package: already published; treating as success."
      return
    }
    throw "cargo publish -p $Package failed.`n$t"
  }

  function Publish-App-With-DependencyRetry {
    param(
      [Parameter(Mandatory = $true)][string]$Package,
      [int]$MaxAttempts = 20,
      [int]$SleepSeconds = 15
    )

    for ($i = 1; $i -le $MaxAttempts; $i++) {
      Write-Host "`n>> cargo publish -p $Package --locked (attempt $i/$MaxAttempts)"
      $r = Invoke-Checked cargo @('publish', '-p', $Package, '--locked') -AllowFailure
      if ($r.ExitCode -eq 0) { return }

      $t = $r.Output
      if ($t -match 'already uploaded|is already uploaded|already exists|is already in the index|has already been published|already published') {
        Write-Host "cargo publish -p $Package: already published; treating as success."
        return
      }

      # crates.io index propagation: app publish may fail briefly right after core publish.
      if ($t -match 'failed to select a version for the requirement|no matching package named|could not find.*rust-switcher-core|rust-switcher-core.*not found') {
        if ($i -lt $MaxAttempts) {
          Write-Host "Dependency index not ready yet; retrying in $SleepSeconds seconds..."
          Start-Sleep -Seconds $SleepSeconds
          continue
        }
      }

      throw "cargo publish -p $Package failed.`n$t"
    }
  }

  # Dependency order: core first, then app.
  Publish-IfNeeded 'rust-switcher-core'
  Publish-App-With-DependencyRetry 'rust-switcher'
}
finally {
  Pop-Location
}
