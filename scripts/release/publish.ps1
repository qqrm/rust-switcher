$ErrorActionPreference = 'Stop'

throw "scripts/release/publish.ps1 is deprecated. Use 'just release' (scripts/release/release.ps1) for the direct tag + GitHub Release + crates.io publish flow on the 'dev' branch."
