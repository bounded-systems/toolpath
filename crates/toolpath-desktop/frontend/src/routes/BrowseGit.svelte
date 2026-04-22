<script lang="ts">
  import { store } from "../lib/store.svelte";
  import SourceLogo from "../lib/SourceLogo.svelte";
  let { embedded = false }: { embedded?: boolean } = $props();
  const git = $derived(store.m.git);
</script>

<div class:page={!embedded}>
  {#if !embedded}
    <div class="row" style="margin-bottom:14px">
      <button class="btn btn--ghost" onclick={() => store.dispatch({ t: "NavigateTo", screen: "home" })}>← Back</button>
    </div>

    <div class="page__header">
      <div>
        <div class="page__eyebrow">§1.3 · GIT · LOCAL REPOSITORY</div>
        <h1 class="page__title">Local git repository</h1>
        <p class="page__lede">
          Point to a repo and pick a branch. History on that branch becomes the
          derived Path document.
        </p>
      </div>
    </div>
  {/if}

  <div class="section-label">
    <span class="section-label__num">§1.3.1 ·</span>
    <span class="section-label__text">Source</span>
    <span class="section-label__right">path list git</span>
  </div>

  <div class="card-panel">
    <label class="page__eyebrow" for="repo-path">Repo path</label>
    <div class="row" style="margin-top:6px">
      <input
        class="input"
        id="repo-path"
        type="text"
        placeholder="/Users/you/project"
        value={git.repoPath}
        oninput={(ev) => store.dispatch({ t: "GitSetRepoPath", value: (ev.target as HTMLInputElement).value })}
        onkeydown={(ev) => { if (ev.key === "Enter") store.dispatch({ t: "GitLoadBranches" }); }}
      />
      <button class="btn" onclick={() => store.dispatch({ t: "GitPickRepo" })}>Browse…</button>
      <button
        class="btn btn--primary"
        disabled={!git.repoPath || git.loading}
        onclick={() => store.dispatch({ t: "GitLoadBranches" })}
      >{git.loading ? "Loading…" : "Load branches"}</button>
    </div>
  </div>

  {#if git.branches !== null && git.branches.length > 0}
    <div class="section-label" style="margin-top:22px">
      <span class="section-label__num">§1.3.2 ·</span>
      <span class="section-label__text">Branches</span>
      <span class="section-label__right">{git.branches.length} refs</span>
    </div>
    <div style="border:0.5px solid var(--ink-5); background:var(--paper-bright)">
      {#each git.branches as b (b.name)}
        <button
          class={"row-card" + (git.selected === b.name ? " row-card--selected" : "")}
          onclick={() => store.dispatch({ t: "GitSelectBranch", name: b.name })}
        >
          <span class="row-card__marker" style="color:#f05133"><SourceLogo kind="git" size={14} /></span>
          <div style="min-width:0">
            <div class="row-card__title">{b.name}</div>
            <div class="row-card__sub">{b.subject || "(no subject)"}</div>
            <div class="row-card__meta">
              <span>HEAD {b.head_short}</span>
              <span>@{b.author}</span>
            </div>
          </div>
          <span class="row-card__right">{b.timestamp}</span>
        </button>
      {/each}
    </div>
  {:else if git.branches !== null}
    <div class="notice" style="margin-top:18px">No branches found in this repo.</div>
  {:else}
    <div class="notice" style="margin-top:18px">No branches loaded yet.</div>
  {/if}

  <div class="row" style="margin-top:18px">
    <span class="spacer"></span>
    <button
      class="btn btn--accent"
      disabled={!git.selected || git.deriving}
      onclick={() => store.dispatch({ t: "GitDerive" })}
    >{git.deriving ? "Deriving…" : "Preview →"}</button>
  </div>
</div>
