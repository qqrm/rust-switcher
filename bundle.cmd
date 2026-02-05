@echo off
setlocal EnableExtensions EnableDelayedExpansion

rem Build bundle.zip from the current working tree (includes uncommitted changes).
rem File list is produced by Git, so .gitignore is respected.
rem Excludes: .git, target, and bundle.zip itself.

for /f "usebackq delims=" %%R in (`git rev-parse --show-toplevel 2^>nul`) do set "REPO_ROOT=%%R"
if "%REPO_ROOT%"=="" (
  echo ERROR: not a git repository
  exit /b 1
)

pushd "%REPO_ROOT%" >nul

set "OUT_NAME=bundle.zip"

if exist "%OUT_NAME%" del /f /q "%OUT_NAME%" >nul 2>&1

powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "$ErrorActionPreference='Stop';" ^
  "$root = (Resolve-Path -LiteralPath '%REPO_ROOT%').Path;" ^
  "$out  = Join-Path $root '%OUT_NAME%';" ^
  "Add-Type -AssemblyName System.IO.Compression;" ^
  "Add-Type -AssemblyName System.IO.Compression.FileSystem;" ^
  "if (Test-Path -LiteralPath $out) { Remove-Item -LiteralPath $out -Force }" ^
  "" ^
  "$zip = [System.IO.Compression.ZipFile]::Open($out, [System.IO.Compression.ZipArchiveMode]::Create);" ^
  "try {" ^
  "  $gitFiles = git -C $root ls-files -co --exclude-standard | ForEach-Object { $_.Trim() } | Where-Object { $_ -ne '' };" ^
  "  foreach ($rel in $gitFiles) {" ^
  "    $relNorm = $rel -replace '/', '\\';" ^
  "" ^
  "    if ($relNorm -match '^(?:\.git\\\\|target\\\\)') { continue }" ^
  "    if ($relNorm -ieq '%OUT_NAME%') { continue }" ^
  "" ^
  "    $full = Join-Path $root $relNorm;" ^
  "    if (-not (Test-Path -LiteralPath $full -PathType Leaf)) { continue }" ^
  "" ^
  "    [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile(" ^
  "      $zip, $full, $relNorm, [System.IO.Compression.CompressionLevel]::Optimal" ^
  "    ) | Out-Null;" ^
  "  }" ^
  "} finally { $zip.Dispose() }"

if errorlevel 1 (
  echo ERROR: bundle build failed
  popd >nul
  exit /b 1
)

rem Compute a cheap content hash (MD5) and rename the bundle as: bundle_<hash>.zip
set "BUNDLE_HASH="
for /f "usebackq delims=" %%H in (`powershell -NoProfile -Command "(Get-FileHash -Algorithm MD5 -LiteralPath '%OUT_NAME%').Hash.Substring(0,8).ToLower()"`) do set "BUNDLE_HASH=%%H"

if "%BUNDLE_HASH%"=="" (
  echo ERROR: failed to compute bundle hash
  popd >nul
  exit /b 1
)

for %%A in ("%OUT_NAME%") do (
  set "BASE=%%~nA"
  set "EXT=%%~xA"
)

set "OUT_NAME_HASHED=%BASE%_%BUNDLE_HASH%%EXT%"

if exist "%OUT_NAME_HASHED%" del /f /q "%OUT_NAME_HASHED%" >nul 2>&1
move /y "%OUT_NAME%" "%OUT_NAME_HASHED%" >nul

if errorlevel 1 (
  echo ERROR: failed to rename bundle
  popd >nul
  exit /b 1
)

echo OK: %OUT_NAME_HASHED%

popd >nul
exit /b 0
