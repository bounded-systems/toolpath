# toolpath-derive

Shared derivation layer that converts a [`toolpath-convo`] `ConversationView`
into a [`toolpath`] `Path` document.

Provider crates (e.g. `toolpath-pi`) build a `ConversationView` from their
source-specific format, then call `toolpath_derive::derive_path` to produce a
standard Toolpath `Path`. This keeps the mapping from conversation to
provenance in one place so every provider produces consistent output.

## Mapping contract

| `ConversationView` concept | Toolpath output |
|---|---|
| `view.id`, `view.provider_id` | `path.id`, `meta.source` |
| First turn's `environment.working_dir` | `path.base.uri` (`file:///…`) |
| `Turn` with role=User/Assistant/System | `Step` with `actor: human:user` / `agent:<model>` / `system:<provider>` |
| `Turn.parent_id` | step parent reference |
| `Turn.text` | structural change `conversation.append` on `<provider>://<session-id>` |
| `Turn.thinking` (when enabled) | step extra `thinking` |
| `Turn.tool_uses` (when enabled) | step extra `tool_uses` |
| Tool invocations with `category=FileWrite` | `change` entry keyed by extracted file path |
| `Turn.token_usage` | step extra `token_usage` |
| `Turn.delegations` | step extra `delegations` |
| `view.files_changed` | `meta.files_changed` |
| Actors seen | `meta.actors` |

File-path extraction for `FileWrite` tool invocations looks for a fixed set
of common field names in the tool input JSON: `file_path`, `path`,
`filename`, `file`. If none match, the invocation is preserved in step
extras but produces no artifact change.

## Quick example

```rust,no_run
use toolpath_convo::ConversationView;
use toolpath_derive::{derive_path, DeriveConfig};

fn run(view: &ConversationView) {
    let path = derive_path(view, &DeriveConfig::default());
    println!("{}", serde_json::to_string_pretty(&path).unwrap());
}
```
