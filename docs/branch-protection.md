# Branch Protection (gh)

Recommended protected branches: `dev`, `main`.

## Required status checks

Use these check names from workflow runs:
- `fmt`
- `clippy-lib-tests`
- `test-lib-tests`
- `windows-full`
- `actionlint`
- `typos`
- `zizmor`

## Apply protection (example)

```powershell
# dev
$payload = @{
  required_status_checks = @{
    strict   = $true
    contexts = @('fmt', 'clippy-lib-tests', 'test-lib-tests', 'windows-full', 'actionlint', 'typos', 'zizmor')
  }
  enforce_admins = $false
  required_pull_request_reviews = @{
    required_approving_review_count = 1
    dismiss_stale_reviews = $true
    require_code_owner_reviews = $false
  }
  restrictions = $null
  required_linear_history = $true
  allow_force_pushes = $false
  allow_deletions = $false
  block_creations = $false
  required_conversation_resolution = $true
  lock_branch = $false
  allow_fork_syncing = $true
}

$payloadJson = $payload | ConvertTo-Json -Depth 10
$payloadJson | gh api \
  -X PUT \
  -H "Accept: application/vnd.github+json" \
  repos/qqrm/rust-switcher/branches/dev/protection \
  --input -

# main (same payload)
$payloadJson | gh api \
  -X PUT \
  -H "Accept: application/vnd.github+json" \
  repos/qqrm/rust-switcher/branches/main/protection \
  --input -
```

## PR flow

```powershell
# from feature branch
git push -u origin codex/my-change
gh pr create --base dev --fill
```
