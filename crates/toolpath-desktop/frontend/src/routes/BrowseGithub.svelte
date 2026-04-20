<script lang="ts">
  import { store } from "../lib/store.svelte";
  const gh = $derived(store.m.github);
</script>

<div class="row">
  <button class="linklike" onclick={() => store.dispatch({ t: "NavigateTo", screen: "home" })}>← Back</button>
</div>
<h1>GitHub pull request</h1>
<p class="subtitle">Paste a PR URL. We fetch commits, reviews, and CI checks, then render the trace.</p>

{#if gh.hasToken === null}
  <div class="notice">Checking keychain… <span class="spinner"></span></div>
{:else if gh.editingToken || !gh.hasToken}
  <div class="stack">
    <label class="list__meta" for="token">GitHub personal access token</label>
    <div class="row">
      <input
        id="token"
        type="password"
        placeholder="ghp_…"
        value={gh.tokenInput}
        oninput={(ev) => store.dispatch({ t: "GithubSetTokenInput", value: (ev.target as HTMLInputElement).value })}
      />
      <button
        class="secondary"
        disabled={!gh.tokenInput.trim() || gh.savingToken}
        onclick={() => store.dispatch({ t: "GithubSaveToken" })}
      >{gh.savingToken ? "Saving…" : "Save to keychain"}</button>
      {#if gh.hasToken}
        <button class="linklike" onclick={() => store.dispatch({ t: "GithubEditToken", on: false })}>Cancel</button>
      {/if}
    </div>
    <div class="list__meta">Stored in your OS keychain under <span class="kbd">dev.pathbase.toolpath-desktop</span>.</div>
  </div>
{:else}
  <div class="notice">
    GitHub token is configured.
    <button class="linklike" onclick={() => store.dispatch({ t: "GithubEditToken", on: true })}>Update</button>
    <button class="linklike" onclick={() => store.dispatch({ t: "GithubClearToken" })}>Remove</button>
  </div>
{/if}

<div class="stack" style="margin-top:14px">
  <label class="list__meta" for="pr-url">Pull request URL</label>
  <input
    id="pr-url"
    type="url"
    placeholder="https://github.com/owner/repo/pull/42"
    value={gh.url}
    oninput={(ev) => store.dispatch({ t: "GithubSetUrl", value: (ev.target as HTMLInputElement).value })}
  />
  <div class="row">
    <label>
      <input
        type="checkbox"
        class="checkbox"
        checked={gh.includeCi}
        onchange={() => store.dispatch({ t: "GithubToggleIncludeCi" })}
      />
      Include CI checks
    </label>
    <label>
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

<div class="row" style="margin-top:14px">
  <div class="spacer"></div>
  <button
    class="primary"
    disabled={!gh.url || !gh.hasToken || gh.deriving}
    onclick={() => store.dispatch({ t: "GithubDerive" })}
  >{gh.deriving ? "Fetching…" : "Preview"}</button>
</div>
