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

  Invoke-Checked git @('fetch', 'origin', '--tags', '--prune') -Quiet

  $v = if ([string]::IsNullOrWhiteSpace($Version)) {
    Get-CargoPackageVersion 'rust-switcher'
  } else {
    Assert-SemVer $Version
  }

  $tag = "v$v"
  $head = (Invoke-Checked git @('rev-parse', 'HEAD') -Quiet).Output.Trim()

  $tagSha = Get-TagSha $tag
  if (-not $tagSha) {
    throw "Tag '$tag' does not exist. Create it first (recommended: just release $v)."
  }
  if ($tagSha -ne $head) {
    throw "Tag '$tag' points to $tagSha while HEAD is $head. Refusing to upload assets for a mismatched tag."
  }

  Write-Host "`n>> cargo build --release"
  Invoke-Checked cargo @('build', '--release')

  $exe = Join-Path $repoRoot 'target\release\rust-switcher.exe'
  if (-not (Test-Path $exe)) {
    throw "Expected build artifact not found: $exe"
  }

  $assets = @($exe)
  $pdb = Join-Path $repoRoot 'target\release\rust-switcher.pdb'
  if (Test-Path $pdb) {
    $assets += $pdb
  }

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

    Invoke-Checked gh @('auth', 'status') -Quiet

    $view = Invoke-Checked gh @('release', 'view', $tag) -AllowFailure -Quiet
    if ($view.ExitCode -ne 0) {
      Invoke-Checked gh @('release', 'create', $tag, '--title', $tag, '--notes-file', $notesFile, '--target', 'dev', '--verify-tag')
    } else {
      Invoke-Checked gh @('release', 'edit', $tag, '--title', $tag, '--notes-file', $notesFile, '--target', 'dev', '--verify-tag')
    }

    $uploadArgs = @('release', 'upload', $tag) + $assets + @('--clobber')
    Invoke-Checked gh $uploadArgs

    $url = (Invoke-Checked gh @('release', 'view', $tag, '--json', 'url', '-q', '.url') -Quiet).Output.Trim()
    Write-Host "`nOK: GitHub Release updated: $url"
  }
  finally {
    Remove-Item -Force $notesFile -ErrorAction SilentlyContinue
  }
}
finally {
  Pop-Location
}
