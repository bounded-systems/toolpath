<script lang="ts">
  import { store } from "../lib/store.svelte";
  import type { AgentStatus } from "../lib/types";

  function statusLabel(s: AgentStatus): string {
    switch (s) {
      case "available": return "Installed";
      case "unavailable": return "Not detected";
      case "coming-soon": return "Coming soon";
    }
  }
</script>

<div class="row">
  <button class="linklike" onclick={() => store.dispatch({ t: "NavigateTo", screen: "home" })}>← Back</button>
</div>
<h1>Agents</h1>
<p class="subtitle">
  {store.m.agents.loading ? "Detecting installed agents… " : "Select an agent to browse its traces."}
  {#if store.m.agents.loading}<span class="spinner"></span>{/if}
</p>

{#if store.m.agents.list === null}
  <!-- first fetch in-flight; nothing to show yet -->
{:else if store.m.agents.list.length === 0}
  <div class="notice">No agents known.</div>
{:else}
  <div class="cards">
    {#each store.m.agents.list as agent (agent.id)}
      {@const clickable = agent.status === "available"}
      <div
        class={"card" + (clickable ? "" : " card--disabled")}
        role="button"
        tabindex={clickable ? 0 : -1}
        onclick={clickable ? () => store.dispatch({ t: "AgentsSelect", agent }) : undefined}
      >
        <div class="row">
          <div class="card__title">{agent.name}</div>
          <div class="spacer"></div>
          <span class={"badge badge--" + agent.status}>{statusLabel(agent.status)}</span>
        </div>
        <div class="card__desc">{agent.tagline ?? ""}</div>
        {#if agent.reason}
          <div class="card__hint">{agent.reason}</div>
        {/if}
      </div>
    {/each}
  </div>
{/if}
