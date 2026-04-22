<script lang="ts">
  import { onMount } from "svelte";
  import { store } from "../lib/store.svelte";
  let { embedded = false }: { embedded?: boolean } = $props();

  // When embedded, there's no NavigateTo("browse-github") to probe the
  // keychain. Do it on mount; idempotent.
  onMount(() => {
    store.dispatch({ t: "GithubEnsureTokenStatus" });
  });

  const gh = $derived(store.m.github);
</script>

<div class:page={!embedded}>
  {#if !embedded}
    <div class="row" style="margin-bottom:14px">
      <button class="btn btn--ghost" onclick={() => store.dispatch({ t: "NavigateTo", screen: "home" })}>← Back</button>
    </div>

    <div class="page__header">
      <div>
        <div class="page__eyebrow">§1.4 · GITHUB · PULL REQUEST</div>
        <h1 class="page__title">GitHub pull request</h1>
        <p class="page__lede">
          Paste a PR URL. We fetch commits, reviews, and CI checks, then render
          the derived trace.
        </p>
      </div>
    </div>
  {/if}

  <div class="section-label">
    <span class="section-label__num">§1.4.1 ·</span>
    <span class="section-label__text">Token</span>
    <span class="section-label__right">OS keychain</span>
  </div>

  {#if gh.hasToken === null}
    <div class="notice">Checking keychain… <span class="spinner"></span></div>
  {:else if gh.editingToken || !gh.hasToken}
    <div class="card-panel">
      <label class="page__eyebrow" for="token">GitHub personal access token</label>
      <div class="row" style="margin-top:6px">
        <input
          class="input"
          id="token"
          type="password"
          placeholder="ghp_…"
          value={gh.tokenInput}
          oninput={(ev) => store.dispatch({ t: "GithubSetTokenInput", value: (ev.target as HTMLInputElement).value })}
        />
        <button
          class="btn btn--primary"
          disabled={!gh.tokenInput.trim() || gh.savingToken}
          onclick={() => store.dispatch({ t: "GithubSaveToken" })}
        >{gh.savingToken ? "Saving…" : "Save to keychain"}</button>
        {#if gh.hasToken}
          <button class="btn btn--ghost" onclick={() => store.dispatch({ t: "GithubEditToken", on: false })}>Cancel</button>
        {/if}
      </div>
      <div style="font-family:var(--font-mono); font-size:10.5px; color:var(--ink-4); letter-spacing:0.06em; margin-top:8px">
        Stored under <span class="kbd">dev.pathbase.toolpath-desktop</span>.
      </div>
    </div>
  {:else}
    <div class="card-panel" style="display:flex; align-items:center; gap:10px">
      <span class="tag tag--ok">◇ Token set</span>
      <span class="spacer"></span>
      <button class="btn btn--ghost" onclick={() => store.dispatch({ t: "GithubEditToken", on: true })}>Update</button>
      <button class="btn btn--ghost" onclick={() => store.dispatch({ t: "GithubClearToken" })}>Remove</button>
    </div>
  {/if}

  <div class="section-label" style="margin-top:22px">
    <span class="section-label__num">§1.4.2 ·</span>
    <span class="section-label__text">Pull request URL</span>
    <span class="section-label__right">path derive github</span>
  </div>

  <div class="card-panel">
    <input
      class="input"
      id="pr-url"
      type="url"
      placeholder="https://github.com/owner/repo/pull/42"
      value={gh.url}
      oninput={(ev) => store.dispatch({ t: "GithubSetUrl", value: (ev.target as HTMLInputElement).value })}
    />
    <div class="row" style="margin-top:10px; gap:18px">
      <label style="display:inline-flex; gap:6px; align-items:center; font-family:var(--font-mono); font-size:11px; color:var(--ink-3); letter-spacing:0.06em; text-transform:uppercase">
        <input
          type="checkbox"
          class="checkbox"
          checked={gh.includeCi}
          onchange={() => store.dispatch({ t: "GithubToggleIncludeCi" })}
        />
        Include CI checks
      </label>
      <label style="display:inline-flex; gap:6px; align-items:center; font-family:var(--font-mono); font-size:11px; color:var(--ink-3); letter-spacing:0.06em; text-transform:uppercase">
        <input
          type="checkbox"
          class="checkbox"
          checked={gh.includeComments}
          onchange={() => store.dispatch({ t: "GithubToggleComments" })}
        />
        Include reviews &amp; comments
      </label>
    </div>
  </div>

  <div class="row" style="margin-top:18px">
    <span class="spacer"></span>
    <button
      class="btn btn--accent"
      disabled={!gh.url || !gh.hasToken || gh.deriving}
      onclick={() => store.dispatch({ t: "GithubDerive" })}
    >{gh.deriving ? "Fetching…" : "Preview →"}</button>
  </div>
</div>
