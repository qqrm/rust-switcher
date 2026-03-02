# Release process (official)

This repository uses a one-command, production-ready release flow driven by `just` + PowerShell scripts.
The primary branch is `dev`.

## One-command release

Patch bump (recommended default):

```powershell
just release
```

Exact version:

```powershell
just release 1.2.3
```

## What `just release` does

1. Safety checks

   * Refuses to run unless the current branch is `dev`.
   * Refuses to run if the worktree is dirty.
   * Fetches tags from `origin` to enforce tag immutability.

2. Quality gates

   * `cargo fmt --all -- --check`
   * `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
   * `cargo test --workspace --all-features --all-targets --locked`

3. Version bump (single commit)

   * Updates versions consistently across workspace crates:

     * `rust-switcher`
     * `rust-switcher-core`
   * Updates the dependency version in the root `Cargo.toml`.
   * Regenerates `Cargo.lock`.
   * Creates a single commit: `chore: bump version to X.Y.Z`.

4. Push

   * Normal push only: `git push origin dev`.

5. Immutable tag

   * Creates `vX.Y.Z` at `HEAD`.
   * If the tag already exists:

     * If it points to `HEAD`: continues.
     * If it points elsewhere: fails (the tag is never moved).
   * Pushes the tag: `git push origin vX.Y.Z`.

6. GitHub Release + Windows artifacts

   * Builds: `cargo build --release`.
   * Uploads:

     * `target/release/rust-switcher.exe` (required)
     * `target/release/rust-switcher.pdb` (optional, if present)
   * Creates or edits the GitHub Release for `vX.Y.Z` and uploads assets with `--clobber`.

     * Note: `--clobber` replaces assets **for the same tag only**; the tag itself is never moved.

7. crates.io publish (idempotent)

   * Publishes in dependency order:

     1. `cargo publish -p rust-switcher-core --locked`
     2. `cargo publish -p rust-switcher --locked`
   * If a version is already published, it is treated as success.

## Safety rules

* No branch history rewrites (no hard resets to remote refs, no force-push). Only normal pushes of commits and tags.
* `dev` is the primary branch; release scripts must not assume any other primary branch.
* Tags are immutable:

  * If `vX.Y.Z` exists and is not `HEAD`, the release flow fails.
* The release flow is designed to be safe to re-run:

  * Existing tags/releases are reused if they match `HEAD`.
  * crates.io publish treats "already published" as success.

## Supporting commands

* Version bump only:

```powershell
just bump
just bump 1.2.3
```

* GitHub Release upload only (requires existing `vX.Y.Z` tag at `HEAD`):

```powershell
just github-release
just github-release 1.2.3
```

* crates.io publish only:

```powershell
just crates-release
```
