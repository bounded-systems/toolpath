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

{#if !r}
  <p>No result.</p>
{:else if r.kind === "export"}
  <h1>Saved</h1>
  <p class="subtitle">Your trace was written to disk.</p>
  <dl style="display:grid;grid-template-columns:auto 1fr;gap:6px 14px;margin:14px 0">
    <dt class="list__meta">Source</dt><dd>{r.source}</dd>
    <dt class="list__meta">Path</dt><dd><code>{r.path}</code></dd>
  </dl>
  <div class="row">
    <button class="secondary" onclick={() => revealInDir(r.path)}>Reveal in file manager</button>
    <button class="primary" onclick={() => store.dispatch({ t: "NavigateTo", screen: "home" })}>Back to home</button>
  </div>
{:else}
  <h1>Upload queued</h1>
  {#if r.stub}
    <div class="notice">
      Pathbase is not live yet — this is a stubbed response. The document validated cleanly;
      the real upload will drop in when the API ships.
    </div>
  {/if}
  <dl style="display:grid;grid-template-columns:auto 1fr;gap:6px 14px;margin:14px 0">
    <dt class="list__meta">Source</dt><dd>{r.source}</dd>
    <dt class="list__meta">URL</dt><dd><code>{r.url}</code></dd>
  </dl>
  <div class="row">
    <button class="secondary" onclick={() => openUrl(r.url)}>Open URL</button>
    <button class="primary" onclick={() => store.dispatch({ t: "NavigateTo", screen: "home" })}>Back to home</button>
  </div>
{/if}
