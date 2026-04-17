<script lang="ts">
  import { onMount } from "svelte";
  import { invoke, listen } from "../lib/ipc";
  import { store } from "../lib/store.svelte";
  import { dbg } from "../lib/debug";
  import type { PiProject, PiSession } from "../lib/types";

  // Permanent subscriptions — `onMount` runs exactly once per component
  // mount, so unlike `$effect` there's no way for Svelte's reactivity
  // graph to re-fire it and stack duplicate listeners.
  let subscribed: Promise<void> | null = null;
  let active = true;
  const pendingSessionStreams = new Set<string>();

  onMount(() => {
    dbg("browse-pi", "mount");
    active = true;
    const unlistens: Promise<() => void>[] = [
      listen<PiProject>("pi:project", (p) => { if (active) store.dispatch({ t: "PiProjectReceived", project: p }); }),
      listen<unknown>("pi:projects-done", () => { if (active) store.dispatch({ t: "PiProjectsDone" }); }),
      listen<string>("pi:projects-error", (e) => { if (active) store.dispatch({ t: "PiProjectsError", error: String(e) }); }),
      listen<PiSession>("pi:session", (s) => { if (active) store.dispatch({ t: "PiSessionReceived", session: s }); }),
      listen<string>("pi:sessions-done", (path) => { if (active) { pendingSessionStreams.delete(path); store.dispatch({ t: "PiSessionsDone", path }); } }),
      listen<string>("pi:sessions-error", (e) => { if (active) store.dispatch({ t: "Error", error: String(e) }); }),
    ];
    subscribed = Promise.all(unlistens).then(() => {
      dbg("browse-pi", "subs live");
    });
    return () => {
      dbg("browse-pi", "unmount");
      active = false;
      for (const u of unlistens) u.then((fn) => fn());
      subscribed = null;
    };
  });

  // Fire the projects invoke on the first transition of loadingProjects
  // into `true`. The effect re-runs on every dispatch (Svelte deep
  // reactivity replaces the proxy tree), but the guard means we only
  // invoke once per load cycle.
  let projectsInvoked = false;
  $effect(() => {
    const loading = store.m.pi.loadingProjects;
    if (loading && !projectsInvoked) {
      projectsInvoked = true;
      (async () => {
        try {
          if (subscribed) await subscribed;
          await invoke("list_pi_projects_stream");
        } catch (e) {
          store.dispatch({ t: "PiProjectsError", error: String(e) });
        }
      })();
    } else if (!loading) {
      projectsInvoked = false;
    }
  });

  // Per-project session streams: dedupe by path. Fires the invoke when a
  // path first appears with loading=true. `pendingSessionStreams` is
  // declared at the top of the script with the other mount-lifetime state.
  $effect(() => {
    const loadingMap = store.m.pi.sessionsLoading;
    for (const path of Object.keys(loadingMap)) {
      if (!loadingMap[path] || pendingSessionStreams.has(path)) continue;
      pendingSessionStreams.add(path);
      (async () => {
        try {
          if (subscribed) await subscribed;
          await invoke("list_pi_sessions_stream", { projectPath: path });
        } catch (e) {
          pendingSessionStreams.delete(path);
          store.dispatch({ t: "Error", error: String(e) });
        }
      })();
    }
  });

  function fmtTime(iso: string | null): string {
    if (!iso) return "—";
    const d = new Date(iso);
    if (isNaN(d.getTime())) return iso;
    const diff = (Date.now() - d.getTime()) / 1000;
    if (diff < 60) return Math.round(diff) + "s ago";
    if (diff < 3600) return Math.round(diff / 60) + "m ago";
    if (diff < 86400) return Math.round(diff / 3600) + "h ago";
    if (diff < 86400 * 7) return Math.round(diff / 86400) + "d ago";
    return d.toISOString().slice(0, 10);
  }

  const pi = $derived(store.m.pi);
  const projectCount = $derived(pi.projects.length);
  const selectedCount = $derived(
    Object.values(pi.selected).reduce((acc, s) => acc + Object.keys(s || {}).length, 0),
  );
</script>

<div class="row">
  <button class="linklike" onclick={() => store.dispatch({ t: "NavigateTo", screen: "browse-agents" })}>← Back</button>
</div>
<h1>pi.dev sessions</h1>
<p class="subtitle">
  {#if projectCount === 0 && pi.loadingProjects}
    Scanning ~/.pi/agent/sessions/…
  {:else}
    {projectCount} project{projectCount === 1 ? "" : "s"}{pi.loadingProjects ? " — still scanning… " : " "}
  {/if}
  {#if pi.loadingProjects}<span class="spinner"></span>{/if}
</p>

{#if projectCount === 0 && pi.projectsDone}
  <div class="notice">No pi.dev projects found. Run a pi.dev session and come back.</div>
{:else}
  <div class="list">
    {#each pi.projects as p (p.project_path)}
      {@const isExpanded = pi.expanded === p.project_path}
      {@const selectedForProject = Object.keys(pi.selected[p.project_path] ?? {}).length}
      <div>
        <div
          class={"list__item" + (isExpanded ? " list__item--expanded" : "")}
          role="button"
          tabindex="0"
          onclick={() => store.dispatch({ t: "PiExpandProject", path: p.project_path })}
        >
          <div class="list__title">{p.display_name}</div>
          <div class="spacer"></div>
          <div class="list__meta">
            {p.session_count} session{p.session_count === 1 ? "" : "s"}
            {#if selectedForProject > 0} · <strong>{selectedForProject} selected</strong>{/if}
          </div>
        </div>
        {#if isExpanded}
          {@const sessions = pi.sessionsByPath[p.project_path] ?? []}
          {@const loading = pi.sessionsLoading[p.project_path]}
          <div class="list__children">
            {#if sessions.length === 0 && loading}
              <div class="list__loading-hint">Loading sessions… <span class="spinner"></span></div>
            {:else if sessions.length === 0}
              <div class="notice">No sessions in this project.</div>
            {:else}
              {#each sessions as s (s.session_id)}
                {@const isChecked = !!(pi.selected[p.project_path] ?? {})[s.session_id]}
                <label
                  class="list__item"
                  onclick={(ev: MouseEvent) => {
                    if ((ev.target as HTMLElement).tagName !== "INPUT") {
                      ev.preventDefault();
                      store.dispatch({ t: "PiToggleSession", path: p.project_path, sid: s.session_id });
                    }
                  }}
                >
                  <input
                    type="checkbox"
                    class="checkbox"
                    checked={isChecked}
                    onclick={(ev: MouseEvent) => ev.stopPropagation()}
                    onchange={() => store.dispatch({ t: "PiToggleSession", path: p.project_path, sid: s.session_id })}
                  />
                  <div>
                    <div class="list__title">{s.session_id}</div>
                    <div class="list__meta">
                      {s.entry_count} entr{s.entry_count === 1 ? "y" : "ies"} · {fmtTime(s.timestamp)}
                    </div>
                  </div>
                </label>
              {/each}
              {#if loading}
                <div class="list__loading-hint">Still loading… <span class="spinner"></span></div>
              {/if}
            {/if}
          </div>
        {/if}
      </div>
    {/each}
  </div>
{/if}

<div class="row" style="margin-top:14px">
  <div class="spacer"></div>
  <span class="list__meta">
    {selectedCount || "No"} session{selectedCount === 1 ? "" : "s"} selected
  </span>
  <button
    class="primary"
    disabled={!selectedCount || pi.deriving}
    onclick={() => store.dispatch({ t: "PiDerive" })}
  >{pi.deriving ? "Deriving…" : "Preview"}</button>
</div>
