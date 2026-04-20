<script lang="ts">
  import { store } from "../lib/store.svelte";
  import type { Route } from "../lib/types";

  // Four source types, mirroring the cartographic source-picker tabs in the
  // design. Each tab kicks off its own browse flow (live backend data).
  type SourceKind = "claude" | "pi" | "git" | "github";
  const SOURCES: { k: SourceKind; glyph: string; label: string; route: Route; desc: string }[] = [
    { k: "claude", glyph: "⊕", label: "Claude Code", route: "browse-claude", desc: "Your local Claude Code sessions in ~/.claude/projects/." },
    { k: "pi",     glyph: "●", label: "Pi.dev",      route: "browse-pi",     desc: "Pi agent conversations tracked on this machine." },
    { k: "git",    glyph: "⎇", label: "Git",         route: "browse-git",    desc: "A local repository; branch history becomes the trace." },
    { k: "github", glyph: "◇", label: "GitHub PR",   route: "browse-github", desc: "A pull request URL. Fetches commits, reviews, and CI." },
  ];

  let active: SourceKind = $state("claude");
</script>

<div class="page">
  <!-- Sheet title -->
  <div class="page__header">
    <div>
      <div class="page__eyebrow">△ TOOLPATH DESKTOP · SHEET 17 · FA·FV</div>
      <h1 class="page__title">Upload provenance to <span class="accent">pathbase.dev</span></h1>
      <p class="page__lede">
        Turn git history, GitHub pull requests, Claude Code sessions, and Pi.dev
        conversations into Toolpath documents — then upload them. Every CLI
        verb has a GUI surface.
      </p>
    </div>
    <div class="page__coord">
      52°14'N · 22°41'E<br/>scale 1:25 000
    </div>
  </div>

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
        class={"source-tabs__item" + (active === s.k ? " source-tabs__item--active" : "")}
        onclick={() => { active = s.k; }}
      >
        <span class="source-tabs__glyph">{s.glyph}</span>
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
              <span class="section-label__num">§1.{SOURCES.indexOf(s) + 1} ·</span>
              <span class="section-label__text">{s.label}</span>
              <span class="section-label__right">sourced live</span>
            </div>
            <p style="font-family:var(--font-serif); font-size:14px; color:var(--ink-2); margin:0 0 8px">
              {s.desc}
            </p>
            <p style="font-family:var(--font-mono); font-size:11px; color:var(--ink-4); letter-spacing:0.06em; margin:0">
              ⎇ derived via <code>path derive {s.k}</code>
            </p>
          </div>
          <button
            class="btn btn--accent"
            onclick={() => store.dispatch({ t: "NavigateTo", screen: s.route })}
          >Browse {s.label} →</button>
        </div>
      </div>
    {/if}
  {/each}

  <!-- Below-fold annotations — three cartographic notes -->
  <div style="margin-top:28px; display:grid; grid-template-columns:repeat(3,1fr); gap:16px">
    <div class="card-panel">
      <div class="page__eyebrow">NOTE · A</div>
      <div style="font-family:var(--font-display); font-size:17px; font-weight:600; color:var(--ink); margin-top:4px">Why a desktop app?</div>
      <div style="font-family:var(--font-serif); font-size:13.5px; color:var(--ink-2); margin-top:6px; line-height:1.55">
        The CLI is the ground truth. This surface exposes every verb — list,
        derive, preview, upload — with one-click flows and a cartographic
        preview of each derived Path.
      </div>
    </div>
    <div class="card-panel">
      <div class="page__eyebrow">NOTE · B</div>
      <div style="font-family:var(--font-display); font-size:17px; font-weight:600; color:var(--ink); margin-top:4px">Four sources, one format</div>
      <div style="font-family:var(--font-serif); font-size:13.5px; color:var(--ink-2); margin-top:6px; line-height:1.55">
        Claude sessions, Pi conversations, git branches, and GitHub PRs all
        derive into the same <code>toolpath/path</code> document. Upload from
        any of them with the same payload.
      </div>
    </div>
    <div class="card-panel">
      <div class="page__eyebrow">NOTE · C</div>
      <div style="font-family:var(--font-display); font-size:17px; font-weight:600; color:var(--ink); margin-top:4px">CLI parity</div>
      <div style="font-family:var(--font-serif); font-size:13.5px; color:var(--ink-2); margin-top:6px; line-height:1.55">
        Every panel surfaces the exact <code>path …</code> command it would
        invoke. Power users can copy it; hand-off to <code>path</code> is one
        keystroke away.
      </div>
    </div>
  </div>
</div>
