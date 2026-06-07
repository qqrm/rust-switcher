param(
    [Parameter(Mandatory = $true)]
    [string]$SharedRoot,

    [switch]$KeepOpen
)

$ErrorActionPreference = "Stop"

$logPath = Join-Path $SharedRoot "e2e.log"
$resultPath = Join-Path $SharedRoot "result.json"
$switcher = $null

function Write-Result {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Status,

        [string]$ErrorMessage,

        [hashtable]$Details = @{}
    )

    [ordered]@{
        status = $Status
        error = $ErrorMessage
        details = $Details
        timestampUtc = (Get-Date).ToUniversalTime().ToString("o")
    } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $resultPath -Encoding UTF8
}

function Add-NativeInputTypes {
    Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public static class NativeInput {
    [StructLayout(LayoutKind.Sequential)]
    public struct INPUT {
        public uint type;
        public INPUTUNION u;
    }

    [StructLayout(LayoutKind.Explicit)]
    public struct INPUTUNION {
        [FieldOffset(0)]
        public KEYBDINPUT ki;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct KEYBDINPUT {
        public ushort wVk;
        public ushort wScan;
        public uint dwFlags;
        public uint time;
        public IntPtr dwExtraInfo;
    }

    [DllImport("user32.dll", SetLastError = true)]
    public static extern uint SendInput(uint nInputs, INPUT[] pInputs, int cbSize);

    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr hWnd);

    [DllImport("user32.dll")]
    public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

    public const uint INPUT_KEYBOARD = 1;
    public const uint KEYEVENTF_KEYUP = 0x0002;
    public const ushort VK_LSHIFT = 0xA0;
    public const int SW_SHOWNORMAL = 1;

    public static void TapLeftShift() {
        INPUT[] inputs = new INPUT[2];
        inputs[0].type = INPUT_KEYBOARD;
        inputs[0].u.ki.wVk = VK_LSHIFT;
        inputs[1].type = INPUT_KEYBOARD;
        inputs[1].u.ki.wVk = VK_LSHIFT;
        inputs[1].u.ki.dwFlags = KEYEVENTF_KEYUP;
        uint sent = SendInput((uint)inputs.Length, inputs, Marshal.SizeOf(typeof(INPUT)));
        if (sent != inputs.Length) {
            throw new InvalidOperationException("SendInput failed");
        }
    }
}
"@
}

function Wait-Until {
    param(
        [Parameter(Mandatory = $true)]
        [scriptblock]$Predicate,

        [int]$TimeoutMilliseconds = 5000,
        [int]$StepMilliseconds = 50
    )

    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMilliseconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        if (& $Predicate) {
            return $true
        }
        Start-Sleep -Milliseconds $StepMilliseconds
    }
    return $false
}

function Get-PlaygroundElement {
    Add-Type -AssemblyName UIAutomationClient
    Add-Type -AssemblyName UIAutomationTypes

    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $windowCondition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::NameProperty,
        "RustSwitcher"
    )
    $window = $root.FindFirst(
        [System.Windows.Automation.TreeScope]::Children,
        $windowCondition
    )
    if ($null -eq $window) {
        return $null
    }

    $editCondition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
        [System.Windows.Automation.ControlType]::Edit
    )
    $edits = $window.FindAll(
        [System.Windows.Automation.TreeScope]::Descendants,
        $editCondition
    )

    foreach ($edit in $edits) {
        $patternObj = $null
        if (-not $edit.TryGetCurrentPattern(
            [System.Windows.Automation.ValuePattern]::Pattern,
            [ref]$patternObj
        )) {
            continue
        }

        $pattern = [System.Windows.Automation.ValuePattern]$patternObj
        if (-not $pattern.Current.IsReadOnly -and $pattern.Current.Value -eq "") {
            return $edit
        }
    }

    return $null
}

function Get-ValuePattern {
    param([Parameter(Mandatory = $true)]$Element)

    $patternObj = $null
    if (-not $Element.TryGetCurrentPattern(
        [System.Windows.Automation.ValuePattern]::Pattern,
        [ref]$patternObj
    )) {
        throw "Focused playground does not support ValuePattern"
    }
    return [System.Windows.Automation.ValuePattern]$patternObj
}

function Tap-LeftShift {
    param([int]$Count)

    for ($i = 0; $i -lt $Count; $i++) {
        [NativeInput]::TapLeftShift()
        Start-Sleep -Milliseconds 120
    }
}

function Wait-For-PlaygroundValue {
    param(
        [Parameter(Mandatory = $true)]
        $Pattern,

        [Parameter(Mandatory = $true)]
        [string]$Expected,

        [int]$TimeoutMilliseconds = 5000
    )

    Wait-Until -TimeoutMilliseconds $TimeoutMilliseconds -Predicate {
        $Pattern.Current.Value -eq $Expected
    }
}

try {
    Start-Transcript -LiteralPath $logPath -Force | Out-Null

    Add-Type -AssemblyName System.Windows.Forms
    Add-NativeInputTypes

    $exe = Join-Path $SharedRoot "rust-switcher-debug.exe"
    if (-not (Test-Path $exe)) {
        throw "rust-switcher-debug.exe is missing from $SharedRoot"
    }

    $appData = "C:\rust-switcher-e2e-appdata"
    Remove-Item -LiteralPath $appData -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Path (Join-Path $appData "RustSwitcherDebug") -Force | Out-Null

@"
start_minimized = false
theme_dark = false
smarter_hotkeys_enabled = true
"@ | Set-Content -LiteralPath (Join-Path $appData "RustSwitcherDebug\config.json") -Encoding UTF8

    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $exe
    $psi.WorkingDirectory = $SharedRoot
    $psi.UseShellExecute = $false
    $psi.Environment["APPDATA"] = $appData
    $psi.Environment["RUST_LOG"] = "trace"
    $psi.Environment["RUST_SWITCHER_E2E_PLAYGROUND"] = "1"
    $psi.Environment["RUST_SWITCHER_E2E_ALLOW_INJECTED"] = "1"
    $switcher = [System.Diagnostics.Process]::Start($psi)

    if (-not (Wait-Until -TimeoutMilliseconds 10000 -Predicate {
        $switcher.Refresh()
        $switcher.MainWindowHandle -ne [IntPtr]::Zero
    })) {
        throw "RustSwitcher window did not appear"
    }

    [NativeInput]::ShowWindow($switcher.MainWindowHandle, [NativeInput]::SW_SHOWNORMAL) | Out-Null
    [NativeInput]::SetForegroundWindow($switcher.MainWindowHandle) | Out-Null
    Start-Sleep -Milliseconds 800

    $playground = Get-PlaygroundElement
    if ($null -eq $playground) {
        throw "Playground edit was not found"
    }
    $playground.SetFocus()
    Start-Sleep -Milliseconds 300
    $value = Get-ValuePattern -Element $playground
    $value.SetValue("")

    [System.Windows.Forms.SendKeys]::SendWait("ghbdtn")
    Start-Sleep -Milliseconds 500
    if (-not (Wait-For-PlaygroundValue -Pattern $value -Expected "ghbdtn" -TimeoutMilliseconds 3000)) {
        throw "Initial playground typing failed. Value='$($value.Current.Value)'"
    }

    Tap-LeftShift -Count 3
    if (-not (Wait-For-PlaygroundValue -Pattern $value -Expected "привет" -TimeoutMilliseconds 6000)) {
        throw "Triple Shift smart conversion failed. Value='$($value.Current.Value)'"
    }

    Tap-LeftShift -Count 2
    if (-not (Wait-For-PlaygroundValue -Pattern $value -Expected "ghbdtn" -TimeoutMilliseconds 6000)) {
        throw "Deferred double Shift conversion failed. Value='$($value.Current.Value)'"
    }

    Write-Result -Status "passed" -Details @{
        finalValue = $value.Current.Value
        appData = $appData
        scenario = "ghbdtn -> 3 Shift -> привет -> 2 Shift -> ghbdtn"
    }
} catch {
    Write-Result -Status "failed" -ErrorMessage $_.Exception.Message -Details @{
        stack = $_.ScriptStackTrace
    }
    throw
} finally {
    if ($null -ne $switcher -and -not $switcher.HasExited) {
        Stop-Process -Id $switcher.Id -Force -ErrorAction SilentlyContinue
    }
    try {
        Stop-Transcript | Out-Null
    } catch {
    }
    if (-not $KeepOpen) {
        shutdown.exe /s /t 0 /f | Out-Null
    }
}
