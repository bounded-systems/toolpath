# toolpath-pi

Read [Pi](https://pi.dev/) coding-agent session logs and derive Toolpath
provenance documents.

Pi stores sessions as JSONL files at
`~/.pi/agent/sessions/--<encoded-cwd>--/<timestamp>_<uuid>.jsonl`. Each
entry forms a tree via `id`/`parentId` fields, enabling in-place branching.
Sessions can link to parent sessions via a `parentSession` file path in
the session header.

This crate implements the [`toolpath_convo::ConversationProvider`] trait
and a `derive_path` wrapper that produces a [`toolpath::v1::Path`] via
the shared derivation in [`toolpath_convo::derive_path`].

## Quick example

```rust,no_run
use toolpath_pi::PiConvo;

let manager = PiConvo::new();
let session = manager
    .most_recent_session("/Users/alex/project")
    .unwrap()
    .expect("no Pi sessions");
let view = manager.to_view(&session);
```
