<script lang="ts">
  import { store } from "../lib/store.svelte";
  import type { AgentStatus } from "../lib/types";

  function statusLabel(s: AgentStatus): string {
    if (s === "available") return "Installed";
    if (s === "unavailable") return "Not detected";
    return "Coming soon";
  }
  function statusTag(s: AgentStatus): string {
    if (s === "available") return "tag tag--ok";
    if (s === "unavailable") return "tag tag--muted";
    return "tag tag--warn";
  }
</script>

<div class="page">
  <div class="row" style="margin-bottom:14px">
    <button class="btn btn--ghost" onclick={() => store.dispatch({ t: "NavigateTo", screen: "home" })}>← Back</button>
  </div>

  <div class="page__header">
    <div>
      <div class="page__eyebrow">§1.A · AGENTS</div>
      <h1 class="page__title">AI coding agents</h1>
      <p class="page__lede">
        {store.m.agents.loading ? "Detecting installed agents…" : "Select an installed agent to browse its sessions."}
        {#if store.m.agents.loading}<span class="spinner"></span>{/if}
      </p>
    </div>
  </div>

  {#if store.m.agents.list === null}
    <!-- first fetch in-flight -->
  {:else if store.m.agents.list.length === 0}
    <div class="notice">No agents known.</div>
  {:else}
    <div style="border:0.5px solid var(--ink-5); background:var(--paper-bright)">
      {#each store.m.agents.list as agent (agent.id)}
        {@const clickable = agent.status === "available"}
        <button
          class={"row-card" + (clickable ? "" : "")}
          disabled={!clickable}
          style={!clickable ? "opacity:0.55; cursor:not-allowed" : ""}
          onclick={() => clickable && store.dispatch({ t: "AgentsSelect", agent })}
        >
          <span class="row-card__marker">△</span>
          <div style="min-width:0">
            <div class="row-card__title">{agent.name}</div>
            <div class="row-card__sub">{agent.tagline ?? ""}</div>
            {#if agent.reason}
              <div class="row-card__meta"><span>{agent.reason}</span></div>
            {/if}
          </div>
          <span class={statusTag(agent.status)}>{statusLabel(agent.status)}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>
