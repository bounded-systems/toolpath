<script lang="ts">
  import { perf, perfOverlayEnabled } from "./perf.svelte";

  const enabled = $derived(perfOverlayEnabled());
  const trace = $derived(perf.latest);

  // Map marks to phase bars. Each bar spans [prev, t] and is labelled by the
  // mark's name — so e.g. "invoke-end" shows the cost of the invoke phase,
  // not the instant it happened.
  type Phase = { name: string; start: number; end: number; width: number };
  const phases = $derived<Phase[]>((() => {
    if (!trace) return [];
    const out: Phase[] = [];
    let prev = 0;
    const total = trace.durationMs ?? trace.marks.at(-1)?.t ?? 1;
    for (const m of trace.marks) {
      out.push({
        name: m.name,
        start: prev,
        end: m.t,
        width: Math.max(0.5, ((m.t - prev) / total) * 100),
      });
      prev = m.t;
    }
    return out;
  })());

  function fmt(n: number): string {
    return n < 10 ? n.toFixed(1) : n.toFixed(0);
  }
</script>

{#if enabled && trace}
  <div class="perf" role="status" aria-live="polite">
    <div class="perf__head">
      <span class="perf__label">{trace.label}</span>
      <span class="perf__total">
        {#if trace.durationMs != null}
          {fmt(trace.durationMs)}ms
        {:else}
          running…
        {/if}
      </span>
    </div>
    <div class="perf__bar">
      {#each phases as p (p.name + p.start)}
        <div
          class="perf__phase"
          style="width:{p.width}%"
          title="{p.name}: {fmt(p.end - p.start)}ms"
        >
          <span class="perf__phase-name">{p.name}</span>
          <span class="perf__phase-t">{fmt(p.end - p.start)}</span>
        </div>
      {/each}
    </div>
    <details class="perf__detail">
      <summary>marks</summary>
      <table>
        <thead>
          <tr><th>mark</th><th>at</th><th>Δ</th></tr>
        </thead>
        <tbody>
          {#each phases as p (p.name + p.start)}
            <tr>
              <td>{p.name}</td>
              <td>{fmt(p.end)}</td>
              <td>+{fmt(p.end - p.start)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </details>
  </div>
{/if}

<style>
  .perf {
    position: fixed;
    right: 12px;
    bottom: 12px;
    z-index: 9999;
    min-width: 340px;
    max-width: 560px;
    padding: 8px 10px;
    background: rgba(20, 20, 22, 0.92);
    color: #f4f0e8;
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11px;
    line-height: 1.35;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.3);
    pointer-events: auto;
  }
  .perf__head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
    margin-bottom: 6px;
  }
  .perf__label { font-weight: 600; }
  .perf__total {
    font-variant-numeric: tabular-nums;
    color: #f4c078;
  }
  .perf__bar {
    display: flex;
    height: 18px;
    border-radius: 3px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.06);
  }
  .perf__phase {
    position: relative;
    overflow: hidden;
    padding: 0 4px;
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 10px;
    line-height: 1;
    white-space: nowrap;
    border-right: 1px solid rgba(0, 0, 0, 0.3);
    color: #1d1d1f;
  }
  .perf__phase:nth-child(6n + 1) { background: #f4c078; }
  .perf__phase:nth-child(6n + 2) { background: #a4c4a0; }
  .perf__phase:nth-child(6n + 3) { background: #c48aa8; }
  .perf__phase:nth-child(6n + 4) { background: #88b4c0; }
  .perf__phase:nth-child(6n + 5) { background: #e0b070; }
  .perf__phase:nth-child(6n)     { background: #b8a078; }
  .perf__phase-name { font-weight: 600; }
  .perf__phase-t {
    font-variant-numeric: tabular-nums;
    opacity: 0.85;
  }
  .perf__detail {
    margin-top: 6px;
    opacity: 0.8;
  }
  .perf__detail summary {
    cursor: pointer;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    font-size: 10px;
  }
  .perf__detail table {
    width: 100%;
    margin-top: 4px;
    border-collapse: collapse;
  }
  .perf__detail th,
  .perf__detail td {
    padding: 2px 6px 2px 0;
    text-align: left;
    font-variant-numeric: tabular-nums;
  }
  .perf__detail th {
    opacity: 0.6;
    font-weight: normal;
  }
</style>
