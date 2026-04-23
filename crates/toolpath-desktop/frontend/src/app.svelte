<script lang="ts">
    import { store } from "./lib/store.svelte";
    import { listen } from "./lib/ipc";
    import type { UnlistenFn } from "@tauri-apps/api/event";
    import Home from "./routes/Home.svelte";
    import BrowseAgents from "./routes/BrowseAgents.svelte";
    import BrowseClaude from "./routes/BrowseClaude.svelte";
    import BrowsePi from "./routes/BrowsePi.svelte";
    import BrowseGit from "./routes/BrowseGit.svelte";
    import BrowseGithub from "./routes/BrowseGithub.svelte";
    import Preview from "./routes/Preview.svelte";
    import Result from "./routes/Result.svelte";
    import type { Document, Route } from "./lib/types";

    const notInTauri =
        typeof window !== "undefined" &&
        !(window as unknown as { __TAURI_INTERNALS__?: unknown })
            .__TAURI_INTERNALS__;

    // Primary nav. §-numbers match the cartographic eyebrow styling.
    const TABS: { key: Route; num: string; label: string }[] = [
        { key: "home", num: "1", label: "New upload" },
        { key: "preview", num: "2", label: "Preview" },
        { key: "result", num: "3", label: "Result" },
    ];

    // "Home" is selected whenever we're on home or in any browse sub-flow.
    function activeTab(r: Route): Route {
        if (r.startsWith("browse-")) return "home";
        return r;
    }

    // Theme — soft-graphite dark mode, persisted to localStorage and applied
    // via <html data-theme="dark">. Mirrors the Pathbase design system.
    type Theme = "light" | "dark";
    const THEME_KEY = "toolpath.theme";

    function readInitialTheme(): Theme {
        if (typeof window === "undefined") return "light";
        const saved = window.localStorage.getItem(THEME_KEY);
        if (saved === "light" || saved === "dark") return saved;
        return window.matchMedia?.("(prefers-color-scheme: dark)").matches
            ? "dark"
            : "light";
    }

    let theme = $state<Theme>(readInitialTheme());

    $effect(() => {
        if (typeof document === "undefined") return;
        document.documentElement.setAttribute("data-theme", theme);
        window.localStorage.setItem(THEME_KEY, theme);
    });

    function toggleTheme() {
        theme = theme === "dark" ? "light" : "dark";
    }

    // Quick View menu-bar → main window: when the user clicks a recent
    // session in the tray popover, the Rust side derives the doc and emits
    // `trace:opened` here. We route it through the standard DeriveSucceeded
    // flow so the reducer navigates to the preview route.
    interface TraceOpenedPayload {
        doc: Document;
        source: string;
        filename: string;
    }
    $effect(() => {
        let unlisten: UnlistenFn | undefined;
        listen<TraceOpenedPayload>("trace:opened", (payload) => {
            store.dispatch({
                t: "DeriveSucceeded",
                doc: payload.doc,
                source: payload.source,
                filename: payload.filename,
            });
        }).then((fn) => (unlisten = fn));
        return () => unlisten?.();
    });
</script>

<div class="backdrop"></div>

<div class="window">
    <!-- Primary tabs -->
    <nav class="tabs" aria-label="Primary">
        {#each TABS as t (t.key)}
            {@const isActive = activeTab(store.m.route) === t.key}
            {@const disabled =
                (t.key === "preview" && !store.m.preview) ||
                (t.key === "result" && !store.m.result)}
            <button
                class={"tabs__item" + (isActive ? " tabs__item--active" : "")}
                {disabled}
                style={disabled ? "opacity:0.4;cursor:not-allowed" : ""}
                onclick={() =>
                    !disabled &&
                    store.dispatch({ t: "NavigateTo", screen: t.key })}
            >
                <span class="tabs__num">{t.num}:</span>
                <span>{t.label}</span>
            </button>
        {/each}
        <button
            type="button"
            class="theme-toggle"
            onclick={toggleTheme}
            aria-label="Toggle theme"
            aria-pressed={theme === "dark"}
            title={theme === "dark"
                ? "Switch to light mode"
                : "Switch to dark mode"}
        >
            <span class="theme-toggle__glyph" aria-hidden="true"
                >{theme === "dark" ? "◑" : "◐"}</span
            >
            <span>{theme === "dark" ? "Dark" : "Light"}</span>
        </button>
    </nav>

    <!-- Main scroll area -->
    <main class="main">
        {#if notInTauri}
            <div class="banner">
                <strong>Not a Tauri window.</strong>
                This page is the Vite dev server; IPC calls won't work. Use the native
                window opened by <code>cargo tauri dev</code>.
            </div>
        {/if}

        {#if store.m.error}
            <div class="page" style="padding-bottom:0">
                <div class="error">
                    <span>{store.m.error}</span>
                    <span class="spacer"></span>
                    <button
                        class="btn btn--sm"
                        onclick={() => store.dispatch({ t: "ClearError" })}
                        >Dismiss</button
                    >
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
</div>
