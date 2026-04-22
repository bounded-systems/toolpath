<script lang="ts">
  // Chat / transcript view modelled on Claude's native desktop chat:
  //   — User messages: right-aligned, soft card, warm paper tint.
  //   — Assistant messages: left-aligned prose, no bubble; model label
  //     above, tool-use chip below if the turn invoked tools.
  //   — Tool invocations: expandable inline blocks (collapsed by default).
  //   — System / init / event entries: centered hair-rule divider.
  //
  // Data is extracted from `step.change[k].structural.extra` by
  // `flattenChatHead` in ./tree.ts.

  import { tick } from "svelte";
  import { store } from "./store.svelte";
  import {
    buildTree,
    flattenChatHead,
    type ChatTurn,
  } from "./tree";

  const preview = $derived(store.m.preview);
  const doc = $derived(preview?.doc ?? null);
  const built = $derived(doc ? buildTree(doc) : null);
  const turns = $derived<ChatTurn[]>(built ? flattenChatHead(built.norm) : []);
  const selectedId = $derived(preview?.selectedStep?.step.id ?? null);


  let scrollEl: HTMLDivElement | null = $state(null);
  let expandedTools: Record<string, true> = $state({});
  // Explicitly-collapsed-by-user set. File-write tools with a diff default
  // to expanded; clicking the head adds the id here to collapse it.
  let collapsedTools: Record<string, true> = $state({});

  function isExpanded(t: ChatTurn): boolean {
    if (collapsedTools[t.id]) return false;
    if (expandedTools[t.id]) return true;
    // Default: auto-expand tool turns that have a raw diff to show.
    return !!t.toolDiff;
  }

  $effect(() => {
    const id = selectedId;
    if (!id || !scrollEl) return;
    (async () => {
      await tick();
      const el = scrollEl?.querySelector<HTMLElement>(`[data-chat-id="${cssEsc(id)}"]`);
      if (el) el.scrollIntoView({ block: "nearest", behavior: "smooth" });
    })();
  });

  function cssEsc(s: string): string {
    return s.replace(/"/g, '\\"');
  }

  function onSelect(t: ChatTurn) {
    if (!built) return;
    store.dispatch({
      t: "PreviewSelectStep",
      step: t.step,
      actors: built.norm.actors,
    });
  }

  function toggleTool(t: ChatTurn) {
    const id = t.id;
    const currentlyOpen = isExpanded(t);
    if (currentlyOpen) {
      collapsedTools = { ...collapsedTools, [id]: true };
      const e = { ...expandedTools };
      delete e[id];
      expandedTools = e;
    } else {
      const c = { ...collapsedTools };
      delete c[id];
      collapsedTools = c;
      expandedTools = { ...expandedTools, [id]: true };
    }
  }

  function fmtTime(iso: string | null): string {
    if (!iso) return "";
    return iso.replace("T", " ").replace("Z", "");
  }

  function readField(t: ChatTurn, field: string): string {
    // StructuralChange#extra is #[serde(flatten)]-ed, so fields live on
    // the structural root. Scan all change artifacts — the tool payload
    // tends to be on the agent://…/tool/… key, not the conversation one.
    const ch = t.step.change;
    if (!ch) return "";
    for (const key of Object.keys(ch)) {
      const s = ch[key]?.structural as Record<string, unknown> | undefined;
      if (!s) continue;
      const v = s[field];
      if (v == null) continue;
      if (typeof v === "string") return v;
      try { return JSON.stringify(v, null, 2); } catch { return String(v); }
    }
    return "";
  }
  const toolInput = (t: ChatTurn) => readField(t, "input");
  const toolResult = (t: ChatTurn) => readField(t, "result");

  /** Tag each diff line with a class so CSS can color add/remove/hunk/meta. */
  function diffLineClass(line: string): string {
    if (line.startsWith("@@")) return "diff-line diff-line--hunk";
    if (line.startsWith("+++") || line.startsWith("---")) return "diff-line diff-line--meta";
    if (line.startsWith("+")) return "diff-line diff-line--add";
    if (line.startsWith("-")) return "diff-line diff-line--del";
    if (line.startsWith("#")) return "diff-line diff-line--meta"; // "# edit N/total"
    return "diff-line";
  }
</script>

{#if preview && built}
  {#if turns.length === 0}
    <div class="chat-view__empty">
      This document has no HEAD path to transcribe. Switch to graph view to
      inspect the raw DAG.
    </div>
  {:else}
    <div class="chat-view" bind:this={scrollEl}>
      {#each turns as t (t.id)}
        {@const selected = t.id === selectedId}

        {#if t.kind === "user"}
          <div
            class={"chat-msg chat-msg--user" + (selected ? " chat-msg--active" : "")}
            data-chat-id={t.id}
            onclick={() => onSelect(t)}
            role="button"
            tabindex="0"
            onkeydown={(e) => { if (e.key === "Enter") onSelect(t); }}
          >
            <div class="chat-msg__bubble">
              {#if t.text}
                <div class="chat-msg__text markdown">{@html t.textHtml}</div>
              {:else}
                <div class="chat-msg__text chat-msg__text--empty">(empty message)</div>
              {/if}
            </div>
            <div class="chat-msg__meta">
              <span>You</span>
              {#if t.timestamp}<span class="chat-msg__sep">·</span><span>{fmtTime(t.timestamp)}</span>{/if}
            </div>
          </div>

        {:else if t.kind === "assistant"}
          <div
            class={"chat-msg chat-msg--assistant" + (selected ? " chat-msg--active" : "")}
            data-chat-id={t.id}
            onclick={() => onSelect(t)}
            role="button"
            tabindex="0"
            onkeydown={(e) => { if (e.key === "Enter") onSelect(t); }}
          >
            <div class="chat-msg__label">
              <span class="chat-msg__name">{t.actorDisplay}</span>
              {#if t.model}<span class="chat-msg__sep">·</span><span class="chat-msg__model">{t.model}</span>{/if}
            </div>

            {#if t.thinking}
              <details class="chat-thinking">
                <summary>Thinking…</summary>
                <div class="chat-thinking__body markdown">{@html t.thinkingHtml}</div>
              </details>
            {/if}

            {#if t.text}
              <div class="chat-msg__text markdown">{@html t.textHtml}</div>
            {:else if t.toolNames.length === 0}
              <div class="chat-msg__text chat-msg__text--empty">(no text in this turn)</div>
            {/if}

            {#if t.toolNames.length}
              <div class="chat-tools">
                <span class="chat-tools__label">Used</span>
                {#each t.toolNames as name, i (i)}
                  <span class="chat-tools__chip">{name}</span>
                {/each}
              </div>
            {/if}

            <div class="chat-msg__meta chat-msg__meta--left">
              {#if t.timestamp}<span>{fmtTime(t.timestamp)}</span>{/if}
            </div>

            {#if t.toolInvocations.length}
              <div class="chat-tool-list">
                {#each t.toolInvocations as tool (tool.id)}
                  {@const expanded = isExpanded(tool)}
                  {@const diff = tool.toolDiff}
                  <div class={"chat-tool" + (diff ? " chat-tool--has-diff" : "")} data-chat-id={tool.id}>
                    <button
                      type="button"
                      class="chat-tool__head"
                      onclick={(e) => { e.stopPropagation(); toggleTool(tool); }}
                      aria-expanded={expanded}
                    >
                      <span class="chat-tool__caret">{expanded ? "▾" : "▸"}</span>
                      <span class="chat-tool__name">{tool.toolName ?? "tool"}</span>
                      {#if diff}
                        <span class="chat-tool__sep">·</span>
                        <span class="chat-tool__path">{diff.path}</span>
                      {/if}
                      <span class="chat-msg__spacer"></span>
                      {#if tool.timestamp}<span class="chat-tool__ts">{fmtTime(tool.timestamp)}</span>{/if}
                    </button>
                    {#if expanded}
                      {@const input = toolInput(tool)}
                      {@const result = toolResult(tool)}
                      <div class="chat-tool__body">
                        {#if diff}
                          <div class="chat-tool__section-label">Diff · {diff.path}</div>
                          <pre class="chat-tool__diff"><code>{#each diff.lines as line, i (i)}<span class={diffLineClass(line)}>{line}
</span>{/each}</code></pre>
                        {:else if input}
                          <div class="chat-tool__section-label">Input</div>
                          <pre class="chat-tool__pre">{input}</pre>
                        {/if}
                        {#if result}
                          <div class="chat-tool__section-label">Result</div>
                          <pre class="chat-tool__pre">{result}</pre>
                        {/if}
                        {#if !diff && !input && !result}
                          <div class="chat-tool__empty">(no captured input/result)</div>
                        {/if}
                      </div>
                    {/if}
                  </div>
                {/each}
              </div>
            {/if}
          </div>

        {:else}
          <!-- system / init / event -->
          <div class="chat-divider" data-chat-id={t.id} onclick={() => onSelect(t)} role="button" tabindex="0" onkeydown={(e) => { if (e.key === "Enter") onSelect(t); }}>
            <span class="chat-divider__line"></span>
            <span class="chat-divider__label">
              {t.intent || t.actorDisplay || "system event"}
              {#if t.timestamp}<span class="chat-msg__sep">·</span>{fmtTime(t.timestamp)}{/if}
            </span>
            <span class="chat-divider__line"></span>
          </div>
        {/if}
      {/each}
    </div>
  {/if}
{/if}
