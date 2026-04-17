<script lang="ts">
  import { store } from "../lib/store.svelte";
  import type { Route } from "../lib/types";

  const cards: { id: Route; title: string; desc: string }[] = [
    { id: "browse-agents", title: "Agents", desc: "Browse sessions from AI coding agents on this machine." },
    { id: "browse-git", title: "Local git repository", desc: "Choose a folder and a branch. The commit history becomes the trace." },
    { id: "browse-github", title: "GitHub pull request", desc: "Paste a PR URL. Requires a GitHub personal access token." },
  ];
</script>

<h1>Pick a source</h1>
<p class="subtitle">
  Select where your agent trace is coming from. You'll be able to preview
  it before exporting or uploading to Pathbase.
</p>
<div class="cards">
  {#each cards as c (c.id)}
    <div class="card" onclick={() => store.dispatch({ t: "NavigateTo", screen: c.id })} role="button" tabindex="0">
      <div class="card__title">{c.title}</div>
      <div class="card__desc">{c.desc}</div>
    </div>
  {/each}
</div>
