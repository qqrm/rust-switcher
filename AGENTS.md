# Agent Instructions Bootstrap

This repository uses local instruction files by default.

## Instruction Load Order
1. If `.codex/AGENTS.remote.md` is already present (provisioned during fresh Codex web-agent environment setup), read it first.
2. Apply repository overrides from `REPO_AGENTS.md` when the file exists.
3. Apply additional scoped `AGENTS.md` files in subdirectories when present.

## Initialization Policy
- Do not run pre-task live fetch in normal local development.
- Use codex-tools initialization scripts only when bootstrapping a fresh Codex web-agent environment from scratch.
- Do not commit `.codex/AGENTS.remote.md`.