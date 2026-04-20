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
  import type { Route } from "./lib/types";

  const notInTauri =
    typeof window !== "undefined" &&
    !(window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;

  // Primary nav. §-numbers match the cartographic eyebrow styling.
  const TABS: { key: Route; num: string; label: string }[] = [
    { key: "home",     num: "1", label: "New upload" },
    { key: "preview",  num: "2", label: "Preview" },
    { key: "result",   num: "3", label: "Result" },
  ];

  function titleFor(r: Route): string {
    if (r === "home") return "New upload";
    if (r === "browse-agents") return "Agents";
    if (r === "browse-claude") return "Claude Code";
    if (r === "browse-pi") return "Pi.dev";
    if (r === "browse-git") return "Local git";
    if (r === "browse-github") return "GitHub PR";
    if (r === "preview") return "Preview";
    if (r === "result") return "Result";
    return "";
  }

  // "Home" is selected whenever we're on home or in any browse sub-flow.
  function activeTab(r: Route): Route {
    if (r.startsWith("browse-")) return "home";
    return r;
  }
</script>

<div class="backdrop"></div>

<div class="window">
  <!-- Title bar -->
  <div class="window__title">
    <div class="window__dots">
      <span class="window__dot"></span>
      <span class="window__dot"></span>
      <span class="window__dot"></span>
      <span class="window__brand">△ TOOLPATH · v2.4</span>
    </div>
    <div class="window__title-center">
      <span>{titleFor(store.m.route)}</span>
      {#if store.m.preview}
        <span class="sep">·</span>
        <span style="color:var(--ink-4); letter-spacing:0.14em">{store.m.preview.source}</span>
      {/if}
    </div>
    <div class="window__title-right">
      <span>§1.0 · FA·FV</span>
      <span class="elevation">△473·8</span>
    </div>
  </div>

  <!-- Primary tabs -->
  <nav class="tabs" aria-label="Primary">
    {#each TABS as t (t.key)}
      {@const isActive = activeTab(store.m.route) === t.key}
      {@const disabled = (t.key === "preview" && !store.m.preview) || (t.key === "result" && !store.m.result)}
      <button
        class={"tabs__item" + (isActive ? " tabs__item--active" : "")}
        disabled={disabled}
        style={disabled ? "opacity:0.4;cursor:not-allowed" : ""}
        onclick={() => !disabled && store.dispatch({ t: "NavigateTo", screen: t.key })}
      >
        <span class="tabs__num">§{t.num}</span>
        <span>{t.label}</span>
      </button>
    {/each}
  </nav>

  <!-- Main scroll area -->
  <main class="main">
    {#if notInTauri}
      <div class="banner">
        <strong>Not a Tauri window.</strong>
        This page is the Vite dev server; IPC calls won't work. Use the native window opened by <code>cargo tauri dev</code>.
      </div>
    {/if}

    {#if store.m.error}
      <div class="page" style="padding-bottom:0">
        <div class="error">
          <span>{store.m.error}</span>
          <span class="spacer"></span>
          <button class="btn btn--sm" onclick={() => store.dispatch({ t: "ClearError" })}>Dismiss</button>
        </div>
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

  <!-- Status bar -->
  <footer class="status-bar">
    <div class="status-bar__items">
      <span class="status-bar__item">
        <span class="status-bar__k">CLI</span>
        <span class="status-bar__v">path v2.4</span>
      </span>
      <span class="status-bar__sep">·</span>
      <span class="status-bar__item">
        <span class="status-bar__k">Endpoint</span>
        <span class="status-bar__v">pathbase.dev</span>
      </span>
      {#if store.m.preview}
        <span class="status-bar__sep">·</span>
        <span class="status-bar__item">
          <span class="status-bar__k">Doc</span>
          <span class="status-bar__v">{store.m.preview.filename}</span>
        </span>
      {/if}
    </div>
    <div class="status-bar__right">
      <span>◦ ◦ ◦</span>
      <span class="status-bar__sep">·</span>
      <span>v2.4</span>
    </div>
  </footer>
</div>
