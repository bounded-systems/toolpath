<script lang="ts">
  import { store } from "../lib/store.svelte";

  const r = $derived(store.m.result);

  async function openUrl(url: string) {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  }
  async function revealInDir(path: string) {
    const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
    await revealItemInDir(path);
  }
</script>

<div class="page">
  {#if !r}
    <div class="empty">
      <div class="empty__mark">△</div>
      <div class="empty__title">No result yet</div>
      <p class="empty__body">Derive and upload a Path document to see the outcome here.</p>
    </div>
  {:else if r.kind === "export"}
    <div class="page__header">
      <div>
        <div class="page__eyebrow">§3 · EXPORT · COMPLETE</div>
        <h1 class="page__title">Saved to disk</h1>
        <p class="page__lede">Your trace was written as a <code>.path.json</code> document.</p>
      </div>
      <span class="tag tag--ok">✓ Saved</span>
    </div>

    <div class="card-panel">
      <div class="meta-table">
        <div class="meta-table__k">Source</div>
        <div class="meta-table__v">{r.source}</div>
        <div class="meta-table__k">Path</div>
        <div class="meta-table__v">{r.path}</div>
      </div>
    </div>

    <div class="row" style="margin-top:18px">
      <span class="spacer"></span>
      <button class="btn" onclick={() => revealInDir(r.path)}>Reveal in file manager</button>
      <button class="btn btn--primary" onclick={() => store.dispatch({ t: "NavigateTo", screen: "home" })}>Back to home</button>
    </div>
  {:else}
    <div class="page__header">
      <div>
        <div class="page__eyebrow">§3 · UPLOAD · QUEUED</div>
        <h1 class="page__title">Upload queued</h1>
        <p class="page__lede">Document validated and sent to pathbase.dev.</p>
      </div>
      <span class="tag tag--accent">↗ Queued</span>
    </div>

    {#if r.stub}
      <div class="notice" style="margin-bottom:14px">
        Pathbase is not live yet — this is a stubbed response. The document
        validated cleanly; the real upload will drop in when the API ships.
      </div>
    {/if}

    <div class="card-panel">
      <div class="meta-table">
        <div class="meta-table__k">Source</div>
        <div class="meta-table__v">{r.source}</div>
        <div class="meta-table__k">URL</div>
        <div class="meta-table__v">{r.url}</div>
      </div>
    </div>

    <div class="row" style="margin-top:18px">
      <span class="spacer"></span>
      <button class="btn" onclick={() => openUrl(r.url)}>Open URL</button>
      <button class="btn btn--primary" onclick={() => store.dispatch({ t: "NavigateTo", screen: "home" })}>Back to home</button>
    </div>
  {/if}
</div>
