// Lightweight performance tracer for click → derive → render flows.
//
// Each logical operation (e.g. "derive claude session") has a single running
// trace with one or more named marks. The most-recently-completed trace lives
// in the reactive `perf.latest` field so a small overlay can visualise which
// phase took how long. Every completed trace is also dumped to the devtools
// console.
//
// Enable the on-screen overlay by calling `perfSetOverlayEnabled(true)` (or
// `localStorage.setItem("perf", "1")`) and reloading. Console output is
// always on.
//
// Typical call sequence for a "click Select → preview mounted" flow:
//
//   perf.start("derive claude");
//   perf.mark("dispatch");
//   perf.mark("invoke-start");
//   perf.mark("invoke-end");
//   perf.mark("model-updated");
//   perf.mark("preview-mounted");
//   perf.mark("viz-rendered");
//   perf.end();

export type PerfMark = { name: string; t: number };
export type PerfTrace = {
  label: string;
  startedAt: number;
  marks: PerfMark[];
  durationMs: number | null;
};

// Reactive module-level state. Svelte 5 tracks reads of these fields across
// .svelte files that import them.
export const perf = $state<{ latest: PerfTrace | null }>({ latest: null });

// Non-reactive scratch pad for the in-flight trace.
let current: PerfTrace | null = null;

function now(): number {
  return typeof performance !== "undefined" ? performance.now() : Date.now();
}

// `perf.latest` is `$state`, so any assignment inside a Svelte `$derived`
// (e.g. `perfMark` from within `buildTree` reached via a `$derived` in
// ChatView) throws `state_unsafe_mutation`. Defer writes to a microtask so
// the mutation always happens outside derivation. The visible ordering is
// unchanged — multiple marks in one task resolve in order, last write wins.
function publish(trace: PerfTrace): void {
  const snapshot = { ...trace, marks: trace.marks };
  queueMicrotask(() => {
    perf.latest = snapshot;
  });
}

export function perfStart(label: string): void {
  const t = now();
  current = { label, startedAt: t, marks: [], durationMs: null };
  publish(current);
}

export function perfMark(name: string): void {
  if (!current) return;
  const t = now() - current.startedAt;
  current.marks = [...current.marks, { name, t }];
  publish(current);
}

export function perfEnd(): void {
  if (!current) return;
  const dur = now() - current.startedAt;
  current.durationMs = dur;
  publish(current);

  // Summary to console. Each mark shows absolute-from-start and delta from
  // the previous mark so the slow phase is easy to spot.
  const lines: string[] = [`${current.label}  (total ${dur.toFixed(1)}ms)`];
  let prev = 0;
  for (const m of current.marks) {
    const delta = m.t - prev;
    lines.push(
      `  ${m.name.padEnd(18)} ${m.t.toFixed(1).padStart(8)}ms  (+${delta.toFixed(1)}ms)`,
    );
    prev = m.t;
  }
  // eslint-disable-next-line no-console
  console.log("%cperf", "color:#b5652b;font-weight:600", "\n" + lines.join("\n"));
  current = null;
}

export function perfOverlayEnabled(): boolean {
  try {
    return globalThis.localStorage?.getItem("perf") === "1";
  } catch {
    return false;
  }
}

export function perfSetOverlayEnabled(on: boolean): void {
  try {
    if (on) globalThis.localStorage?.setItem("perf", "1");
    else globalThis.localStorage?.removeItem("perf");
  } catch {
    // ignore — overlay is a nice-to-have
  }
}
