param(
  [Parameter(Mandatory = $false)]
  [ValidateSet('patch', 'minor', 'major')]
  [string]$Bump = 'patch',

  [Parameter(Mandatory = $false)]
  [string]$Package = 'rust-switcher',

  [Parameter(Mandatory = $false)]
  [string]$Remote = 'origin',

  [Parameter(Mandatory = $false)]
  [string]$Branch = 'dev',

  [Parameter(Mandatory = $false)]
  [string]$Main = 'main',

  [switch]$DryRun,
  [switch]$RunChecks,
  [switch]$Commit,
  [switch]$Push,
  [switch]$CreatePr,
  [switch]$MergePr,
  [switch]$Tag,
  [switch]$TagOnMain
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Invoke-Cmd {
  param(
    [Parameter(Mandatory = $true)][string]$Cmd,
    [switch]$AllowFail
  )

  Write-Host "`n>> $Cmd"
  if ($AllowFail) {
    & pwsh -NoLogo -NoProfile -Command $Cmd
    return $LASTEXITCODE
  }

  & pwsh -NoLogo -NoProfile -Command $Cmd
  if ($LASTEXITCODE -ne 0) {
    throw "Command failed with exit code $LASTEXITCODE: $Cmd"
  }
  return 0
}

function Assert-Tool {
  param([string]$Name)
  if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
    throw "Required tool not found in PATH: $Name"
  }
}

function Assert-CleanWorkingTree {
  $st = (git status --porcelain)
  if ($st) {
    throw "Working tree is not clean. Commit/stash changes first.`n$st"
  }
}

function Get-CargoVersion {
  param([string]$Pkg)

  $metaRaw = cargo metadata --no-deps --format-version 1
  $meta = $metaRaw | ConvertFrom-Json
  $pkgObj = $meta.packages | Where-Object { $_.name -eq $Pkg } | Select-Object -First 1
  if (-not $pkgObj) {
    throw "Package '$Pkg' not found in cargo metadata"
  }
  if (-not ($pkgObj.version -match '^\d+\.\d+\.\d+$')) {
    throw "Package version is not strict semver X.Y.Z: $($pkgObj.version)"
  }
  return [string]$pkgObj.version
}

function Bump-Semver {
  param(
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][ValidateSet('patch', 'minor', 'major')][string]$Kind
  )

  $parts = $Version.Split('.')
  if ($parts.Count -ne 3) {
    throw "Invalid semver: $Version"
  }
  $maj = [int]$parts[0]
  $min = [int]$parts[1]
  $pat = [int]$parts[2]

  switch ($Kind) {
    'major' { $maj += 1; $min = 0; $pat = 0 }
    'minor' { $min += 1; $pat = 0 }
    'patch' { $pat += 1 }
  }

  return "$maj.$min.$pat"
}

function Set-CargoTomlVersion {
  param(
    [Parameter(Mandatory = $true)][string]$NewVersion
  )

  if (-not (Test-Path 'Cargo.toml')) {
    throw "Cargo.toml not found in current directory"
  }

  $lines = Get-Content 'Cargo.toml' -Encoding UTF8
  $out = New-Object System.Collections.Generic.List[string]
  $inPackage = $false
  $done = $false

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

    if ($inPackage -and -not $done -and $line -match '^\s*version\s*=') {
      $out.Add("version = `"$NewVersion`"")
      $done = $true
      continue
    }

    $out.Add($line)
  }

  if (-not $done) {
    throw "Did not find a 'version = ...' line inside [package] in Cargo.toml"
  }

  Set-Content -Path 'Cargo.toml' -Value $out -Encoding UTF8
}

function Ensure-GitIdentity {
  $name = (git config user.name)
  $email = (git config user.email)

  if (-not $name) {
    git config user.name "qqrm"
  }
  if (-not $email) {
    git config user.email "qqrm@users.noreply.github.com"
  }
}

function Ensure-OnBranch {
  param([string]$Expected)
  $cur = (git rev-parse --abbrev-ref HEAD).Trim()
  if ($cur -ne $Expected) {
    throw "Expected current branch '$Expected', but got '$cur'"
  }
}

function Git-FetchPull {
  param([string]$RemoteName, [string]$BranchName)
  Invoke-Cmd "git fetch $RemoteName --prune" | Out-Null
  Invoke-Cmd "git pull --ff-only $RemoteName $BranchName" | Out-Null
}

function Open-OrCreatePr {
  param(
    [string]$Base,
    [string]$Head,
    [string]$Title
  )

  Assert-Tool 'gh'

  $existing = gh pr list --base $Base --head $Head --state open --json number,url --jq '.[0]'
  if ($existing) {
    $obj = $existing | ConvertFrom-Json
    Write-Host "PR already exists: $($obj.url)"
    return $obj
  }

  $url = gh pr create --base $Base --head $Head --title $Title --body ""
  $num = gh pr view $url --json number,url --jq '{number:.number,url:.url}' | ConvertFrom-Json
  Write-Host "Created PR: $($num.url)"
  return $num
}

function Merge-Pr {
  param([int]$Number, [string]$Title)
  Assert-Tool 'gh'

  # Will fail if branch protection forbids it.
  Invoke-Cmd "gh pr merge $Number --squash --subject `"$Title`" --body `"`"" | Out-Null
}

function Create-AndPushTag {
  param(
    [string]$TagName,
    [string]$RemoteName
  )

  $exists = (git tag -l $TagName)
  if ($exists) {
    throw "Tag already exists locally: $TagName"
  }

  Invoke-Cmd "git tag $TagName" | Out-Null
  Invoke-Cmd "git push $RemoteName $TagName" | Out-Null
}

function Rollback-Tag {
  param(
    [string]$TagName,
    [string]$RemoteName
  )
  Write-Host "Rolling back tag: $TagName"
  Invoke-Cmd "git tag -d $TagName" -AllowFail | Out-Null
  Invoke-Cmd "git push $RemoteName :refs/tags/$TagName" -AllowFail | Out-Null
}

# -----------------------------
# Flow
# -----------------------------

Assert-Tool 'git'
Assert-Tool 'cargo'

$tagCreated = $false
$tagName = $null
$success = $false

try {
  Ensure-OnBranch $Branch
  Assert-CleanWorkingTree

  Git-FetchPull -RemoteName $Remote -BranchName $Branch

  $current = Get-CargoVersion -Pkg $Package
  $next = Bump-Semver -Version $current -Kind $Bump

  Write-Host "Current version: $current"
  Write-Host "Next version:    $next"

  if ($DryRun) {
    $success = $true
    exit 0
  }

  if ($RunChecks) {
    Invoke-Cmd "cargo fmt --all -- --check" | Out-Null
    Invoke-Cmd "cargo clippy --workspace --all-targets --all-features --locked -- -D warnings" | Out-Null
    Invoke-Cmd "cargo test --workspace --all-features --all-targets --locked" | Out-Null
  }

  Set-CargoTomlVersion -NewVersion $next
  Invoke-Cmd "cargo generate-lockfile" | Out-Null

  $changed = (git diff --name-only)
  if (-not $changed) {
    throw "No changes after version bump. Refusing to continue."
  }

  if ($Commit) {
    Ensure-GitIdentity
    Invoke-Cmd "git add Cargo.toml Cargo.lock" | Out-Null
    Invoke-Cmd "git commit -m `"release: v$next`"" | Out-Null
  }

  if ($Push) {
    Invoke-Cmd "git push $Remote HEAD:$Branch" | Out-Null
  }

  if ($CreatePr) {
    $pr = Open-OrCreatePr -Base $Main -Head $Branch -Title "release: v$next"
    if ($MergePr) {
      Merge-Pr -Number $pr.number -Title "release: v$next"
    }
  }

  if ($Tag) {
    $tagName = "v$next"
    if ($TagOnMain -and $MergePr) {
      Invoke-Cmd "git fetch $Remote $Main --force" | Out-Null
      Invoke-Cmd "git checkout $Main" | Out-Null
      Invoke-Cmd "git reset --hard $Remote/$Main" | Out-Null
      Create-AndPushTag -TagName $tagName -RemoteName $Remote
      $tagCreated = $true
      Invoke-Cmd "git checkout $Branch" | Out-Null
    } else {
      Create-AndPushTag -TagName $tagName -RemoteName $Remote
      $tagCreated = $true
    }
  }

  $success = $true
} catch {
  Write-Host "ERROR: $($_.Exception.Message)" -ForegroundColor Red
  throw
} finally {
  if (-not $success -and $tagCreated -and $tagName) {
    Rollback-Tag -TagName $tagName -RemoteName $Remote
  }
}
