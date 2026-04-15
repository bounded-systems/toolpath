# RFC: JSONL Streaming Format for Toolpath

**Status:** Draft
**Authors:** with Alex Kesling <alex@empathic.dev>
**Created:** 2026-04-14
**Extends:** [RFC: Toolpath - A Format for Artifact Transformation Provenance](../RFC.md)

## Abstract

This RFC defines a line-oriented JSONL format as a peer to Toolpath's canonical
JSON format, optimized for incremental persistence of `Path` documents. The
format expresses a `Path` as a stream of self-describing lines — each an
instruction that contributes to the final document — so that writers can append
one complete step at a time rather than buffering the entire path before
serializing.

The JSONL format and the canonical JSON format round-trip losslessly in both
directions. Every field reachable in the canonical JSON is reachable through
the JSONL line set, and signatures computed over canonical JSON remain valid
across any number of `stream ↔ seal` cycles.

One additive schema change is required: an optional `graph_ref` field on
`PathIdentity` so a streamed path can name the graph it belongs to up front.

## Motivation

### The Problem

The canonical Toolpath format serializes a `Path` as a single JSON document.
Producers must buffer the entire path before emitting a valid document. This
is a poor fit for two recurring use cases:

1. **Live agent traces.** A single writer (e.g., Claude Code) records steps
   as it works, and a consumer tails the file to display progress. With the
   canonical format the writer must either defer writing until the session
   completes, or repeatedly rewrite a growing JSON blob — neither of which
   supports a readable tailed file.

2. **Multi-writer append logs.** Heterogeneous producers — CI, IDE,
   formatter, git hook — all record changes to the same path. Each producer
   emits one or more complete steps; no single producer has global knowledge
   of the path. Producers cannot coordinate to write a single JSON document.

Both cases want an append-only, line-oriented file where each line is a
self-describing unit that incrementally constructs a `Path` document.

### Goals

1. **Append-only writes.** Writers emit complete lines; no line ever needs
   to be rewritten or removed.
2. **Peer format with canonical JSON.** Bidirectional, lossless round-trip.
3. **Complete expressive power.** Every field reachable in canonical JSON is
   reachable through the line set.
4. **Signature preservation.** Signatures computed over canonical JSON
   survive any number of `stream → file → seal` cycles.
5. **Strict, unambiguous parsing.** Malformed input is a fatal error. No
   recovery heuristics.

### Non-Goals

1. **Streaming individual steps.** Each step is atomic per line. Partial
   step construction (e.g., emitting a step identity now and its diff
   later) is out of scope.
2. **Graph-scoped streams.** A JSONL file contains exactly one `Path`.
   Graphs compose streaming and non-streaming path files via existing
   `$ref` semantics.
3. **Mid-file resync or corruption recovery.** Readers always start from
   line 1.
4. **Transport semantics.** The format is equally usable as a storage
   format, a tailed log, or a transport payload, but no transport is
   specified.

## Scope

v1 covers the `Path` document type. `Step` documents are already a single
JSON blob and do not benefit from streaming. `Graph` documents are a
container of path references; a streaming graph is a graph that references
streaming path files.

## File Structure

### Extension and Encoding

| Property | Value |
| -------- | ----- |
| Extension | `.toolpath.jsonl` |
| Encoding | UTF-8 |
| Line terminator | LF (`\n`) |
| Line format | One JSON object per line |

No blank lines, no comments, no trailing commas. Each line is a single
externally tagged JSON object, mirroring the canonical `Document` envelope:

```
{"<Variant>": <body>}
```

Valid variants: `PathOpen`, `Step`, `ActorDef`, `Signature`, `PathMeta`,
`Head`, `PathClose`.

### Order Constraints

| Constraint | |
| ---------- | --- |
| `PathOpen` | Exactly once, as line 1. |
| `PathClose` | At most once, as the last line (if present). |
| `Head` | Zero or more; last occurrence wins. |
| `Step`, `ActorDef`, `Signature`, `PathMeta` | Zero or more, any order, between `PathOpen` and `PathClose`. |
| `Signature` with `target: "step:<id>"` | Must appear after the referenced `Step` line. |

### Parsing Strictness

Readers MUST treat the following as fatal errors:

- First line is not a valid `PathOpen`.
- Malformed JSON on any line.
- Unknown variant at the top level of a line.
- Unknown `version` value in `PathOpen`.
- `Signature` targeting a step that has not yet appeared.
- Ambiguous head at EOF when no `Head` line was emitted (see *Sealing*).

Forward compatibility is handled by bumping the format version in
`PathOpen`, not by tolerating unknown line kinds. Older readers reject
newer files deterministically.

### Durability

Writer durability is out of band. The format does not mandate an `fsync`
policy. Implementations typically sit somewhere on this spectrum:

| Policy | Tradeoff |
| ------ | -------- |
| No sync (OS-buffered) | Fastest. Unflushed lines lost on crash. Appropriate for ephemeral traces. |
| `fsync` per line | Strongest durability. Highest overhead. Appropriate for multi-writer append logs where each line represents a committed fact. |
| `fsync` per batch / on close | Compromise. Suits single-writer agent traces that emit many steps in close succession. |

POSIX append semantics guarantee atomic concatenation only for writes
less than or equal to `PIPE_BUF` (commonly 4096 bytes) when multiple
writers share a file. Larger lines require external locking or a
single-writer discipline.

## Line Kinds

Each line body is a JSON object with the fields below. Fields marked
optional may be omitted.

### `PathOpen`

Exactly once, as the first line. Carries the initial path identity and any
path-level metadata known at open time.

```json
{"PathOpen": {
  "version": "1",
  "id": "pr-42",
  "base": {"uri": "github:org/repo", "ref": "abc123"},
  "graph_ref": "toolpath://archive/release-v2",
  "meta": {
    "title": "Add email validation",
    "source": "github:myorg/myrepo/pull/42",
    "intent": "...",
    "refs": [{"rel": "fixes", "href": "issue://..."}]
  }
}}
```

| Field | Required | Description |
| ----- | -------- | ----------- |
| `version` | yes | Format version string. v1 = `"1"`. |
| `id` | yes | `PathIdentity.id`. |
| `base` | no | `PathIdentity.base` — same shape as canonical JSON. |
| `graph_ref` | no | `$ref`-style URL naming a graph this path belongs to. See *Schema Change*. |
| `meta` | no | Initial `PathMeta` excluding `actors` and `signatures` (those have dedicated line kinds). |

### `Step`

Zero or more. The body is the existing `Step` JSON shape verbatim — including
`step.meta`, which may carry step-level signatures, intent, refs, or actor
definitions.

```json
{"Step": {
  "step": {
    "id": "step-003",
    "parents": ["step-002"],
    "actor": "agent:claude-code",
    "timestamp": "2026-04-14T15:30:00Z"
  },
  "change": {
    "src/auth/validator.rs": {"raw": "@@ -1,5 +1,25 @@\n..."}
  },
  "meta": {
    "intent": "Add email validation"
  }
}}
```

### `ActorDef`

Zero or more. Inserts or overwrites an entry in `path.meta.actors`.

```json
{"ActorDef": {
  "actor": "human:alex",
  "definition": {
    "name": "Alex Kesling",
    "identities": [{"system": "github", "id": "akesling"}],
    "keys": [{"type": "gpg", "fingerprint": "ABCD1234..."}]
  }
}}
```

Overwrite (rather than merge) is deliberate: an actor definition is the
complete current record for that actor. A writer that wants to add a key
re-emits the full definition.

### `Signature`

Zero or more. Appends to the signature array at either path or step scope.

```json
{"Signature": {
  "target": "path",
  "signature": {
    "signer": "human:bob",
    "key": "gpg:EFGH5678",
    "scope": "reviewer",
    "sig": "-----BEGIN PGP SIGNATURE-----\n...",
    "timestamp": "2026-04-14T16:00:00Z"
  }
}}
```

| `target` | Effect |
| -------- | ------ |
| `"path"` | Append to `path.meta.signatures`. |
| `"step:<step-id>"` | Append to the named step's `meta.signatures`. Referenced `Step` line MUST already have appeared. |

### `PathMeta`

Zero or more. Field-wise merge into `path.meta`. Last-write-wins per field.
Array fields (e.g. `refs`) **replace**; they do not append.

```json
{"PathMeta": {
  "patch": {
    "title": "Revised title",
    "intent": "...",
    "refs": [{"rel": "tracks", "href": "..."}],
    "extra": {"custom-field": "..."}
  }
}}
```

Append-like semantics for `actors` and `signatures` are handled by the
dedicated `ActorDef` and `Signature` line kinds; `PathMeta` does not carry
those fields.

### `Head`

Zero or more. Explicitly names the current head step. Last occurrence wins.

```json
{"Head": {"step_id": "step-005"}}
```

### `PathClose`

Optional terminator. Indicates the writer is done and no more lines will
follow. Readers MAY use this to distinguish a completed file from one still
being appended to. Readers MUST tolerate its absence.

```json
{"PathClose": {}}
```

## Sealing Algorithm (JSONL → JSON)

Produces a canonical `{"Path": {...}}` document from a JSONL stream.

```
state:
  path_id, base, graph_ref     ← from PathOpen
  path_meta                    ← PathOpen.meta or empty; actors={}, signatures=[]
  steps = []                   (in arrival order)
  step_index = {}              (step.id → &steps[i])
  head = null                  (last Head line's step_id, if any)

for each line after PathOpen, in order:
  Step:
    append to steps; record in step_index
  ActorDef { actor, definition }:
    path_meta.actors[actor] = definition
  Signature { target: "path", signature }:
    path_meta.signatures.append(signature)
  Signature { target: "step:<id>", signature }:
    require step_index[id] exists (else fatal error)
    step_index[id].meta.signatures.append(signature)
  PathMeta { patch }:
    for each field in patch:
      path_meta[field] = patch[field]     # arrays replace
  Head { step_id }:
    head = step_id
  PathClose:
    end-of-stream sentinel; no state change

on EOF:
  if head is null:
    candidates = { s in steps : no other s' in steps has s.id in s'.parents }
    if |candidates| != 1: fatal error ("ambiguous or missing head")
    head = the one candidate's id
  emit: {"Path": {
    "path": {
      "id": path_id,
      "base": base,
      "graph_ref": graph_ref,
      "head": head
    },
    "steps": steps,
    "meta": path_meta
  }}
```

The single-tip DAG rule applies only when `head` is unspecified. Paths with
intentional dead ends SHOULD emit an explicit `Head` line.

## Streaming Algorithm (JSON → JSONL)

Produces a deterministic line sequence from a canonical `Path`. Used for
round-trip testing and for re-streaming stored documents.

```
emit PathOpen {
  version: "1",
  id: path.id,
  base: path.base,
  graph_ref: path.graph_ref,
  meta: {                        # path.meta minus actors, signatures
    title, source, intent, refs, extra
  }
}

for actor_key in sorted(path.meta.actors.keys()):
  emit ActorDef { actor: actor_key, definition: path.meta.actors[actor_key] }

for step in path.steps:           # original array order
  emit Step { ...step... }
  for sig in step.meta.signatures:  # original order
    emit Signature { target: "step:" + step.id, signature: sig }

for sig in path.meta.signatures:  # original order
  emit Signature { target: "path", signature: sig }

emit Head { step_id: path.head }
emit PathClose {}
```

## Round-Trip Guarantees

- **Seal after stream:** `seal(stream(P)) == P` field-for-field for any
  canonical `Path` `P`.
- **Stream after seal:** `stream(seal(L))` may differ from `L` in line
  ordering — `stream` emits a normalized order — but sealed documents are
  equal: `seal(stream(seal(L))) == seal(L)`.
- **Idempotent normalization:** `stream(seal(stream(P))) == stream(P)`
  line-for-line.

Round-trip is lossless because every canonical JSON field has a
corresponding line kind:

| Canonical field | Line kind |
| --------------- | --------- |
| `path.id`, `path.base`, `path.graph_ref` | `PathOpen` |
| `path.head` | `Head` (or inferred) |
| `path.meta.title` / `source` / `intent` / `refs` / `extra` | `PathOpen.meta` or `PathMeta` |
| `path.meta.actors` entries | `ActorDef` |
| `path.meta.signatures` entries | `Signature` with `target: "path"` |
| `steps[*]` (entire step including inner `meta`) | `Step` |
| `steps[*].meta.signatures` entries | `Signature` with `target: "step:<id>"` |

## Signatures

Canonicalization is unchanged. Signatures are computed over the canonical
JSON form per JCS (RFC 8785), as defined in the base RFC. Because the
JSONL format is lossless, a signature computed on a sealed JSON document
remains valid across any number of `stream ↔ seal` cycles.

Late-arriving signatures — for example, a reviewer approval added after
the steps are recorded — arrive as `Signature` lines appended to the file.
The signed payload is still the canonical JSON form: the reviewer seals
the current file, computes the signature over the sealed JSON per the
base RFC, and emits a `Signature` line.

## Schema Change

One additive change to the canonical schema, in `crates/toolpath/src/types.rs`:

```rust
pub struct PathIdentity {
    pub id: String,
    pub base: Option<Base>,
    pub head: String,
    pub graph_ref: Option<String>,   // NEW
}
```

Backwards compatible: `graph_ref` is optional and omitted when empty.
Existing documents and signatures validate unchanged. This field lets a
path name the graph it belongs to, supporting cross-referencing from
streaming contexts where the containing graph is known up front.

The value of `graph_ref` uses the same `$ref`-style URL conventions as
`Graph.paths[*].$ref`:

- `https://...`, `s3://...`, `file:///...` — external references
- `toolpath://<archive-name>/<graph-id>` — named archive references

## Example

A minimal streaming session recording a two-step path:

```
{"PathOpen":{"version":"1","id":"pr-42","base":{"uri":"github:org/repo","ref":"abc123"},"meta":{"title":"Add email validation"}}}
{"ActorDef":{"actor":"agent:claude-code","definition":{"name":"Claude Code","provider":"anthropic"}}}
{"Step":{"step":{"id":"step-001","actor":"agent:claude-code","timestamp":"2026-04-14T10:00:00Z"},"change":{"src/validator.rs":{"raw":"@@ -0,0 +1,20 @@\n+pub struct Validator..."}},"meta":{"intent":"Add email validation struct"}}}
{"Step":{"step":{"id":"step-002","parents":["step-001"],"actor":"tool:rustfmt","timestamp":"2026-04-14T10:00:05Z"},"change":{"src/validator.rs":{"raw":"@@ -3,3 +3,3 @@\n..."}},"meta":{"intent":"Auto-format"}}}
{"Head":{"step_id":"step-002"}}
{"Signature":{"target":"path","signature":{"signer":"human:alex","key":"gpg:ABCD1234","scope":"author","sig":"-----BEGIN PGP SIGNATURE-----\n..."}}}
{"PathClose":{}}
```

Sealing this stream produces a canonical `{"Path": {...}}` document with:

- `path.id = "pr-42"`, `path.base = {...}`, `path.head = "step-002"`
- `steps = [step-001, step-002]`
- `path.meta.title = "Add email validation"`
- `path.meta.actors = {"agent:claude-code": {...}}`
- `path.meta.signatures = [<author signature>]`

Streaming that canonical document back produces an equivalent line
sequence (modulo the normalized `stream` ordering).

## Design Rationale

### Why atomic steps instead of open/patch/close?

An earlier sketch considered a `StepOpen` / `StepPatch` / `StepClose`
lifecycle so that a single step could stream in pieces — for example, the
intent becomes known at t=0, the diff at t=20s. This was rejected for v1:

- Atomic steps preserve append-only semantics — no line is ever
  conditional on a later line's arrival.
- Multi-writer append logs (use case B) naturally emit complete steps;
  there is no partial-step use case there.
- Live agent traces (use case A) can emit a step as soon as the agent
  knows both the artifact and the change. The "progress before diff is
  ready" case is addressed by emitting a coarser step on arrival and a
  finer step once the diff stabilizes, or by emitting progress updates
  outside the toolpath file entirely.

The mechanism can be added in a future revision via new line kinds,
guarded by a version bump.

### Why strict parsing?

Skip-on-error parsing is tempting for a streaming format — readers could
ignore unknown variants from future versions and keep going. It was
rejected because:

- Toolpath documents are provenance records. Silently skipping an
  unrecognized line means silently losing provenance.
- Forward compatibility is handled more transparently by a version bump
  in `PathOpen`: an older reader fails fast with a clear message rather
  than producing a subtly incorrect sealed document.
- Round-trip losslessness requires that readers see every line.

### Why one path per file?

A single file could in principle carry an entire graph — `GraphOpen`,
then interleaved `PathOpen` / `Step` / `PathClose` blocks scoped by a
`path` field on every line. That design was rejected because:

- The primary use cases (live agent trace, multi-writer log) have one
  writer per path. Multiplexing multiple paths into one file adds
  coordination complexity that none of the use cases need.
- Graphs already have a composition mechanism (`$ref`). A graph that
  references streaming path files is simpler than a graph-scoped stream
  and composes with the existing model.
- Per-line path scoping makes every line heavier and makes "tail this
  path" harder.

### Why `graph_ref` on `PathIdentity` rather than only in `PathOpen`?

A streaming writer that knows its containing graph up front (a release
automation pipeline recording PRs) benefits from stamping `graph_ref` on
open. But round-trip requires the sealed JSON to carry the same
information, which means the canonical `PathIdentity` must have a place
to store it. Adding the field to `PathIdentity` is a small additive
change and keeps the JSONL format a pure serialization of the canonical
model.

### Why `PathMeta.patch` arrays replace instead of append?

Last-write-wins per field is simple and symmetric. Append semantics for
arrays creates asymmetry (`refs` appends but scalars replace), and the
fields most likely to "grow over time" — `actors`, `signatures` — already
have dedicated line kinds with the right semantics. Writers that want to
add a ref without disturbing existing refs emit a `PathMeta` line with the
complete new array.

## Compatibility

- **Canonical JSON readers** are unaffected. The JSONL format is a peer,
  not a replacement.
- **Existing canonical documents** remain valid. The only schema change —
  `graph_ref` on `PathIdentity` — is additive and optional.
- **Existing signatures** remain valid. Canonicalization is unchanged.
- **Tooling** that operates on canonical JSON (validate, render, query)
  can accept `.toolpath.jsonl` input by sealing internally before
  operating.

## Open Questions

### Should `stream` canonicalize line bodies with JCS?

A stronger "normalized stream" guarantee would canonicalize each line
body per JCS, so that `stream(P)` is byte-identical across
implementations. This would allow signing a stream directly rather than
signing the sealed JSON. Current answer: not in v1. The canonical signing
path (seal → JCS → sign) is sufficient and avoids introducing a second
canonicalization rule.

### Should `seal` warn or error on a truncated file?

A file missing `PathClose` and ending mid-line (the last line is not
newline-terminated) is ambiguous: still being written, or crashed
mid-write? Current answer: `seal` treats an unterminated final line as a
fatal error and treats EOF-without-`PathClose` as informational. Readers
tailing a live file use heuristics (file still open, recent mtime) to
distinguish the cases.

### Should there be a `StepRef` line for late step-level metadata?

A writer might learn, after emitting a step, that the step's `meta.intent`
should change or that a new ref should be attached. v1 has no mechanism
for this — a step is immutable once emitted. A future `StepPatch` line
could cover the case, guarded by a version bump.

## Prior Art

- **JSON Lines** (jsonlines.org): Line-oriented JSON for log files.
  Direct inspiration for the container format.
- **NDJSON**: Same idea, different name. Widely used in data pipelines.
- **Server-Sent Events (SSE)**: Line-oriented event streams over HTTP.
  The `event:` / `data:` framing influenced the variant-per-line
  approach.
- **Git pack protocol**: Variant-tagged messages over a stream. Similar
  structural pattern at a lower level of abstraction.
- **OpenTelemetry OTLP**: Structured append-only telemetry streams.
  Shares the "each record is self-describing" property.
- **JCS (RFC 8785)**: JSON Canonicalization Scheme. Used unchanged for
  the signature path.
