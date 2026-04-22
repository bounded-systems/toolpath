<script lang="ts">
    import SourceLogo from "../lib/SourceLogo.svelte";
    import BrowseClaude from "./BrowseClaude.svelte";
    import BrowsePi from "./BrowsePi.svelte";
    import BrowseGit from "./BrowseGit.svelte";
    import BrowseGithub from "./BrowseGithub.svelte";

    // Four source types, mirroring the cartographic source-picker tabs in the
    // design. Each tab reveals the live browse view for that source.
    type SourceKind = "claude" | "pi" | "git" | "github";
    const SOURCES: { k: SourceKind; label: string }[] = [
        { k: "claude", label: "Claude Code" },
        { k: "pi", label: "Pi.dev" },
        { k: "git", label: "Git" },
        { k: "github", label: "GitHub PR" },
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

    <!-- Inline browse view for the active source -->
    {#if active === "claude"}
        <BrowseClaude embedded />
    {:else if active === "pi"}
        <BrowsePi embedded />
    {:else if active === "git"}
        <BrowseGit embedded />
    {:else if active === "github"}
        <BrowseGithub embedded />
    {/if}
</div>
