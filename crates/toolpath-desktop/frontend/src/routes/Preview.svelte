<script lang="ts">
  import { render as renderViz } from "../lib/viz";
  import { store } from "../lib/store.svelte";
  import StepTree from "../lib/StepTree.svelte";
  import ChatView from "../lib/ChatView.svelte";
  import type { StepRef } from "../lib/types";

  let canvasEl: HTMLDivElement | null = $state(null);

  // Graph-view zoom. Wheel/pinch with ctrl/meta zooms; plain wheel pans via
  // the scroll container. Reset whenever the previewed doc changes.
  const MIN_ZOOM = 0.25;
  const MAX_ZOOM = 3;
  const ZOOM_STEP = 1.15;
  let zoom = $state(1);
  $effect(() => { void store.m.preview?.doc; zoom = 1; });

  function clampZoom(z: number): number {
    return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, z));
  }
  function zoomAt(delta: number) { zoom = clampZoom(zoom * delta); }
  function zoomIn()   { zoomAt(ZOOM_STEP); }
  function zoomOut()  { zoomAt(1 / ZOOM_STEP); }
  function zoomReset() { zoom = 1; }

  function onCanvasWheel(ev: WheelEvent) {
    // macOS trackpad pinch arrives as wheel events with ctrlKey=true; plain
    // two-finger scroll has ctrlKey=false and should bubble to the default
    // scroll behaviour.
    if (!(ev.ctrlKey || ev.metaKey)) return;
    ev.preventDefault();
    // deltaY < 0 means the user scrolled/pinched outward → zoom in.
    const factor = Math.exp(-ev.deltaY * 0.01);
    zoomAt(factor);
  }

  // Re-render the graph whenever the underlying doc, selection, or toggles
  // change. `vizEpoch` bumps on every toggle / branch-expand. When view
  // mode is "chat" the canvas div is not in the DOM, so this is a no-op.
  $effect(() => {
    const p = store.m.preview;
    if (!canvasEl || !p || p.viewMode !== "graph") return;
    const _epoch = p.vizEpoch;
    renderViz(p.doc, canvasEl, {
      selectedStepId: p.selectedStep?.step.id ?? null,
      expandedBranches: p.expandedBranches,
      showTs: p.showTs,
      showFiles: p.showFiles,
      onSelectStep: (step, actors) =>
        store.dispatch({ t: "PreviewSelectStep", step, actors }),
      onToggleBranch: (nodeId) =>
        store.dispatch({ t: "PreviewToggleBranch", nodeId }),
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
  const docTitle = $derived(preview?.filename ?? "(untitled)");
</script>

{#if !preview}
  <div class="empty">
    <div class="empty__mark">△</div>
    <div class="empty__title">Nothing to preview</div>
    <p class="empty__body">
      Pick a repo, pull request, or conversation from the home tab. Toolpath
      will derive a Path document, show its provenance graph, and let you
      upload it to <span style="color:var(--road)">pathbase.dev</span>.
    </p>
    <div class="empty__hint">⎇  ◇  ⊕  ●   ·   four source types</div>
  </div>
{:else}
  <!-- Preview header band -->
  <div class="preview-header">
    <div>
      <div class="preview-header__eyebrow">§2 · DERIVED PATH · {preview.source}</div>
      <div class="preview-header__title">{docTitle}</div>
      <div class="preview-header__stats">
        △ {steps.length} step{steps.length === 1 ? "" : "s"}
        · ◆ {actorSet.size} actor{actorSet.size === 1 ? "" : "s"}
        {#if times.length >= 2}· {times[0].slice(0, 10)} → {times[times.length - 1].slice(0, 10)}{/if}
      </div>
    </div>
    <div class="preview-header__actions">
      <span class="tag tag--ok">◇ Valid</span>
      <button class="btn" disabled={preview.exporting} onclick={() => store.dispatch({ t: "PreviewExport" })}>
        {preview.exporting ? "Saving…" : "Export .json"}
      </button>
      <button class="btn btn--accent" disabled={preview.uploading} onclick={() => store.dispatch({ t: "PreviewUpload" })}>
        {preview.uploading ? "Uploading…" : "Upload to pathbase.dev →"}
      </button>
    </div>
  </div>

  <!-- Body: tree + inspector (left) + visualizer (right) -->
  <div class="preview-body preview-body--split">
    <div class="preview-body__left">
      <div class="section-label">
        <span class="section-label__num">§2.1 ·</span>
        <span class="section-label__text">Steps</span>
        <span class="section-label__right">TREE</span>
      </div>

      <StepTree />

      <div style="height:18px"></div>

      <div class="section-label">
        <span class="section-label__num">§2.2 ·</span>
        <span class="section-label__text">Inspector</span>
        <span class="section-label__right">STEP</span>
      </div>

      <div class="inspector">
        {#if !preview.selectedStep}
          <div class="inspector__empty">Click a step in the tree or graph to inspect its diff and metadata.</div>
        {:else}
          {@const s = preview.selectedStep}
          {@const actor = s.step.actor}
          {@const def = preview.selectedActors?.[actor]}
          {@const displayName = def?.name ?? actor.split(":").slice(1).join(":")}
          {@const changeKeys = s.change ? Object.keys(s.change) : []}
          <div style="font-family:var(--font-display); font-size:18px; font-weight:600; color:var(--ink); margin-bottom:8px">
            {s.step.id}
          </div>
          <div class="meta-table">
            <div class="meta-table__k">Actor</div>
            <div class="meta-table__v">{displayName} <span style="color:var(--ink-4)">{actor}</span></div>
            {#if s.step.timestamp}
              <div class="meta-table__k">Time</div>
              <div class="meta-table__v">{s.step.timestamp}</div>
            {/if}
            {#if s.step.parents && s.step.parents.length}
              <div class="meta-table__k">Parents</div>
              <div class="meta-table__v">{s.step.parents.join(", ")}</div>
            {/if}
            {#if s.meta?.intent}
              <div class="meta-table__k">Intent</div>
              <div class="meta-table__v" style="font-family:var(--font-serif); font-size:13.5px">{s.meta.intent}</div>
            {/if}
          </div>
          {#if changeKeys.length === 0}
            <div class="inspector__empty" style="margin-top:10px">No change body on this step.</div>
          {:else}
            {#each changeKeys as k (k)}
              {@const ch = s.change![k]}
              <div class="page__eyebrow" style="margin-top:14px">{k}</div>
              {#if ch.raw}<pre>{ch.raw}</pre>{/if}
              {#if ch.structural}<pre>{JSON.stringify(ch.structural, null, 2)}</pre>{/if}
            {/each}
          {/if}
        {/if}
      </div>

      <div style="height:18px"></div>
      <div class="section-label">
        <span class="section-label__num">§2.3 ·</span>
        <span class="section-label__text">Document</span>
        <span class="section-label__right">META</span>
      </div>
      <div class="meta-table">
        <div class="meta-table__k">Source</div>
        <div class="meta-table__v">{preview.source}</div>
        <div class="meta-table__k">Filename</div>
        <div class="meta-table__v">{preview.filename}</div>
        <div class="meta-table__k">Steps</div>
        <div class="meta-table__v">{steps.length}</div>
        <div class="meta-table__k">Actors</div>
        <div class="meta-table__v">{actorSet.size}</div>
      </div>
    </div>

    <div class="preview-body__right">
      <div class="section-label">
        <span class="section-label__num">§2.4 ·</span>
        <span class="section-label__text">
          {preview.viewMode === "chat" ? "Transcript" : "Graph view"}
        </span>
        <span class="section-label__right">
          {preview.viewMode === "chat" ? "CHAT" : "DAG"}
        </span>
      </div>

      <div class="view-toggle" role="tablist" aria-label="Preview view mode">
        <button
          type="button"
          role="tab"
          aria-selected={preview.viewMode === "chat"}
          class={"view-toggle__btn" + (preview.viewMode === "chat" ? " view-toggle__btn--active" : "")}
          onclick={() => store.dispatch({ t: "PreviewSetViewMode", value: "chat" })}
        >Chat view</button>
        <button
          type="button"
          role="tab"
          aria-selected={preview.viewMode === "graph"}
          class={"view-toggle__btn" + (preview.viewMode === "graph" ? " view-toggle__btn--active" : "")}
          onclick={() => store.dispatch({ t: "PreviewSetViewMode", value: "graph" })}
        >Graph view</button>
      </div>

      {#if preview.viewMode === "chat"}
        <ChatView />

        <div style="margin-top:12px; font-family:var(--font-mono); font-size:10px; color:var(--ink-4); letter-spacing:0.08em; text-transform:uppercase; display:flex; gap:16px; align-items:center; border-top:0.5px solid var(--ink-5); padding-top:8px">
          <span>HEAD path only</span>
          <span class="spacer"></span>
          <span>{steps.length} steps · {actorSet.size} actors</span>
        </div>
      {:else}
        <div class="toolbar">
          <label>
            <input type="checkbox" class="checkbox" checked={preview.showTs} onchange={() => store.dispatch({ t: "PreviewToggle", key: "showTs" })} />
            Timestamps
          </label>
          <label>
            <input type="checkbox" class="checkbox" checked={preview.showFiles} onchange={() => store.dispatch({ t: "PreviewToggle", key: "showFiles" })} />
            Files touched
          </label>
          <span class="spacer"></span>
          <div class="zoom-ctl" role="group" aria-label="Graph zoom">
            <button type="button" class="zoom-ctl__btn" onclick={zoomOut} disabled={zoom <= MIN_ZOOM + 0.001} aria-label="Zoom out">−</button>
            <button type="button" class="zoom-ctl__pct" onclick={zoomReset} title="Reset to 100%">{Math.round(zoom * 100)}%</button>
            <button type="button" class="zoom-ctl__btn" onclick={zoomIn} disabled={zoom >= MAX_ZOOM - 0.001} aria-label="Zoom in">+</button>
          </div>
          <span class="kbd">⌘/ctrl + scroll to zoom</span>
        </div>

        <div
          class="graph-scroll"
          onwheel={onCanvasWheel}
        >
          <!-- `zoom` (non-standard but supported in WebKit/Chromium → all
               Tauri runtimes) actually scales layout size, so the outer
               scroll container sees correct content bounds. We fall back to
               `transform: scale` with inverse-compensated width/height in
               the stylesheet for engines that don't implement `zoom`
               (e.g. Firefox), via an `@supports not (zoom: 2)` block. -->
          <div class="graph-zoom" style="--pg-zoom: {zoom}" bind:this={canvasEl}></div>
        </div>

        <div style="margin-top:12px; font-family:var(--font-mono); font-size:10px; color:var(--ink-4); letter-spacing:0.08em; text-transform:uppercase; display:flex; gap:16px; align-items:center; border-top:0.5px solid var(--ink-5); padding-top:8px">
          <span style="display:inline-flex; align-items:center; gap:5px">
            <span style="width:8px; height:8px; background:var(--road); transform:rotate(45deg); display:inline-block"></span>
            human
          </span>
          <span style="display:inline-flex; align-items:center; gap:5px">
            <span style="width:8px; height:8px; background:var(--water); border-radius:999px; display:inline-block"></span>
            agent
          </span>
          <span style="display:inline-flex; align-items:center; gap:5px">
            <span style="width:8px; height:8px; background:var(--paper-white); border:1px dashed var(--road); display:inline-block"></span>
            dead-end
          </span>
          <span class="spacer"></span>
          <span>{steps.length} steps · {actorSet.size} actors</span>
        </div>
      {/if}
    </div>
  </div>
{/if}
