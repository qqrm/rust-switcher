param(
    [Parameter(Mandatory = $true)]
    [string]$SharedRoot,

    [switch]$KeepOpen
)

$ErrorActionPreference = "Stop"

$logPath = Join-Path $SharedRoot "e2e.log"
$resultPath = Join-Path $SharedRoot "result.json"
$switcher = $null
$form = $null

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

function Pump-Ui {
    param([int]$Milliseconds)

    $deadline = [DateTime]::UtcNow.AddMilliseconds($Milliseconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        [System.Windows.Forms.Application]::DoEvents()
        Start-Sleep -Milliseconds 25
    }
}

function Send-Keys-And-Pump {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Keys,

        [int]$AfterMilliseconds = 250
    )

    [System.Windows.Forms.SendKeys]::SendWait($Keys)
    Pump-Ui -Milliseconds $AfterMilliseconds
}

function Wait-For-Text {
    param(
        [Parameter(Mandatory = $true)]
        [System.Windows.Forms.TextBox]$TextBox,

        [Parameter(Mandatory = $true)]
        [string]$Expected,

        [int]$TimeoutMilliseconds = 5000
    )

    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMilliseconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        [System.Windows.Forms.Application]::DoEvents()
        if ($TextBox.Text -eq $Expected) {
            return $true
        }
        Start-Sleep -Milliseconds 50
    }
    return $false
}

try {
    Start-Transcript -LiteralPath $logPath -Force | Out-Null

    $exe = Join-Path $SharedRoot "rust-switcher.exe"
    if (-not (Test-Path $exe)) {
        throw "rust-switcher.exe is missing from $SharedRoot"
    }

    $appData = "C:\rust-switcher-e2e-appdata"
    Remove-Item -LiteralPath $appData -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Path (Join-Path $appData "RustSwitcher") -Force | Out-Null
    $env:APPDATA = $appData
    $env:RUST_LOG = "trace"

    $config = [ordered]@{
        delay_ms = 100
        start_minimized = $true
        theme_dark = $false
        hotkey_convert_last_word = $null
        hotkey_convert_selection = $null
        hotkey_switch_layout = $null
        hotkey_pause = $null
        hotkey_convert_last_word_sequence = [ordered]@{
            first = [ordered]@{
                mods = 4
                mods_vks = 0
                vk = 123
            }
            second = $null
            max_gap_ms = 1000
        }
        hotkey_pause_sequence = $null
        hotkey_convert_selection_sequence = $null
        hotkey_switch_layout_sequence = $null
    }
    $config | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $appData "RustSwitcher\config.json") -Encoding UTF8

    Add-Type -AssemblyName System.Windows.Forms
    Add-Type -AssemblyName System.Drawing

    $english = [System.Windows.Forms.InputLanguage]::InstalledInputLanguages |
        Where-Object { $_.Culture.TwoLetterISOLanguageName -eq "en" } |
        Select-Object -First 1
    if ($null -ne $english) {
        [System.Windows.Forms.InputLanguage]::CurrentInputLanguage = $english
    }

    $switcher = Start-Process -FilePath $exe -WorkingDirectory $SharedRoot -PassThru
    Start-Sleep -Milliseconds 1200

    $form = New-Object System.Windows.Forms.Form
    $form.Text = "Rust Switcher E2E Host"
    $form.Width = 700
    $form.Height = 220
    $form.StartPosition = "CenterScreen"
    $form.TopMost = $true

    $textBox = New-Object System.Windows.Forms.TextBox
    $textBox.Multiline = $true
    $textBox.AcceptsReturn = $true
    $textBox.AcceptsTab = $true
    $textBox.Font = New-Object System.Drawing.Font("Consolas", 18)
    $textBox.Dock = [System.Windows.Forms.DockStyle]::Fill
    $form.Controls.Add($textBox)

    $form.Show()
    Pump-Ui -Milliseconds 500
    $form.Activate()
    $textBox.Focus()
    Pump-Ui -Milliseconds 500

    Send-Keys-And-Pump -Keys "ghbdtn world" -AfterMilliseconds 500
    if ($textBox.Text -ne "ghbdtn world") {
        throw "Initial typing failed. TextBox contains '$($textBox.Text)'"
    }

    Send-Keys-And-Pump -Keys "{LEFT 5}" -AfterMilliseconds 300
    if ($textBox.SelectionStart -ne 7) {
        throw "Caret setup failed. Expected SelectionStart=7, got $($textBox.SelectionStart)"
    }

    Send-Keys-And-Pump -Keys "+{F12}" -AfterMilliseconds 300
    if (-not (Wait-For-Text -TextBox $textBox -Expected "привет world" -TimeoutMilliseconds 6000)) {
        throw "Conversion failed. Expected 'привет world', got '$($textBox.Text)'"
    }

    if ($textBox.SelectionStart -ne 7) {
        throw "Caret position changed unexpectedly. Expected SelectionStart=7, got $($textBox.SelectionStart)"
    }

    Write-Result -Status "passed" -Details @{
        finalText = $textBox.Text
        selectionStart = $textBox.SelectionStart
        appData = $appData
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
    if ($null -ne $form) {
        $form.Close()
        $form.Dispose()
    }
    try {
        Stop-Transcript | Out-Null
    } catch {
    }
    if (-not $KeepOpen) {
        shutdown.exe /s /t 0 /f | Out-Null
    }
}
