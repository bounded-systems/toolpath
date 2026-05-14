---
layout: base.njk
title: "Kind: agent-coding-session v1.0.0"
permalink: /kinds/agent-coding-session/v1.0.0/
---

# Kind: `agent-coding-session` v1.0.0

<dl class="kind-meta">
  <dt>URI</dt>
  <dd><code>https://toolpath.dev/kinds/agent-coding-session/v1.0.0</code></dd>
  <dt>Schema</dt>
  <dd><a href="./schema.json"><code>schema.json</code></a></dd>
  <dt>Status</dt>
  <dd>Stable. This URI is immutable; subsequent revisions ship at a new version URI.</dd>
</dl>

A Toolpath path whose `meta.kind` is this URI records an AI coding conversation. It is an ordinary Toolpath path — `head`-ancestry, dead ends, signatures, and `base` all work as in the [base format](/format/) — with the additional structure below.

## The turn payload

A step that represents a conversational turn has one entry in its `change` map that is an `ArtifactChange` whose `structural.type` is `"conversation.append"`. **Locate it by that `type`, not by artifact key** — the key is producer-specific (`agent://claude/<session-id>`, `gemini://<session-id>`, `codex://<session-id>`, `opencode://<session-id>`, or `<provider>://<conversation-id>`).

That change's `structural` object always carries:

| Field  | Type   | Meaning                                                            |
| ------ | ------ | ------------------------------------------------------------------ |
| `type` | string | the literal `"conversation.append"`                                |
| `role` | string | `"user"`, `"assistant"`, `"system"`, or a producer-specific string |

and, when the turn has prose, also carries `text` (string; treat a missing `text` as empty). It may additionally carry any of the following — all optional and producer-dependent, so check before using each:

| Field | Meaning |
| ----- | ------- |
| `thinking` | the model's reasoning text for this turn |
| `tool_uses` | the tools the agent invoked this turn; element shape is producer-specific — a bare tool-name string, or an object like `{ "id", "name", "input", "category", "result"? }` |
| `token_usage`, or `input_tokens` / `output_tokens` / `cache_read_tokens` / `cache_write_tokens` | token accounting for the turn |
| `stop_reason` | why the model stopped (`end_turn`, `tool_use`, …) |
| `environment`, or `cwd` / `git_branch` / `version` / `user_type` | the session environment at this turn |
| `delegations` | sub-agent work spawned from this turn |
| `model` | the model that produced an assistant turn |
| `turn_extra` / `entry_extra` / `<provider>` | a producer-namespaced bag of everything else, keyed by the producer's short name (`"claude"`, `"gemini"`, …) |

## Actors

`step.actor` follows the usual `type:name` convention:

| Actor pattern | Turn |
| ------------- | ---- |
| `human:user` | a user message |
| `agent:<model>` | a model reply, named by model when the source records one (e.g. `agent:gpt-5.4`) |
| `agent:<harness>` | a model reply when the model is not recorded (e.g. `agent:claude-code`) |
| `system:<harness>` or `tool:<harness>` | a synthetic entry — session init, system prompt, environment note |
| `agent:<harness>/tool:<tool-name>` | a tool execution that the producer broke out as its own step (e.g. `agent:claude-code/tool:Write`) |

Walk steps in `head`-ancestry order for the linear transcript.

## Path metadata

`meta.source` names the producing harness: `claude-code`, `gemini-cli`, `codex`, `opencode`, or `pi`. `meta.actors` defines the actors the steps reference. `meta.extra` may carry a producer-namespaced aggregate (e.g. `meta.extra.codex.files_changed`).

## What's producer-specific

Anything not described above. Some producers add synthetic steps (a session-init step, system-message steps) with their own `structural.type` rather than `conversation.append`; some attach tool calls as additional `change` entries on the turn's step (keyed by file path, with a producer-specific `structural.type` such as `file.write`, `codex.update`, or `gemini.write_file`, and often a unified diff in `raw`), while others record a tool execution as a separate child step. Treat anything that is not a `conversation.append` change as an ordinary Toolpath change — `meta.source` tells you whose conventions to expect.

## Producers

`agent-coding-session` paths are produced by every conversation provider crate (`toolpath-claude`, `toolpath-gemini`, `toolpath-codex`, `toolpath-opencode`) and by the shared `ConversationView → Path` derivation in `toolpath-convo` (which `toolpath-pi` uses).
