---
description: Share an agent session to Pathbase and get a link
argument-hint: "[session hint] [--anon] [--public] [--harness <name>]"
allowed-tools: Bash(${CLAUDE_PLUGIN_ROOT}/scripts/ensure-path.sh:*)
---

## Context

- Toolpath CLI: !`"${CLAUDE_PLUGIN_ROOT}/scripts/ensure-path.sh"`
- Auth: !`"${CLAUDE_PLUGIN_ROOT}/scripts/ensure-path.sh" exec auth status`
- Claude Code sessions for this project, newest first: !`"${CLAUDE_PLUGIN_ROOT}/scripts/ensure-path.sh" sessions`

## Your task

Share an agent session to Pathbase with the Toolpath CLI and report the resulting URL.

User arguments: $ARGUMENTS

Always invoke the CLI through the wrapper (it resolves or installs the binary regardless of PATH):

```
"${CLAUDE_PLUGIN_ROOT}/scripts/ensure-path.sh" exec <path-cli arguments...>
```

In every command you run, write paths as literal absolute strings — never `$PWD` or other variables (they fail the permission check), and never relative paths like `.` (the CLI matches them against nothing).

### Choose the session

The session list above is TSV: project, session id, timestamp, step count, first user message.

- **No arguments**: share the current conversation. The list is newest-first and includes this live session, so take the first row's session id.
- **A textual hint**: match it against the first-user-message and session-id columns. If no row matches confidently, show the closest candidates and ask which one.
- **`--harness <name>`** (claude / gemini / codex / copilot / opencode / cursor / pi): list that harness instead with `exec p list <name> --format tsv`. For claude/gemini/pi add `--project <absolute cwd>` and read the session id from column 2; for the others the session id is column 1.

If the list above is empty, run `exec p list claude --format tsv` without `--project` and match rows by the project column — its decoded paths are lossy (`.`, `_`, and `/` all display as `/`), so compare loosely against the cwd.

### Check auth

If the Auth context shows no valid login and the user did not pass `--anon`, stop and ask: upload anonymously (`--anon` — unlisted, addressable only by UUID), or log in first? Logging in means the user runs `path auth login` in their own terminal (it needs an interactive code paste — never run it yourself), then re-runs `/path:share`.

### Upload

```
"${CLAUDE_PLUGIN_ROOT}/scripts/ensure-path.sh" exec share --harness <harness> --session <session-id> [flags]
```

- Add `--project <absolute cwd>` for claude/gemini/pi session ids; omit it for codex/opencode/cursor/copilot.
- Pass through any of `--anon`, `--public`, `--repo <owner/name>`, `--name <label>`, `--url <server>` from the user's arguments.
- Never run `share` without `--session` — the interactive picker cannot run here.

### Report

Give the user the Pathbase URL from the output. On failure show the error and the likely fix: log in or use `--anon` for auth errors, or upgrade the CLI (`curl --proto '=https' --tlsv1.2 -fsS https://toolpath.net/install.sh | bash`) if the installed version predates `share`.
