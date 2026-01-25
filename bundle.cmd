@echo off
setlocal EnableExtensions EnableDelayedExpansion

rem Build a review bundle ZIP from the current working tree (includes uncommitted changes).
rem Excludes: .git, target

for /f "usebackq delims=" %%R in (`git rev-parse --show-toplevel 2^>nul`) do set "REPO_ROOT=%%R"
if "%REPO_ROOT%"=="" (
  echo ERROR: not a git repository
  exit /b 1
)

pushd "%REPO_ROOT%" >nul

for /f "usebackq delims=" %%H in (`git rev-parse --short HEAD 2^>nul`) do set "GIT_SHA=%%H"
if "%GIT_SHA%"=="" set "GIT_SHA=unknown"

for /f "usebackq delims=" %%T in (`powershell -NoProfile -Command "Get-Date -Format 'yyyy-MM-dd-HHmm'"`) do set "TS=%%T"

rem Output name: rust-switcher-YYYY-MM-DD-HHMM-<sha>.zip
set "OUT_NAME=rust-switcher-%TS%-%GIT_SHA%.zip"

if exist "%OUT_NAME%" del /f /q "%OUT_NAME%" >nul 2>&1

powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "$ErrorActionPreference='Stop';" ^
  "$root = (Resolve-Path -LiteralPath '%REPO_ROOT%').Path;" ^
  "$out  = Join-Path $root '%OUT_NAME%';" ^
  "Add-Type -AssemblyName System.IO.Compression;" ^
  "Add-Type -AssemblyName System.IO.Compression.FileSystem;" ^
  "if (Test-Path -LiteralPath $out) { Remove-Item -LiteralPath $out -Force }" ^
  "$zip = [System.IO.Compression.ZipFile]::Open($out, [System.IO.Compression.ZipArchiveMode]::Create);" ^
  "try {" ^
  "  $files = Get-ChildItem -LiteralPath $root -Recurse -File -Force | Where-Object {" ^
  "    $p = $_.FullName;" ^
  "    $rel = $p.Substring($root.Length).TrimStart('\','/');" ^
  "    ($rel -notmatch '^(?:\.git[\\/]|target[\\/])') -and ($rel -ne '%OUT_NAME%')" ^
  "  };" ^
  "  foreach ($f in $files) {" ^
  "    $full = $f.FullName;" ^
  "    $rel  = $full.Substring($root.Length).TrimStart('\','/');" ^
  "    [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, $full, $rel, [System.IO.Compression.CompressionLevel]::Optimal) | Out-Null;" ^
  "  }" ^
  "} finally { $zip.Dispose() }"

if errorlevel 1 (
  echo ERROR: bundle build failed
  popd >nul
  exit /b 1
)

echo OK: %OUT_NAME%

popd >nul
exit /b 0
