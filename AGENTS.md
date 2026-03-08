# Agent Instructions Bootstrap

This repository consumes baseline instructions via live fetch from Codex Tools GitHub Pages.

## Optional Live Fetch Step
For tasks that need the freshest shared baseline instructions (especially remote or ephemeral agent runs), fetch updates first:

```bash
curl -fsSL "https://qqrm.github.io/codex-tools/scripts/PretaskInitialization.sh" \
  | AGENTS_DEST=".codex/AGENTS.remote.md" bash -s --
```

Local development in a stable environment may skip this step.
If the download fails, continue with local repository instructions and explicitly report the fetch error in the final summary.

## Instruction Load Order
1. If `.codex/AGENTS.remote.md` exists, read it first.
2. Apply repository overrides from `REPO_AGENTS.md` when the file exists.
3. Apply additional scoped `AGENTS.md` files in subdirectories when present.

## Operational Notes
- Live fetch is recommended for new remote tasks, but optional for local development.
- Do not commit `.codex/AGENTS.remote.md`.