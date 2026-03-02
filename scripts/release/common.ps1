Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true

function Assert-Tool {
  param([Parameter(Mandatory = $true)][string]$Name)
  if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
    throw "Required tool not found in PATH: $Name"
  }
}

function Invoke-Checked {
  param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string[]]$Args,
    [switch]$AllowFailure,
    [switch]$Quiet
  )

  $out = & $Exe @Args 2>&1
  $code = $LASTEXITCODE

  if (-not $Quiet) {
    if ($out) { $out | ForEach-Object { Write-Host $_ } }
  }

  if (-not $AllowFailure -and $code -ne 0) {
    $text = if ($out) { ($out -join "`n") } else { "" }
    throw ("Command failed (exit {0}): {1} {2}`n{3}" -f $code, $Exe, ($Args -join ' '), $text)
  }

  return @{
    ExitCode = $code
    Output   = if ($out) { ($out -join "`n") } else { "" }
  }
}

function Get-RepoRoot {
  $r = Invoke-Checked git @('rev-parse', '--show-toplevel') -Quiet
  return $r.Output.Trim()
}

function Get-CurrentBranch {
  $r = Invoke-Checked git @('branch', '--show-current') -Quiet
  return $r.Output.Trim()
}

function Assert-OnBranch {
  param([Parameter(Mandatory = $true)][string]$Expected)
  $cur = Get-CurrentBranch
  if ($cur -ne $Expected) {
    throw "This command must be run on branch '$Expected' (current: '$cur')."
  }
}

function Assert-CleanWorktree {
  $r = Invoke-Checked git @('status', '--porcelain') -Quiet
  $s = $r.Output.Trim()
  if ($s) {
    throw "Worktree is dirty. Commit or stash changes first.`n$s"
  }
}

function Assert-SemVer {
  param([Parameter(Mandatory = $true)][string]$Version)
  $v = ($Version ?? '').Trim()
  if ($v -notmatch '^\d+\.\d+\.\d+$') {
    throw "Invalid VERSION '$Version'. Expected strict SemVer: X.Y.Z"
  }
  return $v
}

function Parse-SemVer {
  param([Parameter(Mandatory = $true)][string]$Version)
  $v = Assert-SemVer $Version
  $p = $v.Split('.')
  return @([int]$p[0], [int]$p[1], [int]$p[2])
}

function Compare-SemVer {
  param(
    [Parameter(Mandatory = $true)][string]$A,
    [Parameter(Mandatory = $true)][string]$B
  )

  $pa = Parse-SemVer $A
  $pb = Parse-SemVer $B
  for ($i = 0; $i -lt 3; $i++) {
    if ($pa[$i] -lt $pb[$i]) { return -1 }
    if ($pa[$i] -gt $pb[$i]) { return 1 }
  }
  return 0
}

function Max-SemVer {
  param(
    [Parameter(Mandatory = $true)][string]$A,
    [Parameter(Mandatory = $true)][string]$B
  )

  if ((Compare-SemVer $A $B) -ge 0) { return $A }
  return $B
}

function Bump-Patch {
  param([Parameter(Mandatory = $true)][string]$Version)
  $p = Parse-SemVer $Version
  return ("{0}.{1}.{2}" -f $p[0], $p[1], ($p[2] + 1))
}

function Get-CargoPackageVersion {
  param([Parameter(Mandatory = $true)][string]$Package)

  $raw = Invoke-Checked cargo @('metadata', '--no-deps', '--format-version', '1') -Quiet
  $meta = $raw.Output | ConvertFrom-Json
  $pkg = $meta.packages | Where-Object { $_.name -eq $Package } | Select-Object -First 1
  if (-not $pkg) {
    throw "Package '$Package' not found in cargo metadata."
  }
  $v = [string]$pkg.version
  Assert-SemVer $v | Out-Null
  return $v
}

function Update-CargoTomlPackageVersion {
  param(
    [Parameter(Mandatory = $true)][string]$Path,
    [Parameter(Mandatory = $true)][string]$NewVersion
  )

  if (-not (Test-Path $Path)) {
    throw "File not found: $Path"
  }

  $NewVersion = Assert-SemVer $NewVersion

  $lines = Get-Content -Path $Path -Encoding UTF8
  $out = New-Object System.Collections.Generic.List[string]
  $inPackage = $false
  $foundVersion = $false
  $changed = $false

  foreach ($line in $lines) {
    if ($line -match '^\[package\]\s*$') {
      $inPackage = $true
      $out.Add($line)
      continue
    }
    if ($line -match '^\[[^\]]+\]\s*$' -and $line -notmatch '^\[package\]\s*$') {
      $inPackage = $false
      $out.Add($line)
      continue
    }

    if ($inPackage -and $line -match '^\s*version\s*=\s*"([^"]+)"\s*$') {
      $cur = $Matches[1]
      $foundVersion = $true
      if ($cur -ne $NewVersion) {
        $out.Add("version = `"$NewVersion`"")
        $changed = $true
      } else {
        $out.Add($line)
      }
      continue
    }

    $out.Add($line)
  }

  if (-not $foundVersion) {
    throw "Did not find a [package] version line in $Path"
  }

  if ($changed) {
    Set-Content -Path $Path -Value $out -Encoding UTF8
  }

  return $changed
}

function Update-DependencyVersionInCargoToml {
  param(
    [Parameter(Mandatory = $true)][string]$Path,
    [Parameter(Mandatory = $true)][string]$DependencyName,
    [Parameter(Mandatory = $true)][string]$NewVersion
  )

  if (-not (Test-Path $Path)) {
    throw "File not found: $Path"
  }
  $NewVersion = Assert-SemVer $NewVersion

  $toml = Get-Content -Path $Path -Raw -Encoding UTF8
  $pattern = "(?m)^(\s*" + [regex]::Escape($DependencyName) + "\s*=\s*\{[^\n]*?\bversion\s*=\s*`")([^\`"]+)(`"[^\n]*\})"
  $updated = [regex]::Replace($toml, $pattern, ('$1' + $NewVersion + '$3'), 1)
  if ($updated -ne $toml) {
    Set-Content -Path $Path -Value $updated -Encoding UTF8
    return $true
  }
  return $false
}

function Get-TagSha {
  param([Parameter(Mandatory = $true)][string]$Tag)
  $r = Invoke-Checked git @('rev-parse', '-q', '--verify', "refs/tags/$Tag") -AllowFailure -Quiet
  if ($r.ExitCode -ne 0) { return $null }
  return $r.Output.Trim()
}

function Ensure-Tag-Immutable {
  param(
    [Parameter(Mandatory = $true)][string]$Tag,
    [Parameter(Mandatory = $true)][string]$ExpectedSha
  )

  $ExpectedSha = $ExpectedSha.Trim()
  $tagSha = Get-TagSha $Tag
  if ($tagSha) {
    if ($tagSha -ne $ExpectedSha) {
      throw "Tag '$Tag' already exists but points to $tagSha while HEAD is $ExpectedSha. Refusing to move tag."
    }
    return $false
  }

  Invoke-Checked git @('tag', $Tag, $ExpectedSha) -Quiet
  return $true
}