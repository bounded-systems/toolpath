<script lang="ts">
  import { render as renderViz } from "../lib/viz";
  import { store } from "../lib/store.svelte";
  import type { StepRef } from "../lib/types";

  let svgEl: SVGSVGElement | null = $state(null);

  // Re-render the viz when the underlying doc or toggles change.
  $effect(() => {
    const p = store.m.preview;
    if (!svgEl || !p) return;
    const doc = p.doc;
    const toggles = { showDead: p.showDead, showTs: p.showTs, showFiles: p.showFiles };
    // Referenced to establish dep tracking:
    const _epoch = p.vizEpoch;
    renderViz(doc, svgEl, {
      ...toggles,
      onStepClick: (step, actors) => store.dispatch({ t: "PreviewSelectStep", step, actors }),
    });
  });

  function extractSteps(): StepRef[] {
    const d = store.m.preview?.doc;
    if (!d) return [];
    if ("Step" in d) return [d.Step];
    if ("Path" in d) return d.Path.steps;
    if ("Graph" in d) return d.Graph.paths.flatMap((p) => ("$ref" in p ? [] : p.steps));
    return [];
  }

  const preview = $derived(store.m.preview);
  const steps = $derived(extractSteps());
  const actorSet = $derived(new Set(steps.map((s) => s.step.actor)));
  const times = $derived(
    steps.map((s) => s.step.timestamp).filter((t): t is string => !!t).sort(),
  );
</script>

{#if !preview}
  <p>Nothing to preview.</p>
{:else}
  <div class="row">
    <button class="linklike" onclick={() => store.dispatch({ t: "NavigateTo", screen: "home" })}>← Back</button>
    <div class="spacer"></div>
    <span class="list__meta">{preview.source}</span>
  </div>
  <h1>Preview</h1>
  <p class="subtitle">
    {steps.length} step{steps.length === 1 ? "" : "s"} · {actorSet.size} actor{actorSet.size === 1 ? "" : "s"}
    {#if times.length >= 2} · spans {times[0].slice(0, 10)} → {times[times.length - 1].slice(0, 10)}{/if}
  </p>

  <div class="toolbar">
    <label>
      <input type="checkbox" class="checkbox" checked={preview.showDead} onchange={() => store.dispatch({ t: "PreviewToggle", key: "showDead" })} />
      Dead ends
    </label>
    <label>
      <input type="checkbox" class="checkbox" checked={preview.showTs} onchange={() => store.dispatch({ t: "PreviewToggle", key: "showTs" })} />
      Timestamps
    </label>
    <label>
      <input type="checkbox" class="checkbox" checked={preview.showFiles} onchange={() => store.dispatch({ t: "PreviewToggle", key: "showFiles" })} />
      Files touched
    </label>
    <div class="spacer"></div>
  </div>

  <div class="preview-layout">
    <div class="preview-canvas">
      <svg bind:this={svgEl}></svg>
    </div>
    <div class="preview-panel">
      {#if !preview.selectedStep}
        <div class="preview-panel__empty">Click a step in the graph to inspect its diff and metadata.</div>
      {:else}
        {@const s = preview.selectedStep}
        {@const actor = s.step.actor}
        {@const def = preview.selectedActors?.[actor]}
        {@const displayName = def?.name ?? actor.split(":").slice(1).join(":")}
        {@const changeKeys = s.change ? Object.keys(s.change) : []}
        <h3>{s.step.id}</h3>
        <dl>
          <dt>Actor</dt>
          <dd>{displayName} <span class="list__meta">{actor}</span></dd>
          {#if s.step.timestamp}
            <dt>Time</dt><dd>{s.step.timestamp}</dd>
          {/if}
          {#if s.step.parents && s.step.parents.length}
            <dt>Parents</dt><dd>{s.step.parents.join(", ")}</dd>
          {/if}
          {#if s.meta?.intent}
            <dt>Intent</dt><dd>{s.meta.intent}</dd>
          {/if}
        </dl>
        {#if changeKeys.length === 0}
          <div class="preview-panel__empty">No change body on this step.</div>
        {:else}
          {#each changeKeys as k (k)}
            {@const ch = s.change![k]}
            <div class="list__meta" style="margin-top:10px">{k}</div>
            {#if ch.raw}<pre>{ch.raw}</pre>{/if}
            {#if ch.structural}<pre>{JSON.stringify(ch.structural, null, 2)}</pre>{/if}
          {/each}
        {/if}
      {/if}
    </div>
  </div>

  <div class="row" style="margin-top:12px">
    <div class="spacer"></div>
    <button class="secondary" disabled={preview.uploading} onclick={() => store.dispatch({ t: "PreviewUpload" })}>
      {preview.uploading ? "Uploading…" : "Upload to Pathbase"}
    </button>
    <button class="primary" disabled={preview.exporting} onclick={() => store.dispatch({ t: "PreviewExport" })}>
      {preview.exporting ? "Saving…" : "Export as .path.json"}
    </button>
  </div>
{/if}
