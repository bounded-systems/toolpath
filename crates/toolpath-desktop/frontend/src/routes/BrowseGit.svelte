<script lang="ts">
  import { store } from "../lib/store.svelte";
  const git = $derived(store.m.git);
</script>

<div class="row">
  <button class="linklike" onclick={() => store.dispatch({ t: "NavigateTo", screen: "home" })}>← Back</button>
</div>
<h1>Local git repository</h1>
<p class="subtitle">Point to a repo and pick a branch. History on that branch becomes the trace.</p>

<div class="stack">
  <label class="list__meta" for="repo-path">Repo path</label>
  <div class="row">
    <input
      id="repo-path"
      type="text"
      placeholder="/Users/you/project"
      value={git.repoPath}
      oninput={(ev) => store.dispatch({ t: "GitSetRepoPath", value: (ev.target as HTMLInputElement).value })}
      onkeydown={(ev) => { if (ev.key === "Enter") store.dispatch({ t: "GitLoadBranches" }); }}
    />
    <button class="secondary" onclick={() => store.dispatch({ t: "GitPickRepo" })}>Browse…</button>
    <button
      class="secondary"
      disabled={!git.repoPath || git.loading}
      onclick={() => store.dispatch({ t: "GitLoadBranches" })}
    >{git.loading ? "Loading…" : "Load branches"}</button>
  </div>
</div>

{#if git.branches === null}
  <div class="notice" style="margin-top:14px">No branches loaded yet.</div>
{:else if git.branches.length === 0}
  <div class="notice" style="margin-top:14px">No branches found in this repo.</div>
{:else}
  <div class="list" style="margin-top:14px">
    {#each git.branches as b (b.name)}
      <div
        class={"list__item" + (git.selected === b.name ? " list__item--selected" : "")}
        role="button"
        tabindex="0"
        onclick={() => store.dispatch({ t: "GitSelectBranch", name: b.name })}
      >
        <div>
          <div class="list__title">{b.name}</div>
          <div class="list__meta">{b.subject || "(no subject)"}</div>
        </div>
        <div class="spacer"></div>
        <div class="list__meta">{b.head_short} · {b.author}</div>
      </div>
    {/each}
  </div>
{/if}

<div class="row" style="margin-top:14px">
  <div class="spacer"></div>
  <button
    class="primary"
    disabled={!git.selected || git.deriving}
    onclick={() => store.dispatch({ t: "GitDerive" })}
  >{git.deriving ? "Deriving…" : "Preview"}</button>
</div>
