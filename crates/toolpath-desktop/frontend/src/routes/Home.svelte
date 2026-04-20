<script lang="ts">
    import { store } from "../lib/store.svelte";
    import SourceLogo from "../lib/SourceLogo.svelte";
    import type { Route } from "../lib/types";

    // Four source types, mirroring the cartographic source-picker tabs in the
    // design. Each tab kicks off its own browse flow (live backend data).
    type SourceKind = "claude" | "pi" | "git" | "github";
    const SOURCES: {
        k: SourceKind;
        label: string;
        route: Route;
        desc: string;
    }[] = [
        {
            k: "claude",
            label: "Claude Code",
            route: "browse-claude",
            desc: "Your local Claude Code sessions in ~/.claude/projects/.",
        },
        {
            k: "pi",
            label: "Pi.dev",
            route: "browse-pi",
            desc: "Pi agent conversations tracked on this machine.",
        },
        {
            k: "git",
            label: "Git",
            route: "browse-git",
            desc: "A local repository; branch history becomes the trace.",
        },
        {
            k: "github",
            label: "GitHub PR",
            route: "browse-github",
            desc: "A pull request URL. Fetches commits, reviews, and CI.",
        },
    ];

    let active: SourceKind = $state("claude");
</script>

<div class="page">
    <!-- Source-type tabs -->
    <div class="section-label">
        <span class="section-label__num">§1 ·</span>
        <span class="section-label__text">Select a source</span>
        <span class="section-label__right">path list {active}</span>
    </div>

    <div class="source-tabs" role="tablist">
        {#each SOURCES as s (s.k)}
            <button
                role="tab"
                aria-selected={active === s.k}
                class={"source-tabs__item" +
                    (active === s.k ? " source-tabs__item--active" : "")}
                onclick={() => {
                    active = s.k;
                }}
            >
                <span class="source-tabs__glyph">
                    <SourceLogo kind={s.k} size={18} />
                </span>
                <span class="source-tabs__label">{s.label}</span>
                <span class="source-tabs__count">LIVE</span>
            </button>
        {/each}
    </div>

    <!-- Selected-source introduction card + continue button -->
    {#each SOURCES as s (s.k)}
        {#if active === s.k}
            <div class="card-panel" style="margin-top:0; border-top:0">
                <div class="row" style="align-items:flex-start; gap:16px">
                    <div style="flex:1">
                        <div class="section-label" style="margin-bottom:6px">
                            <span class="section-label__num"
                                >§1.{SOURCES.indexOf(s) + 1} ·</span
                            >
                            <span class="section-label__text">{s.label}</span>
                            <span class="section-label__right"
                                >sourced live</span
                            >
                        </div>
                        <p
                            style="font-family:var(--font-serif); font-size:14px; color:var(--ink-2); margin:0 0 8px"
                        >
                            {s.desc}
                        </p>
                        <p
                            style="font-family:var(--font-mono); font-size:11px; color:var(--ink-4); letter-spacing:0.06em; margin:0"
                        >
                            ⎇ derived via <code>path derive {s.k}</code>
                        </p>
                    </div>
                    <button
                        class="btn btn--accent"
                        onclick={() =>
                            store.dispatch({
                                t: "NavigateTo",
                                screen: s.route,
                            })}>Browse {s.label} →</button
                    >
                </div>
            </div>
        {/if}
    {/each}
</div>
