# toolpath-desktop Preview benchmarks

Tracks the Preview's performance on synthetic `Path` fixtures at 1k / 5k / 10k
steps. Filed against [issue #41](https://github.com/empathic/toolpath/issues/41).
Rerun after [#38](https://github.com/empathic/toolpath/issues/38) (markdown
memoization) and [#39](https://github.com/empathic/toolpath/issues/39)
(buildTree dep narrowing) land to quantify the win.

## What's measured

Two layers, split by whether an agent can run them unattended:

| Layer | Hot path | Where it runs | Automatable? |
|-------|----------|----------------|--------------|
| Pure TS | `normalize` / `buildTree` / `flattenChatHead` / `classify` / `matchesFilter` | Node-compatible, runs in `bun` | Yes — `bun run bench` |
| Tauri webview | `renderMarkdown` per turn, `diff.raw.split("\n")` per tool turn, whole-graph re-layout, DOM update | `cargo tauri dev` + real webview | No — manual Chrome DevTools |

Pure-TS numbers are reproducible on any host. Render / memory numbers need a
human at a running Tauri binary because the cost is dominated by the
webview's layout/paint, which `bun` doesn't simulate.

## Generating fixtures

Fixtures are not committed (~5 MB at 10k steps, trivially regenerated). From
the repo root:

```bash
cargo run -p toolpath-cli --bin gen_synthetic_path -- --steps 1000  --out bench/fixtures/synthetic-1k.path.json
cargo run -p toolpath-cli --bin gen_synthetic_path -- --steps 5000  --out bench/fixtures/synthetic-5k.path.json
cargo run -p toolpath-cli --bin gen_synthetic_path -- --steps 10000 --out bench/fixtures/synthetic-10k.path.json
```

Mix is ~70% conversation turns (alternating user/assistant), ~20% Edit/Write
tool invocations, ~10% MultiEdit — chosen to approximate a derived Claude
session. Seed is deterministic (default 42).

## Running the pure-TS bench

From `crates/toolpath-desktop/frontend`:

```bash
bun install
bun run bench
# or a single fixture:
bun run src/lib/__bench__/preview.bench.ts --fixture ../../../bench/fixtures/synthetic-10k.path.json
```

Prints `median / p95 / max / mean` over 10 iterations per (size × op) cell.

## Running the manual Tauri bench

1. Generate the 10k fixture (steps above).
2. `cd crates/toolpath-desktop && cargo tauri dev`.
3. When the app opens, use **New upload → Local git → Pick** (or whatever
   route gets you into Preview) on any real session; then in the dev
   console, paste the fixture as a `PreviewSlice`:
   ```js
   // In DevTools console, with the app on the Preview route:
   const raw = await fetch("file:///ABSOLUTE/PATH/TO/bench/fixtures/synthetic-10k.path.json").then(r => r.text());
   // Then dispatch a DeriveSucceeded msg via the store. Exact wiring TBD
   // — the cleanest path is a hidden "load from file" debug Msg; for now
   // just derive from a real large session and use that.
   ```
   In practice the easier route is to derive from a real long Claude session
   (e.g. `~/.claude/projects/<something>` with 1000+ turns) and measure that
   directly. Synthetic fixtures catch regressions in normalize / buildTree;
   they don't exercise `renderMarkdown` the way real prose does.
4. Open **Chrome DevTools → Performance** (Tauri exposes Chromium devtools;
   right-click → Inspect).
5. Click **Record**, interact (open Preview, type in the tree search, toggle
   view mode between `chat` and `graph`), stop.
6. Read **Scripting**, **Rendering**, **Painting**, and **Total** columns
   from the summary.
7. For memory: DevTools → **Memory → Take heap snapshot** before and after
   opening the Preview. Subtract for the Document + parsed-DOM footprint.

## Metrics table

Fill in the columns as you run them. Leave `—` for "not measured yet";
`N/A` for "not measurable in this environment".

### Legend
- **TFP** — time to first paint of the Preview after `DeriveSucceeded`
- **Keystroke** — median render time after a keypress in the tree search box
- **Mem** — resident heap delta when Preview is open vs Home route

### 2026-04-22 baseline (pre-#38, pre-#39)

Host: Apple M4 Pro / Darwin 25.4 / bun 1.3.5 (Node v24 compat) / commit `eliot/issue-41-preview-benchmark` HEAD.

Pure-TS ops, 10 iterations each:

| Size | JSON.parse | normalize | buildTree (median) | buildTree (p95) | keystroke filter | flattenChatHead | classify × all |
|------|------------|-----------|---------------------|------------------|-------------------|------------------|-----------------|
| 1k   | 1.16 ms    | 0.23 ms   | **3.98 ms**         | 7.5 ms           | 0.08 ms           | 0.23 ms          | 0.14 ms         |
| 5k   | 3.17 ms    | 0.79 ms   | **82.2 ms**         | 113 ms           | 0.43 ms           | 1.20 ms          | 0.47 ms         |
| 10k  | 6.32 ms    | 2.15 ms   | **579 ms**          | 1830 ms          | 1.12 ms           | 5.34 ms          | 1.49 ms         |

Tauri webview ops (measure manually, DevTools → Performance):

| Size | TFP | Keystroke (DOM-updated) | Mem (Document + DOM) |
|------|-----|--------------------------|------------------------|
| 1k   | —   | —                        | —                      |
| 5k   | —   | —                        | —                      |
| 10k  | —   | —                        | —                      |

### Notes on the 2026-04-22 baseline

- `buildTree` at 10k is **way above** the 200 ms target from the issue
  (median 579 ms, p95 1.8 s). Expected — the HEAD-ancestor walk inside
  `normalize` and the DFS in `flattenTree` are both O(N) with per-node
  allocations, and the variance suggests significant GC pressure. This is
  the primary thing #39 should improve.
- `filter(matchesFilter)` itself is fine in pure TS (1 ms at 10k) — the
  keystroke cost in the UI is dominated by `renderMarkdown` inside
  `StepTree.svelte`'s re-render, not the filter. #38 should eliminate that.
- `flattenChatHead` is cheap (5.3 ms at 10k) because it only walks the HEAD
  spine.
- `classify` is near-free; no need to memoize it unless #38 reveals it
  inside a hot loop.
- Numbers are from `bun` running on Node-compatible mode, not a real
  webview. Webview layout/paint and V8 JIT vs JavaScriptCore differ — treat
  these as **lower bounds** for what the Preview actually experiences.

### Post-#38 / post-#39 rerun

Rerun `bun run bench` on this same branch layout after the fix PRs land and
fill the table below. Commit numbers alongside the reference to the merged
PR number.

| Size | buildTree (median) | keystroke (DOM) | TFP (Tauri) |
|------|---------------------|------------------|--------------|
| 1k   | —                   | —                | —            |
| 5k   | —                   | —                | —            |
| 10k  | —                   | —                | —            |

## When to act

The issue calls out **200 ms on a keystroke** as the threshold. A few rules
of thumb for reading the table:

- Pure-TS `buildTree` > 100 ms at any size → probably visible as a freeze;
  deserves a targeted fix (memoization, incremental update, or dep
  narrowing — #39).
- Keystroke (DOM-updated) > 200 ms → real user-visible lag; deserves a
  targeted fix (virtualized list, markdown memo — #38).
- Memory delta > ~200 MB for a 10k-step Document → investigate the parsed
  DOM; consider lazy-rendering off-screen cards.

Don't file sub-issues for known wins from #38 / #39 — they're already
tracked.
