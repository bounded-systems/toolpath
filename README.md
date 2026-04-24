# Toolpath

A tool-agnostic format for tracking artifact transformation provenance.

## What is this?

Toolpath records the complete history of how code (and other artifacts) evolved:

- **Who** made changes (humans, AI agents, formatters, linters, CI)
- **What** they changed (unified diffs + structural AST operations)
- **Why** they changed it (intent, linked issues, reasoning)
- **What else they tried** (dead ends preserved for reflection)
- **Verification** (cryptographic signatures, identity resolution)

## Three core objects

| Object    | What it represents                        | Example                |
|-----------|-------------------------------------------|------------------------|
| **Step**  | A single change to artifact(s)            | One commit, one edit   |
| **Path**  | A sequence of steps with a base context   | A PR, a coding session |
| **Graph** | A collection of related paths             | A release              |

Steps form a DAG via parent references. Dead ends are implicit: steps not in the ancestry of `path.head`.

```
              +-- step-3a -- step-4a  (dead end)
step-1 -- step-2 --+
              +-- step-3b -- step-4b -- step-5b  (head)
```

## Install

```bash
cargo install toolpath-cli
```

This installs a binary called `path`.

## Workspace

```
crates/
  toolpath/           Core types, builders, query API
  toolpath-convo/     Provider-agnostic conversation types, traits, and Toolpath-Path derivation
  toolpath-git/       Derive from git repository history
  toolpath-github/    Derive from GitHub pull requests
  toolpath-claude/    Derive from Claude conversation logs
  toolpath-gemini/    Derive from Gemini CLI conversation logs
  toolpath-codex/     Derive from Codex CLI rollout files
  toolpath-opencode/  Derive from opencode SQLite databases
  toolpath-pi/        Derive from Pi (pi.dev) agent sessions
  toolpath-dot/       Graphviz DOT visualization
  toolpath-md/        Markdown rendering for LLM consumption
  toolpath-cli/       Unified CLI (binary: path)
```

See each crate's README for library-level documentation.

## Quick start

```bash
# Build everything
cargo build --workspace

# Import a Toolpath document from this repo's git history (cached under ~/.toolpath/documents/)
path import git --repo . --branch main

# Visualize it
path import git --repo . --branch main --no-cache | path render dot | dot -Tpng -o graph.png

# Render as Markdown for an LLM
path import git --repo . --branch main --no-cache | path render md

# Import from a GitHub pull request
path import github https://github.com/owner/repo/pull/42

# Import from Claude conversation logs
path import claude --project /path/to/project

# Import from Gemini CLI conversation logs
path import gemini --project /path/to/project

# Import from Codex CLI rollout files (most recent session by default)
path import codex

# Import from opencode session database (most recent session by default)
path import opencode

# List what's in the cache
path cache ls

# Export a cached document back into a Claude Code session
path export claude --input claude-<session-id> --project /path/to/resume

# Push a cached document to Pathbase
path auth login
path export pathbase --input claude-<session-id>

# Pull a trace from Pathbase back into the local cache
path import pathbase <trace-id-or-url>

# Query for dead ends (abandoned approaches)
path query dead-ends --input doc.json

# Filter steps by actor
path query filter --input doc.json --actor "agent:"

# Walk the ancestry of a step
path query ancestors --input doc.json --step-id step-003

# Merge multiple documents into a graph
path merge doc1.json doc2.json --title "Release v2" --pretty

# Validate a document
path validate --input examples/step-01-minimal.json
```

## CLI reference

```
path
  list
    git       [--repo PATH] [--remote NAME] [--json]
    github    --repo OWNER/REPO [--json]
    claude    [--project PATH] [--json]
    gemini    [--project PATH] [--json]
    codex     [--json]
    opencode  [--project ID] [--json]
  derive
    git       --repo PATH --branch NAME[:START] [--base COMMIT] [--remote NAME] [--title TEXT]
    github    --repo OWNER/REPO --pr NUMBER [--no-ci] [--no-comments]
    claude    --project PATH [--session ID] [--all]
    gemini    --project PATH [--session UUID] [--all] [--include-thinking]
    codex     [--session UUID|STEM] [--all]
    opencode  [--session ID] [--all] [--project ID] [--no-snapshot-diffs]
  query
    ancestors --input FILE --step-id ID
    dead-ends --input FILE
    filter    --input FILE [--actor PREFIX] [--artifact PATH] [--after TIME] [--before TIME]
  render
    dot       [--input FILE] [--output FILE] [--show-files] [--show-timestamps]
    md        [--input FILE] [--output FILE] [--detail summary|full] [--front-matter]
  merge       FILE... [--title TEXT]
  track
    init      --file PATH --actor ACTOR [--title TEXT] [--base-uri URI] [--base-ref REF]
    step      --session FILE --seq N [--actor ACTOR] [--intent TEXT]
    visit     --session FILE --seq N
    note      --session FILE --intent TEXT
    export    --session FILE
    close     --session FILE
    list
  validate    --input FILE
  haiku
```

Global: `--pretty` for formatted JSON output.

## Using the libraries

### Core types

```rust
use toolpath::{Step, Path, Base, Document};

let step = Step::new("step-001", "human:alex", "2026-01-29T10:00:00Z")
    .with_parent("step-000")
    .with_raw_change("src/main.rs", "@@ -1,1 +1,1 @@\n-hello\n+world")
    .with_intent("Fix greeting");

let path = Path::new(
    "path-pr-42",
    Some(Base::vcs("github:org/repo", "abc123")),
    "step-001",
);
```

### Query operations

```rust
use toolpath::query;

let ancestors = query::ancestors(&path.steps, &path.path.head);
let dead_ends = query::dead_ends(&path.steps, &path.path.head);
let by_actor = query::filter_by_actor(&path.steps, "agent:");
let artifacts = query::all_artifacts(&path.steps);
```

### Git derivation

```rust
use toolpath_git::{derive, DeriveConfig};

let repo = git2::Repository::open(".")?;
let config = DeriveConfig { remote: "origin".into(), title: None, base: None };
let doc = derive(&repo, &["main".into()], &config)?;
```

### DOT rendering

```rust
use toolpath_dot::{render, RenderOptions};

let dot_string = render(&doc, &RenderOptions::default());
```

### Markdown rendering

```rust
use toolpath_md::{render, RenderOptions};

let md_string = render(&doc, &RenderOptions::default());
```

## Documentation

- [RFC.md](RFC.md) -- Full format specification
- [FAQ.md](FAQ.md) -- Design rationale and FAQ
- [CHANGELOG.md](CHANGELOG.md) -- Release history
- [schema/toolpath.schema.json](schema/toolpath.schema.json) -- JSON Schema
- [examples/](examples/) -- 11 example documents covering steps, paths, and graphs
- [docs/agents/formats/](docs/agents/formats/README.md) -- Reference for the on-disk
  formats emitted by agents we derive from (Claude Code today; more as they land)

## Requirements

Rust 1.85+ (edition 2024).

## License

Apache-2.0
