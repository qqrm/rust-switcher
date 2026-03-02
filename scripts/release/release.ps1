param(
  [Parameter(Mandatory = $false)]
  [string]$Version = ''
)

. "$PSScriptRoot\common.ps1"

Assert-Tool git
Assert-Tool cargo
Assert-Tool gh

$repoRoot = Get-RepoRoot
Push-Location $repoRoot
try {
  Assert-OnBranch 'dev'
  Assert-CleanWorktree

  # Ensure we see remote tags for immutability checks.
  Invoke-Checked git @('fetch', 'origin', '--tags', '--prune') -Quiet

  # Determine target version (for early tag immutability validation).
  $appCur  = Get-CargoPackageVersion 'rust-switcher'
  $coreCur = Get-CargoPackageVersion 'rust-switcher-core'

  $target = if ([string]::IsNullOrWhiteSpace($Version)) {
    $base = Max-SemVer $appCur $coreCur
    Bump-Patch $base
  } else {
    Assert-SemVer $Version
  }

  $tag = "v$target"
  $headBefore = (Invoke-Checked git @('rev-parse', 'HEAD') -Quiet).Output.Trim()

  $tagSha = Get-TagSha $tag
  if ($tagSha -and $tagSha -ne $headBefore) {
    throw "Tag '$tag' already exists but points to $tagSha while HEAD is $headBefore. Refusing to continue."
  }

  # Checks (must pass before version bump / release).
  Write-Host "`n>> cargo fmt --all -- --check"
  Invoke-Checked cargo @('fmt', '--all', '--', '--check')

  Write-Host "`n>> cargo clippy --workspace --all-targets --all-features --locked -- -D warnings"
  Invoke-Checked cargo @('clippy', '--workspace', '--all-targets', '--all-features', '--locked', '--', '-D', 'warnings')

  Write-Host "`n>> cargo test --workspace --all-features --all-targets --locked"
  Invoke-Checked cargo @('test', '--workspace', '--all-features', '--all-targets', '--locked')

  # If tag already exists at HEAD, we treat the bump step as already done.
  $didBump = $false
  if (-not $tagSha) {
    Write-Host "`n>> bumping version to $target"
    $newV = & pwsh -ExecutionPolicy Bypass -NoLogo -NoProfile -File "$PSScriptRoot\bump.ps1" $target
    $newV = ($newV | Select-Object -Last 1).Trim()
    if ($newV -ne $target) {
      throw "bump.ps1 returned '$newV' but expected '$target'"
    }
    $didBump = $true
  } else {
    # Sanity: tag points to HEAD; versions must match target.
    $appNow  = Get-CargoPackageVersion 'rust-switcher'
    $coreNow = Get-CargoPackageVersion 'rust-switcher-core'
    if ($appNow -ne $target -or $coreNow -ne $target) {
      throw "Tag '$tag' points to HEAD but Cargo.toml versions are app=$appNow core=$coreNow (expected $target). Refusing to proceed."
    }
  }

  Assert-CleanWorktree

  # Push dev (normal push only).
  Write-Host "`n>> git push origin dev"
  Invoke-Checked git @('push', 'origin', 'dev')

  # Re-fetch tags to reduce race with concurrent tag creation.
  Invoke-Checked git @('fetch', 'origin', '--tags', '--prune') -Quiet

  # Create immutable tag at HEAD (or verify existing).
  $head = (Invoke-Checked git @('rev-parse', 'HEAD') -Quiet).Output.Trim()
  $created = Ensure-Tag-Immutable -Tag $tag -ExpectedSha $head

  Write-Host "`n>> git push origin $tag"
  Invoke-Checked git @('push', 'origin', $tag)

  # Build artifacts.
  Write-Host "`n>> cargo build --release"
  Invoke-Checked cargo @('build', '--release')

  $exe = Join-Path $repoRoot 'target\release\rust-switcher.exe'
  if (-not (Test-Path $exe)) {
    throw "Expected build artifact not found: $exe"
  }
  $assets = @($exe)
  $pdb = Join-Path $repoRoot 'target\release\rust-switcher.pdb'
  if (Test-Path $pdb) { $assets += $pdb }

  # Create/update GitHub Release and upload assets.
  Write-Host "`n>> GitHub Release $tag (create/edit + upload assets)"
  Invoke-Checked gh @('auth', 'status') -Quiet

  $notesFile = New-TemporaryFile
  try {
    $prev = (Invoke-Checked git @('tag', '--list', 'v*', '--sort=-v:refname') -Quiet).Output.Split("`n") |
      ForEach-Object { $_.Trim() } |
      Where-Object { $_ -and $_ -ne $tag } |
      Select-Object -First 1

    $changes = if ($prev) {
      (Invoke-Checked git @('log', '--oneline', "$prev..$tag") -Quiet).Output.Trim()
    } else {
      (Invoke-Checked git @('log', '--oneline', '-n', '20') -Quiet).Output.Trim()
    }

    $notes = @(
      "Release $tag",
      "",
      "Commit: $head",
      "",
      "Changes:",
      ($changes ?? '(no commits found)')
    ) -join "`n"
    Set-Content -Path $notesFile -Value $notes -Encoding UTF8

    $view = Invoke-Checked gh @('release', 'view', $tag) -AllowFailure -Quiet
    if ($view.ExitCode -ne 0) {
      Invoke-Checked gh @('release', 'create', $tag, '--title', $tag, '--notes-file', $notesFile, '--target', 'dev', '--verify-tag')
    } else {
      Invoke-Checked gh @('release', 'edit', $tag, '--title', $tag, '--notes-file', $notesFile, '--target', 'dev', '--verify-tag')
    }

    $uploadArgs = @('release', 'upload', $tag) + $assets + @('--clobber')
    Invoke-Checked gh $uploadArgs
  }
  finally {
    Remove-Item -Force $notesFile -ErrorAction SilentlyContinue
  }

  # Publish to crates.io in dependency order.
  Write-Host "`n>> crates.io publish"
  & pwsh -ExecutionPolicy Bypass -NoLogo -NoProfile -File "$PSScriptRoot\cargo_publish.ps1"

  $releaseUrl = (Invoke-Checked gh @('release', 'view', $tag, '--json', 'url', '-q', '.url') -Quiet).Output.Trim()
  $sha = (Invoke-Checked git @('rev-parse', 'HEAD') -Quiet).Output.Trim()

  Write-Host "`nOK"
  Write-Host ("  version: {0}" -f $target)
  Write-Host ("  tag:     {0}" -f $tag)
  Write-Host ("  head:    {0}" -f $sha)
  if ($releaseUrl) { Write-Host ("  release: {0}" -f $releaseUrl) }
  if ($didBump) { Write-Host "  bump:    committed + pushed" }
  if ($created) { Write-Host "  tag:     created" } else { Write-Host "  tag:     already existed" }
}
finally {
  Pop-Location
}
