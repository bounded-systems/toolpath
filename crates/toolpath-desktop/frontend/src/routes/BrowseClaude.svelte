<script lang="ts">
  import { onMount } from "svelte";
  import { invoke, listen } from "../lib/ipc";
  import { store } from "../lib/store.svelte";
  import SourceLogo from "../lib/SourceLogo.svelte";
  import type { ClaudeProject, ClaudeSession } from "../lib/types";

  let { embedded = false }: { embedded?: boolean } = $props();

  // When embedded (e.g. in Home), there's no NavigateTo("browse-claude") to
  // seed loadingProjects. Kick it off on mount; the msg is idempotent.
  onMount(() => {
    store.dispatch({ t: "ClaudeEnsureProjects" });
  });

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
</script>

<div class:page={!embedded}>
  {#if !embedded}
    <div class="row" style="margin-bottom:14px">
      <button class="btn btn--ghost" onclick={() => store.dispatch({ t: "NavigateTo", screen: "home" })}>← Back</button>
    </div>

    <div class="page__header">
      <div>
        <div class="page__eyebrow">§1.1 · CLAUDE CODE · SESSIONS</div>
        <h1 class="page__title">Claude Code</h1>
        <p class="page__lede">
          {#if projectCount === 0 && claude.loadingProjects}
            Scanning <code>~/.claude/projects/</code> …
          {:else}
            {projectCount} project{projectCount === 1 ? "" : "s"}{claude.loadingProjects ? " — still scanning …" : ""}
          {/if}
          {#if claude.loadingProjects}<span class="spinner"></span>{/if}
        </p>
      </div>
    </div>
  {/if}

  {#if projectCount === 0 && claude.projectsDone}
    <div class="notice">No Claude projects found. Use Claude Code at least once and come back.</div>
  {:else}
    <div style="border:0.5px solid var(--ink-5); background:var(--paper-bright)">
      {#each claude.projects as p (p.project_path)}
        {@const isExpanded = claude.expanded === p.project_path}
        {@const projectTitle = claude.projectTitles[p.project_path]}
        <div>
          <button
            class={"row-card" + (isExpanded ? " row-card--selected" : "")}
            onclick={() => store.dispatch({ t: "ClaudeExpandProject", path: p.project_path })}
          >
            <span class="row-card__marker"><SourceLogo kind="claude" size={14} /></span>
            <div style="min-width:0">
              <div class="row-card__title">{projectTitle ?? p.display_name}</div>
              <div class="row-card__sub">{p.project_path}</div>
            </div>
            <div class="row-card__right">
              {p.session_count} session{p.session_count === 1 ? "" : "s"}
            </div>
          </button>
          {#if isExpanded}
            {@const sessions = claude.sessionsByPath[p.project_path] ?? []}
            {@const loading = claude.sessionsLoading[p.project_path]}
            <div class="row-card__children">
              {#if sessions.length === 0 && loading}
                <div style="padding:12px 18px; font-family:var(--font-mono); font-size:11px; color:var(--ink-3); letter-spacing:0.05em">
                  Loading sessions… <span class="spinner"></span>
                </div>
              {:else if sessions.length === 0}
                <div style="padding:12px 18px"><div class="notice">No sessions in this project.</div></div>
              {:else}
                {#each sessions as s (s.session_id)}
                  {@const title = claude.titles[`${p.project_path}|${s.session_id}`]}
                  <div class="row-card" style="padding-left:36px; cursor:default">
                    <span class="row-card__marker"></span>
                    <div style="min-width:0">
                      <div class="row-card__title">
                        {#if title}{title}{:else}<span class="row-card__sub">loading title…</span>{/if}
                      </div>
                      <div class="row-card__meta">
                        <span>{s.turn_count} turn{s.turn_count === 1 ? "" : "s"}</span>
                        <span>{fmtTime(s.last_activity)}</span>
                        <span>{s.session_id.slice(0, 8)}</span>
                      </div>
                    </div>
                    <button
                      class="btn btn--accent btn--sm"
                      disabled={claude.deriving}
                      onclick={() => store.dispatch({ t: "ClaudeDerive", path: p.project_path, sid: s.session_id })}
                    >{claude.deriving ? "Deriving…" : "Select →"}</button>
                  </div>
                {/each}
                {#if loading}
                  <div style="padding:10px 18px; font-family:var(--font-mono); font-size:11px; color:var(--ink-3)">
                    Still loading… <span class="spinner"></span>
                  </div>
                {/if}
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>
