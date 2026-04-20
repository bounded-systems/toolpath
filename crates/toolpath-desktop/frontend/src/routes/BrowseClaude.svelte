<script lang="ts">
  import { invoke, listen } from "../lib/ipc";
  import { store } from "../lib/store.svelte";
  import type { ClaudeProject, ClaudeSession } from "../lib/types";

  // Permanent subscriptions on mount. We avoid reactive reads inside this
  // effect so it never re-runs — every dispatch replaces `store.m` under
  // Svelte 5's deep `$state`, which would otherwise tear down listeners
  // between events and lose them.
  let subscribed: Promise<void> | null = $state(null);
  $effect(() => {
    let active = true;
    const unlistens: Promise<() => void>[] = [
      listen<ClaudeProject>("claude:project", (p) => { if (active) store.dispatch({ t: "ClaudeProjectReceived", project: p }); }),
      listen<unknown>("claude:projects-done", () => { if (active) store.dispatch({ t: "ClaudeProjectsDone" }); }),
      listen<string>("claude:projects-error", (e) => { if (active) store.dispatch({ t: "ClaudeProjectsError", error: String(e) }); }),
      listen<ClaudeSession>("claude:session", (s) => { if (active) store.dispatch({ t: "ClaudeSessionReceived", session: s }); }),
      listen<string>("claude:sessions-done", (path) => { if (active) { pendingSessionStreams.delete(path); store.dispatch({ t: "ClaudeSessionsDone", path }); } }),
      listen<string>("claude:sessions-error", (e) => { if (active) store.dispatch({ t: "Error", error: String(e) }); }),
    ];
    subscribed = Promise.all(unlistens).then(() => {});
    return () => {
      active = false;
      for (const u of unlistens) u.then((fn) => fn());
      subscribed = null;
    };
  });

  let projectsInvoked = false;
  $effect(() => {
    const loading = store.m.claude.loadingProjects;
    if (loading && !projectsInvoked) {
      projectsInvoked = true;
      (async () => {
        try {
          if (subscribed) await subscribed;
          await invoke("list_claude_projects_stream");
        } catch (e) {
          store.dispatch({ t: "ClaudeProjectsError", error: String(e) });
        }
      })();
    } else if (!loading) {
      projectsInvoked = false;
    }
  });

  const pendingSessionStreams = new Set<string>();
  $effect(() => {
    const loadingMap = store.m.claude.sessionsLoading;
    for (const path of Object.keys(loadingMap)) {
      if (!loadingMap[path] || pendingSessionStreams.has(path)) continue;
      pendingSessionStreams.add(path);
      (async () => {
        try {
          if (subscribed) await subscribed;
          await invoke("list_claude_sessions_stream", { projectPath: path });
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

  const claude = $derived(store.m.claude);
  const projectCount = $derived(claude.projects.length);
  const selectedCount = $derived(
    Object.values(claude.selected).reduce((acc, s) => acc + Object.keys(s || {}).length, 0),
  );
</script>

<div class="row">
  <button class="linklike" onclick={() => store.dispatch({ t: "NavigateTo", screen: "browse-agents" })}>← Back</button>
</div>
<h1>Claude Code sessions</h1>
<p class="subtitle">
  {#if projectCount === 0 && claude.loadingProjects}
    Scanning ~/.claude/projects/…
  {:else}
    {projectCount} project{projectCount === 1 ? "" : "s"}{claude.loadingProjects ? " — still scanning… " : " "}
  {/if}
  {#if claude.loadingProjects}<span class="spinner"></span>{/if}
</p>

{#if projectCount === 0 && claude.projectsDone}
  <div class="notice">No Claude projects found. Use Claude Code at least once and come back.</div>
{:else}
  <div class="list">
    {#each claude.projects as p (p.project_path)}
      {@const isExpanded = claude.expanded === p.project_path}
      {@const selectedForProject = Object.keys(claude.selected[p.project_path] ?? {}).length}
      <div>
        <div
          class={"list__item" + (isExpanded ? " list__item--expanded" : "")}
          role="button"
          tabindex="0"
          onclick={() => store.dispatch({ t: "ClaudeExpandProject", path: p.project_path })}
        >
          <div class="list__title">{p.display_name}</div>
          <div class="spacer"></div>
          <div class="list__meta">
            {p.session_count} session{p.session_count === 1 ? "" : "s"}
            {#if selectedForProject > 0} · <strong>{selectedForProject} selected</strong>{/if}
          </div>
        </div>
        {#if isExpanded}
          {@const sessions = claude.sessionsByPath[p.project_path] ?? []}
          {@const loading = claude.sessionsLoading[p.project_path]}
          <div class="list__children">
            {#if sessions.length === 0 && loading}
              <div class="list__loading-hint">Loading sessions… <span class="spinner"></span></div>
            {:else if sessions.length === 0}
              <div class="notice">No sessions in this project.</div>
            {:else}
              {#each sessions as s (s.session_id)}
                {@const isChecked = !!(claude.selected[p.project_path] ?? {})[s.session_id]}
                {@const title = claude.titles[`${p.project_path}|${s.session_id}`]}
                <label
                  class="list__item"
                  onclick={(ev: MouseEvent) => {
                    if ((ev.target as HTMLElement).tagName !== "INPUT") {
                      ev.preventDefault();
                      store.dispatch({ t: "ClaudeToggleSession", path: p.project_path, sid: s.session_id });
                    }
                  }}
                >
                  <input
                    type="checkbox"
                    class="checkbox"
                    checked={isChecked}
                    onclick={(ev: MouseEvent) => ev.stopPropagation()}
                    onchange={() => store.dispatch({ t: "ClaudeToggleSession", path: p.project_path, sid: s.session_id })}
                  />
                  <div>
                    <div class="list__title">
                      {#if title}{title}{:else}<span class="list__meta">loading title…</span>{/if}
                    </div>
                    <div class="list__meta">
                      {s.turn_count} turn{s.turn_count === 1 ? "" : "s"} · {fmtTime(s.last_activity)} · {s.session_id.slice(0, 8)}
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
    disabled={!selectedCount || claude.deriving}
    onclick={() => store.dispatch({ t: "ClaudeDerive" })}
  >{claude.deriving ? "Deriving…" : "Preview"}</button>
</div>
