<script lang="ts">
    import { invoke, listen } from "../lib/ipc";
    import type { UnlistenFn } from "@tauri-apps/api/event";

    type Provider = "claude" | "gemini" | "codex" | "opencode" | "pi";

    type ProviderCounts = {
        provider: Provider;
        active: number;
        recent: number;
    };

    type RecentSession = {
        provider: Provider;
        project: string;
        session_id: string;
        last_activity: string;
    };

    type TrayStats = {
        counts: ProviderCounts[];
        recent: RecentSession[];
        total_active: number;
        total_recent: number;
        polled_at: string;
    };

    let stats = $state<TrayStats | null>(null);

    // Fetch an immediate snapshot so the UI isn't blank on first open.
    invoke<TrayStats>("tray_stats_now")
        .then((s) => (stats = s))
        .catch(() => {});

    $effect(() => {
        let unlisten: UnlistenFn | undefined;
        listen<TrayStats>("tray:stats", (payload) => {
            stats = payload;
        }).then((fn) => (unlisten = fn));
        return () => unlisten?.();
    });

    // "Claude Code · myproj"
    function sessionLabel(r: RecentSession): string {
        const name = r.project ? basename(r.project) : r.session_id.slice(0, 8);
        return `${providerName(r.provider)} · ${name}`;
    }

    function providerName(p: Provider): string {
        switch (p) {
            case "claude":
                return "Claude";
            case "gemini":
                return "Gemini";
            case "codex":
                return "Codex";
            case "opencode":
                return "opencode";
            case "pi":
                return "pi.dev";
        }
    }

    function basename(path: string): string {
        const parts = path.split(/[/\\]/).filter(Boolean);
        return parts[parts.length - 1] ?? path;
    }

    function ago(iso: string): string {
        const then = new Date(iso).getTime();
        if (!isFinite(then)) return "";
        const sec = Math.max(0, Math.floor((Date.now() - then) / 1000));
        if (sec < 60) return `${sec}s ago`;
        const min = Math.floor(sec / 60);
        if (min < 60) return `${min}m ago`;
        const hr = Math.floor(min / 60);
        if (hr < 24) return `${hr}h ago`;
        const day = Math.floor(hr / 24);
        return `${day}d ago`;
    }

    async function openMain() {
        await invoke("tray_open_main");
    }

    async function dismiss() {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        await getCurrentWindow().hide();
    }

    // Providers whose derive command is wired up on the Rust side.
    // Others show in the list (for activity-tracking) but don't open.
    const SUPPORTED: ReadonlySet<Provider> = new Set<Provider>(["claude", "pi"]);

    let openError = $state<string | null>(null);

    async function openTrace(r: RecentSession) {
        if (!SUPPORTED.has(r.provider)) return;
        openError = null;
        try {
            await invoke("tray_open_trace", {
                provider: r.provider,
                project: r.project,
                sessionId: r.session_id,
            });
        } catch (e) {
            openError = String(e);
        }
    }
</script>

<div class="quick">
    <header class="quick__header">
        <span class="quick__title">Quick View</span>
        {#if stats}
            <span class="quick__meta"
                >{stats.total_active} active · {stats.total_recent} recent</span
            >
        {:else}
            <span class="quick__meta">loading…</span>
        {/if}
    </header>

    {#if stats}
        <ul class="quick__counts">
            {#each stats.counts as c (c.provider)}
                <li class="quick__count">
                    <span class="quick__count-name">{providerName(c.provider)}</span>
                    <span class="quick__count-active" class:on={c.active > 0}
                        >● {c.active}</span
                    >
                    <span class="quick__count-recent">{c.recent} today</span>
                </li>
            {/each}
        </ul>

        <div class="quick__section-label">Recent</div>
        <ul class="quick__recent">
            {#if stats.recent.length === 0}
                <li class="quick__empty">No recent activity.</li>
            {/if}
            {#each stats.recent as r (r.provider + ":" + r.session_id)}
                {@const supported = SUPPORTED.has(r.provider)}
                <li class="quick__session">
                    <button
                        class="quick__session-btn"
                        class:quick__session-btn--disabled={!supported}
                        disabled={!supported}
                        title={supported
                            ? "Open in Toolpath"
                            : `Opening ${providerName(r.provider)} traces from Quick View is coming soon.`}
                        onclick={() => openTrace(r)}
                    >
                        <span class="quick__session-label">{sessionLabel(r)}</span>
                        <span class="quick__session-when">{ago(r.last_activity)}</span>
                    </button>
                </li>
            {/each}
        </ul>

        {#if openError}
            <div class="quick__open-error" role="alert">{openError}</div>
        {/if}
    {/if}

    <footer class="quick__footer">
        <button class="quick__btn quick__btn--primary" onclick={openMain}
            >Open Toolpath</button
        >
        <button class="quick__btn" onclick={dismiss}>Close</button>
        <span class="quick__spacer"></span>
        {#if stats}
            <span class="quick__polled" title={stats.polled_at}
                >updated {ago(stats.polled_at)}</span
            >
        {/if}
    </footer>
</div>

<style>
    :global(html, body) {
        margin: 0;
        padding: 0;
        background: transparent;
        font-family:
            -apple-system, BlinkMacSystemFont, "SF Pro Text", system-ui,
            sans-serif;
        font-size: 12px;
        color: #1d1d1f;
    }
    :global(html[data-theme="dark"]),
    :global(html[data-theme="dark"] body) {
        color: #f2f2f2;
    }

    .quick {
        display: flex;
        flex-direction: column;
        height: 100vh;
        padding: 10px 12px;
        box-sizing: border-box;
        background: rgba(250, 250, 250, 0.98);
        border-radius: 10px;
    }
    :global(html[data-theme="dark"]) .quick {
        background: rgba(30, 30, 32, 0.98);
    }

    .quick__header {
        display: flex;
        align-items: baseline;
        justify-content: space-between;
        margin-bottom: 8px;
    }
    .quick__title {
        font-weight: 600;
        font-size: 13px;
    }
    .quick__meta {
        font-size: 11px;
        opacity: 0.7;
    }

    .quick__counts {
        list-style: none;
        padding: 0;
        margin: 0 0 10px 0;
        border-top: 1px solid rgba(0, 0, 0, 0.08);
        border-bottom: 1px solid rgba(0, 0, 0, 0.08);
    }
    :global(html[data-theme="dark"]) .quick__counts {
        border-top-color: rgba(255, 255, 255, 0.1);
        border-bottom-color: rgba(255, 255, 255, 0.1);
    }
    .quick__count {
        display: grid;
        grid-template-columns: 1fr auto auto;
        gap: 10px;
        padding: 4px 2px;
        font-variant-numeric: tabular-nums;
    }
    .quick__count-active {
        opacity: 0.35;
    }
    .quick__count-active.on {
        opacity: 1;
        color: #1f9e4e;
    }
    .quick__count-recent {
        opacity: 0.7;
    }

    .quick__section-label {
        font-size: 10px;
        text-transform: uppercase;
        letter-spacing: 0.08em;
        opacity: 0.55;
        margin-bottom: 4px;
    }
    .quick__recent {
        list-style: none;
        padding: 0;
        margin: 0;
        flex: 1;
        overflow-y: auto;
    }
    .quick__empty {
        opacity: 0.5;
        padding: 12px 0;
        text-align: center;
    }
    .quick__session {
        list-style: none;
    }
    .quick__session-btn {
        display: flex;
        width: 100%;
        align-items: center;
        justify-content: space-between;
        padding: 4px 6px;
        margin: 1px 0;
        border: 0;
        background: transparent;
        border-radius: 4px;
        color: inherit;
        font: inherit;
        text-align: left;
        cursor: pointer;
    }
    .quick__session-btn:hover:not(:disabled) {
        background: rgba(0, 0, 0, 0.06);
    }
    :global(html[data-theme="dark"]) .quick__session-btn:hover:not(:disabled) {
        background: rgba(255, 255, 255, 0.08);
    }
    .quick__session-btn--disabled {
        cursor: not-allowed;
        opacity: 0.55;
    }
    .quick__session-label {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        padding-right: 8px;
    }
    .quick__session-when {
        font-size: 10px;
        opacity: 0.6;
        flex-shrink: 0;
    }
    .quick__open-error {
        margin-top: 6px;
        padding: 5px 7px;
        font-size: 11px;
        color: #b00020;
        background: rgba(176, 0, 32, 0.08);
        border-radius: 4px;
    }
    :global(html[data-theme="dark"]) .quick__open-error {
        color: #ff8b9b;
        background: rgba(255, 139, 155, 0.12);
    }

    .quick__footer {
        display: flex;
        align-items: center;
        margin-top: 8px;
        padding-top: 6px;
        border-top: 1px solid rgba(0, 0, 0, 0.08);
    }
    :global(html[data-theme="dark"]) .quick__footer {
        border-top-color: rgba(255, 255, 255, 0.1);
    }
    .quick__btn {
        font: inherit;
        padding: 3px 10px;
        margin-right: 6px;
        border: 1px solid rgba(0, 0, 0, 0.15);
        background: transparent;
        border-radius: 6px;
        cursor: pointer;
        color: inherit;
    }
    .quick__btn:hover {
        background: rgba(0, 0, 0, 0.05);
    }
    .quick__btn--primary {
        background: #1d1d1f;
        color: #fff;
        border-color: #1d1d1f;
    }
    .quick__btn--primary:hover {
        background: #000;
    }
    :global(html[data-theme="dark"]) .quick__btn {
        border-color: rgba(255, 255, 255, 0.2);
    }
    :global(html[data-theme="dark"]) .quick__btn:hover {
        background: rgba(255, 255, 255, 0.08);
    }
    :global(html[data-theme="dark"]) .quick__btn--primary {
        background: #f2f2f2;
        color: #1d1d1f;
        border-color: #f2f2f2;
    }
    :global(html[data-theme="dark"]) .quick__btn--primary:hover {
        background: #fff;
    }
    .quick__spacer {
        flex: 1;
    }
    .quick__polled {
        font-size: 10px;
        opacity: 0.55;
    }
</style>
