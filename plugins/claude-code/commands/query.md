---
description: Query your local agent-session history with plain English or a jaq filter
argument-hint: "<question or jaq filter> [--source claude|codex|...]"
allowed-tools: Bash(${CLAUDE_PLUGIN_ROOT}/scripts/ensure-path.sh:*)
---

## Context

- Toolpath CLI: !`"${CLAUDE_PLUGIN_ROOT}/scripts/ensure-path.sh"`

## Your task

Answer the user's question by querying the local Toolpath cache — the on-disk archive of their agent coding sessions (Claude Code, Gemini CLI, Codex, Copilot, opencode, Cursor, Pi) and derived git/GitHub history. Translate plain English into a jaq (jq-compatible) filter; run a filter verbatim if the user already wrote one.

User arguments: $ARGUMENTS

Always invoke the CLI through the wrapper (it resolves or installs the binary regardless of PATH):

```
"${CLAUDE_PLUGIN_ROOT}/scripts/ensure-path.sh" exec query [--source <s>] [--project <dir>] [-r|-c] '<filter>'
```

### Data model

`query` flattens every step of every cached document into one JSON array; the filter runs over that array. Each element wraps one step:

```json
{
  "cache_id": "claude-path-claude-code-6987afe8",
  "step":     { "id": "...", "actor": "agent:claude-opus-5", "timestamp": "2026-07-29T15:44:20.239Z" },
  "change":   [ { "artifact": null, "structural": { "type": "conversation.append", "role": "user", "text": "..." } } ],
  "dead_end": false,
  "path":     { "id": "...", "base": { }, "meta": { "source": "claude-code" } }
}
```

- `step.actor` is `human:<name>`, `agent:<model>` (e.g. `agent:claude-opus-5`, `agent:gpt-5.2-codex`), or `tool:<harness>` (e.g. `tool:claude-code`); to filter by harness use `path.meta.source` or `--source`, not the actor.
- `change[].structural.type` for agent sessions is one of `conversation.append` (role, text, thinking, tool_uses, token_usage, ...), `conversation.event`, `conversation.compact`, or `file.write`; git/GitHub-derived steps carry file diffs instead.
- `dead_end` marks steps not on the ancestry of the path head (abandoned work).
- For the full field reference run `exec kind agent-coding-session`.

### Scoping and freshness

- `--source claude|gemini|codex|copilot|opencode|cursor|pi|git|github` narrows by harness, `--project <dir>` by project, `--kind <selector>` by path kind, `--id <cache-id>` by document; `--input <file>` queries a file without touching the cache.
- `-r` prints raw strings (like `jq -r`); `-c` forces compact output.
- path-cli 0.16+ auto-syncs the queried scope from the installed harnesses before running. On older versions, if results look empty or stale, fill the cache first with `exec p cache sync` (0.16+) or `exec p import claude --project <absolute cwd> --force`, and inspect it with `exec p cache ls`. Always write `--project` as a literal absolute path — `$PWD` fails the permission check, and relative paths match nothing.

### Example filters

```bash
# ids of abandoned (dead-end) steps
'map(select(.dead_end)) | map(.step.id)'

# sessions where a user prompt mentions "tailscale"
'[.[] | select(any(.change[]?.structural;
    .type == "conversation.append" and .role == "user"
    and ((.text // "") | test("tailscale"; "i"))))
  | .cache_id] | unique'

# steps that burned >50k input tokens in one message
'map(select(any(.change[]?.structural.token_usage; .input_tokens > 50000)))'

# step count per source document, largest first
'group_by(.cache_id) | map({id: .[0].cache_id, steps: length}) | sort_by(-.steps)'

# agent (vs. human) steps this month
'map(select((.step.actor | startswith("agent:")) and .step.timestamp > "2026-07")) | length'
```

### Report

Interpret the JSON for the user — answer the question in prose, quoting the relevant ids/values, rather than dumping raw output. Show the filter you ran so they can refine it. If a filter errors, fix it and retry rather than reporting the syntax error.
