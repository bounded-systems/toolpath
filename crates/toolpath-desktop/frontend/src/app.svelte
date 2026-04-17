<script lang="ts">
  import { store } from "./lib/store.svelte";
  import Home from "./routes/Home.svelte";
  import BrowseAgents from "./routes/BrowseAgents.svelte";
  import BrowseClaude from "./routes/BrowseClaude.svelte";
  import BrowsePi from "./routes/BrowsePi.svelte";
  import BrowseGit from "./routes/BrowseGit.svelte";
  import BrowseGithub from "./routes/BrowseGithub.svelte";
  import Preview from "./routes/Preview.svelte";
  import Result from "./routes/Result.svelte";

  // `__TAURI_INTERNALS__` is injected by Tauri's webview. In a plain browser
  // tab (e.g. the Vite dev server at localhost:1420 opened manually for
  // CSS/DevTools work) it's undefined, every IPC call errors, and every
  // screen looks broken. Detect that once and surface a clear banner.
  const notInTauri =
    typeof window !== "undefined" &&
    !(window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
</script>

{#if notInTauri}
  <div class="banner banner--warn">
    <strong>Not a Tauri window.</strong>
    This page is the Vite dev server; IPC calls won't work. Use the native
    window opened by <code>cargo tauri dev</code>.
  </div>
{/if}

<header class="appbar">
  <div class="appbar__brand">
    <span class="appbar__logo" aria-hidden="true"></span>
    <strong>Toolpath</strong>
    <span class="appbar__tag">Pathbase companion</span>
  </div>
  <nav class="appbar__nav">
    <button
      class="linklike"
      onclick={() => store.dispatch({ t: "NavigateTo", screen: "home" })}
    >Home</button>
  </nav>
</header>

<main class="screen">
  {#if store.m.error}
    <div class="error">
      {store.m.error}
      <button class="linklike" onclick={() => store.dispatch({ t: "ClearError" })}>dismiss</button>
    </div>
  {/if}

  {#if store.m.route === "home"}
    <Home />
  {:else if store.m.route === "browse-agents"}
    <BrowseAgents />
  {:else if store.m.route === "browse-claude"}
    <BrowseClaude />
  {:else if store.m.route === "browse-pi"}
    <BrowsePi />
  {:else if store.m.route === "browse-git"}
    <BrowseGit />
  {:else if store.m.route === "browse-github"}
    <BrowseGithub />
  {:else if store.m.route === "preview"}
    <Preview />
  {:else if store.m.route === "result"}
    <Result />
  {/if}
</main>
