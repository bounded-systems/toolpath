# toolpath-desktop

Desktop GUI for selecting, previewing, and exporting Toolpath traces.

This crate ships a [Tauri 2](https://tauri.app) application that provides a
non-technical-user entry point to the Toolpath ecosystem. It is the companion
app to **Pathbase** — the GitHub-like service for agent traces — and is
intended for people who won't open a terminal to run the `path` CLI.

## What it does

1. **Discover** — enumerate the user's local agent traces:
   - Claude Code sessions under `~/.claude/projects/`
   - Local git branches
   - GitHub pull requests (by URL)
2. **Preview** — render the derived Toolpath `Document` as an interactive DAG
   so the user can see what's in the trace before sharing it.
3. **Export** — save the document as a local `.path.json` file, **or** upload
   it to Pathbase. The Pathbase upload is stubbed in v0.1 and logs a mock
   response; the real API will be wired up in a follow-up.

## Architecture

- **Rust backend** (`src/`) — Tauri commands that link `toolpath`,
  `toolpath-claude`, `toolpath-git`, and `toolpath-github` directly
  in-process. No subprocess, no Wasm.
- **Frontend** (`frontend/`) — Svelte 5 + TypeScript + Vite, built
  with `bun`. Elm-architecture (TEA) shape: `src/lib/types.ts`,
  `update.ts`, and `store.svelte.ts` hold `Model`/`Msg`/`Cmd` and
  the reducer; `src/routes/*.svelte` are pure views over the
  reactive store. Event subscriptions use Svelte `$effect` for
  auto-cleanup. DAG rendering via `src/lib/viz.ts` (dagre-d3-es).

## Run in development

```bash
cargo install tauri-cli --locked --version "^2"  # once
cd crates/toolpath-desktop
cd frontend && bun install && cd ..              # once
cargo tauri dev
```

`cargo tauri dev` auto-starts the Vite dev server on
`http://localhost:1420` (via `beforeDevCommand`) and wires Tauri's
webview to it, so frontend edits hot-reload without restarting Rust.

## Build a bundle

```bash
cd crates/toolpath-desktop
cargo tauri build   # runs `bun run build` first, then bundles
```

Artifacts land under `target/release/bundle/`.

## Scope

In scope for v0.1: source discovery, trace preview, local export, stubbed
Pathbase upload.

Out of scope: real Pathbase API integration, live/track mode, query UI,
editing documents, multi-doc Graph merging, identity signing.
