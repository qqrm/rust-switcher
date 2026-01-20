@echo off
setlocal enabledelayedexpansion

rem Build a review bundle zip using git archive (tracked files only).
rem Excludes: .git, target, and anything not explicitly included below (e.g. assets).

for /f "usebackq delims=" %%R in (`git rev-parse --show-toplevel 2^>nul`) do set "REPO_ROOT=%%R"
if "%REPO_ROOT%"=="" (
  echo ERROR: not a git repository
  exit /b 1
)

pushd "%REPO_ROOT%" >nul

for /f "usebackq delims=" %%H in (`git rev-parse --short HEAD 2^>nul`) do set "GIT_SHA=%%H"
if "%GIT_SHA%"=="" set "GIT_SHA=unknown"

for /f "usebackq tokens=1-4 delims=.:/ " %%a in ("%date% %time%") do (
  set "D1=%%a"
  set "D2=%%b"
  set "D3=%%c"
  set "T1=%%d"
)

rem Output name: rust-switcher-gpt-YYYY-MM-DD-HHMM-<sha>.zip
set "OUT_NAME=rust-switcher-gpt-%D3%-%D2%-%D1%-%time:~0,2%%time:~3,2%-%GIT_SHA%.zip"
set "OUT_NAME=%OUT_NAME: =0%"

if exist "%OUT_NAME%" del /f /q "%OUT_NAME%" >nul 2>&1

git archive --format=zip --output "%OUT_NAME%" HEAD ^
  .cargo ^
  .github ^
  .vscode ^
  docs ^
  res ^
  src ^
  Cargo.toml ^
  Cargo.lock ^
  build.rs ^
  rust-toolchain.toml ^
  rustfmt.toml ^
  README.md ^
  LICENSE ^
  bacon.toml ^
  .gitignore

if errorlevel 1 (
  echo ERROR: git archive failed
  popd >nul
  exit /b 1
)

echo OK: %OUT_NAME%

popd >nul
exit /b 0
