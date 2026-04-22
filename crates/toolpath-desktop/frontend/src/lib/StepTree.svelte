<script lang="ts">
  // Searchable step-tree sidebar for the Preview route. DFS-ordered list of
  // every step in the derived document, with ├─/└─ tree connectors, actor-
  // coloured names, and HEAD/dead-end markers. Mirrors the index.html
  // prototype that the design was iterated against.

  import { store } from "./store.svelte";
  import { buildTree, matchesFilter, type FlatNode } from "./tree";
  import type { TreeFilter } from "./types";

  const FILTERS: { key: TreeFilter; label: string }[] = [
    { key: "all",  label: "all" },
    { key: "head", label: "on HEAD" },
    { key: "dead", label: "dead ends" },
  ];

  const preview = $derived(store.m.preview);
  const doc = $derived(preview?.doc ?? null);
  const built = $derived(doc ? buildTree(doc) : null);
  const query = $derived((preview?.treeQuery ?? "").trim().toLowerCase());
  const filter = $derived<TreeFilter>(preview?.treeFilter ?? "all");
  const rows = $derived<FlatNode[]>(
    built ? built.nodes.filter((n) => matchesFilter(n, query, filter)) : [],
  );
  const selectedId = $derived(preview?.selectedStep?.step.id ?? null);

  function shortId(id: string): string {
    // Derived IDs from Claude/Pi/etc are UUIDs (~36 chars). Keep them
    // compact in the tree; the full id is available via the title tooltip.
    return id.length > 10 ? id.slice(0, 8) + "…" : id;
  }

  function onRowClick(node: FlatNode) {
    if (!built || !preview) return;
    const step = built.norm.stepMap.get(node.id);
    if (!step) return;
    // If the target is a dead node, auto-expand its HEAD-ancestor branch so
    // the graph reveals it when the user looks over.
    if (!node.onHead) {
      let cursor: string | undefined = node.id;
      let ancestor: string | undefined;
      while (cursor) {
        const parents: string[] = built.norm.stepMap.get(cursor)?.step.parents ?? [];
        const p: string | undefined = parents[0];
        if (!p) break;
        if (built.norm.headSet.has(p)) { ancestor = p; break; }
        cursor = p;
      }
      if (ancestor && !preview.expandedBranches[ancestor]) {
        store.dispatch({ t: "PreviewToggleBranch", nodeId: ancestor });
      }
    }
    store.dispatch({ t: "PreviewSelectStep", step, actors: built.norm.actors });
  }
</script>

{#if preview && built}
  {@const total = built.nodes.length}
  {@const onHead = built.nodes.filter((n) => n.onHead).length}
  <div class="step-tree">
    <div class="step-tree__head">
      <div class="step-tree__title">{preview.filename}</div>
      <div class="step-tree__sub">
        {total} step{total === 1 ? "" : "s"} · {onHead} on HEAD
        {#if built.norm.head}· head <code>{built.norm.head}</code>{/if}
      </div>
    </div>
    <div class="step-tree__controls">
      <input
        class="step-tree__search"
        type="search"
        placeholder="search steps…"
        value={preview.treeQuery}
        oninput={(e: Event) =>
          store.dispatch({ t: "PreviewSetTreeQuery", value: (e.currentTarget as HTMLInputElement).value })}
      />
    </div>
    <div class="step-tree__filters" role="tablist">
      {#each FILTERS as f (f.key)}
        <button
          class={"step-tree__filter-btn" + (filter === f.key ? " step-tree__filter-btn--active" : "")}
          role="tab"
          aria-selected={filter === f.key}
          onclick={() => store.dispatch({ t: "PreviewSetTreeFilter", value: f.key })}
        >{f.label}</button>
      {/each}
    </div>
    <div class="step-tree__rows">
      {#if rows.length === 0}
        <div class="step-tree__empty">
          {query || filter !== "all" ? "No steps match." : "No steps in this document."}
        </div>
      {:else}
        {#each rows as node (node.id)}
          {@const isSelected = node.id === selectedId}
          <button
            type="button"
            class={
              "step-tree__row step-tree__row--role-" + node.actorKind
              + (node.onHead ? " step-tree__row--on-head" : " step-tree__row--off-head")
              + (isSelected ? " step-tree__row--active" : "")
            }
            onclick={() => onRowClick(node)}
          >
            <span class="step-tree__prefix">{node.prefix}</span>
            <span
              class={
                "step-tree__marker "
                + (node.isHead ? "step-tree__marker--head"
                  : node.isDead ? "step-tree__marker--dead"
                  : "step-tree__marker--on")
              }
            >{node.isHead ? "★" : node.isDead ? "⌀" : "●"}</span>
            <span class="step-tree__id" title={node.id}>{shortId(node.id)}</span>
            <span class="step-tree__actor">{node.actorDisplay}</span>
            {#if node.intent}
              <span class="step-tree__intent">· {node.intent}</span>
            {/if}
          </button>
        {/each}
      {/if}
    </div>
    <div class="step-tree__legend">
      <span><span style="color:var(--contour-2)">★</span> HEAD</span>
      <span><span style="color:var(--ink-4)">●</span> on HEAD</span>
      <span><span style="color:var(--road)">⌀</span> dead end</span>
    </div>
  </div>
{/if}
