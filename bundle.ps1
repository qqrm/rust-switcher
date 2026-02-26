# bundle.ps1
# Single-file, double-click runnable PowerShell bundler for a monorepo.
#
# What it does:
# - Creates bundle.zip in the repo root (then renames to bundle_<md5-8>.zip)
# - Includes tracked + untracked files (i.e., “working tree”), but respects ALL nested .gitignore rules
# - Smart recursion: prunes ignored directories (does not traverse them)
# - Hard excludes anywhere: .git/, target/, bundle*.zip
# - Skips reparse points (junctions/symlinks) to avoid recursion traps
#
# Usage:
# - Double click bundle.ps1 (PowerShell will prompt/run depending on your policy)
# - Or run in a terminal:  powershell -ExecutionPolicy Bypass -File .\bundle.ps1

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Assert-GitPresent {
  if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    throw "git not found in PATH"
  }
}

function Get-RepoRoot {
  $root = (& git rev-parse --show-toplevel 2>$null)
  if (-not $root) { throw "not a git repository (git rev-parse failed)" }
  $root = $root.Trim()
  if (-not (Test-Path -LiteralPath $root -PathType Container)) {
    throw "repo root does not exist: $root"
  }
  return (Resolve-Path -LiteralPath $root).Path
}

function New-TempBundlePath {
  param([string]$Root)
  return (Join-Path $Root 'bundle.zip')
}

function Remove-IfExists {
  param([string]$Path)
  if (Test-Path -LiteralPath $Path) {
    Remove-Item -LiteralPath $Path -Force
  }
}

function Get-RelativePath {
  param([string]$Root, [string]$FullPath)
  $rel = [System.IO.Path]::GetRelativePath($Root, $FullPath)
  if ([string]::IsNullOrWhiteSpace($rel) -or $rel -eq '.') { return '' }

  # было (плохо для zip):
  # return ($rel -replace '/', '\')

  # должно быть (zip standard, кроссплатформа):
  return ($rel -replace '\\', '/')
}


function Test-HardExclude {
  param([string]$RelPath)

  if ([string]::IsNullOrWhiteSpace($RelPath)) { return $false }

  $r = $RelPath -replace '/', '\'
  $parts = $r -split '\\'
  foreach ($p in $parts) {
    if ($p -ieq '.git')   { return $true }
    if ($p -ieq 'target') { return $true }
  }

  $leaf = [System.IO.Path]::GetFileName($r)
  if ($leaf -ilike 'bundle*.zip') { return $true }

  return $false
}

function Test-GitIgnored {
  param(
    [Parameter(Mandatory=$true)][string]$Root,
    [Parameter(Mandatory=$true)][string]$RelPath
  )

  if ([string]::IsNullOrWhiteSpace($RelPath)) { return $false }

  # Critical: pass RELATIVE path to git, not absolute.
  & git -C $Root check-ignore -q -- $RelPath | Out-Null
  return ($LASTEXITCODE -eq 0)
}

function Test-ReparsePoint {
  param([System.IO.FileSystemInfo]$Item)
  return (($Item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)
}

function Open-ZipForCreate {
  param([string]$OutPath)
  Add-Type -AssemblyName System.IO.Compression
  Add-Type -AssemblyName System.IO.Compression.FileSystem
  return [System.IO.Compression.ZipFile]::Open($OutPath, [System.IO.Compression.ZipArchiveMode]::Create)
}

function Add-FileToZip {
  param(
    [Parameter(Mandatory=$true)][System.IO.Compression.ZipArchive]$Zip,
    [Parameter(Mandatory=$true)][string]$FullPath,
    [Parameter(Mandatory=$true)][string]$RelPath
  )
  if (-not (Test-Path -LiteralPath $FullPath -PathType Leaf)) { return }

  [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
    $Zip,
    $FullPath,
    $RelPath,
    [System.IO.Compression.CompressionLevel]::Optimal
  ) | Out-Null
}

function Walk-And-Pack {
  param(
    [Parameter(Mandatory=$true)][string]$Root,
    [Parameter(Mandatory=$true)][string]$Dir,
    [Parameter(Mandatory=$true)][System.IO.Compression.ZipArchive]$Zip
  )

  $items = Get-ChildItem -LiteralPath $Dir -Force -ErrorAction Stop
  foreach ($item in $items) {
    $full = $item.FullName
    $rel  = Get-RelativePath -Root $Root -FullPath $full

    if (Test-HardExclude -RelPath $rel) { continue }
    if (Test-GitIgnored -Root $Root -RelPath $rel) { continue }

    if ($item.PSIsContainer) {
      if (Test-ReparsePoint $item) { continue }
      Walk-And-Pack -Root $Root -Dir $full -Zip $Zip
      continue
    }

    Add-FileToZip -Zip $Zip -FullPath $full -RelPath $rel
  }
}

function Get-ShortMd5 {
  param([string]$Path)
  $h = (Get-FileHash -Algorithm MD5 -LiteralPath $Path).Hash
  if (-not $h) { throw "failed to hash: $Path" }
  return $h.Substring(0,8).ToLower()
}

function Rename-WithHash {
  param(
    [Parameter(Mandatory=$true)][string]$ZipPath
  )

  $dir  = Split-Path -Parent $ZipPath
  $base = [System.IO.Path]::GetFileNameWithoutExtension($ZipPath)
  $ext  = [System.IO.Path]::GetExtension($ZipPath)

  $hash = Get-ShortMd5 -Path $ZipPath
  $dst  = Join-Path $dir ("{0}_{1}{2}" -f $base, $hash, $ext)

  Remove-IfExists -Path $dst
  Move-Item -LiteralPath $ZipPath -Destination $dst -Force
  return $dst
}

function Main {
  Assert-GitPresent
  $root = Get-RepoRoot

  $out = New-TempBundlePath -Root $root
  Remove-IfExists -Path $out

  $zip = Open-ZipForCreate -OutPath $out
  try {
    Walk-And-Pack -Root $root -Dir $root -Zip $zip
  } finally {
    $zip.Dispose()
  }

  if (-not (Test-Path -LiteralPath $out -PathType Leaf)) {
    throw "bundle was not created: $out"
  }

  $final = Rename-WithHash -ZipPath $out
  Write-Host ("OK: {0}" -f $final)
}

try {
  Main
} catch {
  Write-Host ("ERROR: {0}" -f $_.Exception.Message)
  exit 1
}
