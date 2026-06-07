param(
    [switch]$KeepOpen,
    [switch]$NoWait
)

$ErrorActionPreference = "Stop"

function XmlEscape([string]$Value) {
    return [System.Security.SecurityElement]::Escape($Value)
}

$root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$sandboxExe = Join-Path $env:WINDIR "System32\WindowsSandbox.exe"
if (-not (Test-Path $sandboxExe)) {
    throw "Windows Sandbox is not available at $sandboxExe. Enable the Windows Sandbox optional feature first."
}

Push-Location $root
try {
    $env:CARGO_TARGET_DIR = ".target"
    & cargo +nightly build --bin rust-switcher-debug --features debug-tracing,debug-binary
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed with exit code $LASTEXITCODE"
    }

    $exe = Join-Path $root ".target\debug\rust-switcher-debug.exe"
    if (-not (Test-Path $exe)) {
        throw "Built executable was not found at $exe"
    }

    $stamp = Get-Date -Format "yyyyMMdd-HHmmss"
    $bundle = Join-Path ([System.IO.Path]::GetTempPath()) "rust-switcher-e2e-$stamp"
    New-Item -ItemType Directory -Path $bundle -Force | Out-Null

    Copy-Item -LiteralPath $exe -Destination (Join-Path $bundle "rust-switcher-debug.exe") -Force
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot "sandbox-runner.ps1") -Destination $bundle -Force

    $leaf = Split-Path $bundle -Leaf
    $guestRoot = "C:\Users\WDAGUtilityAccount\Desktop\$leaf"
    $keepOpenArg = if ($KeepOpen) { " -KeepOpen" } else { "" }
    $command = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$guestRoot\sandbox-runner.ps1`" -SharedRoot `"$guestRoot`"$keepOpenArg"

    $wsb = Join-Path $bundle "rust-switcher-e2e.wsb"
    $hostFolder = XmlEscape $bundle
    $logonCommand = XmlEscape $command

    @"
<Configuration>
  <MappedFolders>
    <MappedFolder>
      <HostFolder>$hostFolder</HostFolder>
      <ReadOnly>false</ReadOnly>
    </MappedFolder>
  </MappedFolders>
  <ClipboardRedirection>Disable</ClipboardRedirection>
  <PrinterRedirection>Disable</PrinterRedirection>
  <Networking>Disable</Networking>
  <LogonCommand>
    <Command>$logonCommand</Command>
  </LogonCommand>
</Configuration>
"@ | Set-Content -LiteralPath $wsb -Encoding UTF8

    Write-Host "Launching Windows Sandbox..."
    Write-Host "Bundle: $bundle"

    $proc = Start-Process -FilePath $sandboxExe -ArgumentList "`"$wsb`"" -PassThru
    if ($NoWait) {
        Write-Host "Sandbox started. Result will be written to $bundle\result.json"
        return
    }

    $proc.WaitForExit()

    $resultPath = Join-Path $bundle "result.json"
    if (-not (Test-Path $resultPath)) {
        throw "Sandbox exited without writing $resultPath. Check $bundle\e2e.log"
    }

    $result = Get-Content -LiteralPath $resultPath -Raw | ConvertFrom-Json
    $result | ConvertTo-Json -Depth 8

    if ($result.status -ne "passed") {
        throw "Interactive E2E failed: $($result.error)"
    }
} finally {
    Pop-Location
}
