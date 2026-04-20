// Pure Msg -> Model reducer. No Svelte, no Tauri — callable from tests.
//
// Returns a [nextModel, cmd] pair. `cmd` is either a single `Cmd` or null.

import { invoke } from "./ipc";
import type { Cmd, Model, Msg } from "./types";

export function initialModel(): Model {
  return {
    route: "home",
    error: null,
    agents: { loading: false, list: null },
    claude: {
      loadingProjects: false,
      projects: [],
      projectsDone: false,
      expanded: null,
      sessionsByPath: {},
      sessionsLoading: {},
      titles: {},
      selected: {},
      deriving: false,
    },
    pi: {
      loadingProjects: false,
      projects: [],
      projectsDone: false,
      expanded: null,
      sessionsByPath: {},
      sessionsLoading: {},
      selected: {},
      deriving: false,
    },
    git: {
      repoPath: "",
      branches: null,
      loading: false,
      selected: null,
      deriving: false,
    },
    github: {
      url: "",
      hasToken: null,
      editingToken: false,
      tokenInput: "",
      includeCi: true,
      includeComments: true,
      savingToken: false,
      deriving: false,
    },
    preview: null,
    result: null,
  };
}

export function update(msg: Msg, m: Model): [Model, Cmd | null] {
  switch (msg.t) {
    case "NavigateTo": {
      if (msg.screen === "browse-agents" && m.agents.list === null && !m.agents.loading) {
        return [
          { ...m, route: msg.screen, error: null, agents: { ...m.agents, loading: true } },
          {
            type: "invoke",
            name: "list_agents",
            onOk: (list) => ({ t: "AgentsLoaded", list: list as AgentList }),
            onErr: (e) => ({ t: "Error", error: e }),
          },
        ];
      }
      // NOTE for streaming flows (claude/pi): we flip `loadingProjects: true`
      // here but DO NOT kick off the invoke. The component's mount-effect
      // subscribes to the events, awaits those Promises, and only then
      // issues the invoke — eliminating the race where backend events fire
      // before Tauri has registered the listener.
      if (msg.screen === "browse-claude" && !m.claude.loadingProjects && m.claude.projects.length === 0) {
        return [
          {
            ...m,
            route: msg.screen,
            error: null,
            claude: { ...m.claude, loadingProjects: true, projectsDone: false },
          },
          null,
        ];
      }
      if (msg.screen === "browse-pi" && !m.pi.loadingProjects && m.pi.projects.length === 0) {
        return [
          {
            ...m,
            route: msg.screen,
            error: null,
            pi: { ...m.pi, loadingProjects: true, projectsDone: false },
          },
          null,
        ];
      }
      if (msg.screen === "browse-github") {
        return [
          { ...m, route: msg.screen, error: null, github: { ...m.github, hasToken: null } },
          {
            type: "invoke",
            name: "github_has_token",
            onOk: (b) => ({ t: "GithubTokenStatus", hasToken: !!b }),
            onErr: (e) => ({ t: "Error", error: e }),
          },
        ];
      }
      return [{ ...m, route: msg.screen, error: null }, null];
    }

    case "Error":
      return [{ ...m, error: fmtErr(msg.error) }, null];
    case "ClearError":
      return [{ ...m, error: null }, null];

    case "AgentsLoaded":
      return [{ ...m, agents: { loading: false, list: msg.list } }, null];
    case "AgentsSelect": {
      if (!msg.agent || msg.agent.status !== "available") return [m, null];
      if (msg.agent.id === "claude-code") {
        return [m, { type: "emitMsg", msg: { t: "NavigateTo", screen: "browse-claude" } }];
      }
      if (msg.agent.id === "pi-dev") {
        return [m, { type: "emitMsg", msg: { t: "NavigateTo", screen: "browse-pi" } }];
      }
      return [m, null];
    }

    case "ClaudeProjectReceived":
      return [
        { ...m, claude: { ...m.claude, projects: [...m.claude.projects, msg.project] } },
        null,
      ];
    case "ClaudeProjectsDone":
      return [{ ...m, claude: { ...m.claude, loadingProjects: false, projectsDone: true } }, null];
    case "ClaudeProjectsError":
      return [
        { ...m, error: msg.error, claude: { ...m.claude, loadingProjects: false, projectsDone: true } },
        null,
      ];

    case "ClaudeExpandProject": {
      const wasExpanded = m.claude.expanded === msg.path;
      const claude = { ...m.claude, expanded: wasExpanded ? null : msg.path };
      if (wasExpanded || m.claude.sessionsByPath[msg.path]) {
        return [{ ...m, claude }, null];
      }
      claude.sessionsByPath = { ...claude.sessionsByPath, [msg.path]: [] };
      claude.sessionsLoading = { ...claude.sessionsLoading, [msg.path]: true };
      // Invoke is kicked off by the component's subscribe-effect once the
      // `claude:session` listener is live — see BrowseClaude.svelte.
      return [{ ...m, claude }, null];
    }
    case "ClaudeSessionReceived": {
      const path = msg.session.project_path;
      const existing = m.claude.sessionsByPath[path] ?? [];
      const claude = {
        ...m.claude,
        sessionsByPath: { ...m.claude.sessionsByPath, [path]: [...existing, msg.session] },
      };
      return [
        { ...m, claude },
        {
          type: "invoke",
          name: "claude_session_title",
          args: { projectPath: path, sessionId: msg.session.session_id },
          onOk: (title) => ({
            t: "ClaudeTitleLoaded",
            path,
            sid: msg.session.session_id,
            title: (title ?? null) as string | null,
          }),
        },
      ];
    }
    case "ClaudeSessionsDone":
      return [
        { ...m, claude: { ...m.claude, sessionsLoading: { ...m.claude.sessionsLoading, [msg.path]: false } } },
        null,
      ];
    case "ClaudeToggleSession": {
      const sel = { ...(m.claude.selected[msg.path] ?? {}) };
      if (sel[msg.sid]) delete sel[msg.sid];
      else sel[msg.sid] = true;
      return [
        { ...m, claude: { ...m.claude, selected: { ...m.claude.selected, [msg.path]: sel } } },
        null,
      ];
    }
    case "ClaudeTitleLoaded": {
      const titles = { ...m.claude.titles, [`${msg.path}|${msg.sid}`]: msg.title ?? "" };
      return [{ ...m, claude: { ...m.claude, titles } }, null];
    }
    case "ClaudeDerive": {
      const path = Object.keys(m.claude.selected).find(
        (p) => Object.keys(m.claude.selected[p] ?? {}).length > 0,
      );
      if (!path) return [m, null];
      const sessionIds = Object.keys(m.claude.selected[path] ?? {});
      const displayName = path.split("/").filter(Boolean).pop() ?? "claude";
      const shortId = sessionIds[0]?.slice(0, 8) ?? "";
      const filename = `${displayName}${shortId ? `-${shortId}` : ""}.path.json`;
      return [
        { ...m, claude: { ...m.claude, deriving: true }, error: null },
        {
          type: "invoke",
          name: "derive_claude",
          args: { projectPath: path, sessionIds, includeThinking: false },
          onOk: (doc) => ({ t: "DeriveSucceeded", doc: doc as import("./types").Document, source: `Claude: ${displayName}`, filename }),
          onErr: (e) => ({ t: "DeriveFailed", error: e }),
        },
      ];
    }

    // --- Pi -------------------------------------------------------------
    case "PiProjectReceived":
      return [
        { ...m, pi: { ...m.pi, projects: [...m.pi.projects, msg.project] } },
        null,
      ];
    case "PiProjectsDone":
      return [{ ...m, pi: { ...m.pi, loadingProjects: false, projectsDone: true } }, null];
    case "PiProjectsError":
      return [
        { ...m, error: msg.error, pi: { ...m.pi, loadingProjects: false, projectsDone: true } },
        null,
      ];
    case "PiExpandProject": {
      const wasExpanded = m.pi.expanded === msg.path;
      const pi = { ...m.pi, expanded: wasExpanded ? null : msg.path };
      if (wasExpanded || m.pi.sessionsByPath[msg.path]) {
        return [{ ...m, pi }, null];
      }
      pi.sessionsByPath = { ...pi.sessionsByPath, [msg.path]: [] };
      pi.sessionsLoading = { ...pi.sessionsLoading, [msg.path]: true };
      // Invoke kicked off from BrowsePi.svelte's subscribe-effect.
      return [{ ...m, pi }, null];
    }
    case "PiSessionReceived": {
      const path = msg.session.project_path;
      const existing = m.pi.sessionsByPath[path] ?? [];
      return [
        {
          ...m,
          pi: {
            ...m.pi,
            sessionsByPath: { ...m.pi.sessionsByPath, [path]: [...existing, msg.session] },
          },
        },
        null,
      ];
    }
    case "PiSessionsDone":
      return [
        { ...m, pi: { ...m.pi, sessionsLoading: { ...m.pi.sessionsLoading, [msg.path]: false } } },
        null,
      ];
    case "PiToggleSession": {
      const sel = { ...(m.pi.selected[msg.path] ?? {}) };
      if (sel[msg.sid]) delete sel[msg.sid];
      else sel[msg.sid] = true;
      return [
        { ...m, pi: { ...m.pi, selected: { ...m.pi.selected, [msg.path]: sel } } },
        null,
      ];
    }
    case "PiDerive": {
      const path = Object.keys(m.pi.selected).find(
        (p) => Object.keys(m.pi.selected[p] ?? {}).length > 0,
      );
      if (!path) return [m, null];
      const sessionIds = Object.keys(m.pi.selected[path] ?? {});
      const displayName = path.split("/").filter(Boolean).pop() ?? "pi";
      const shortId = sessionIds[0]?.slice(0, 8) ?? "";
      const filename = `${displayName}${shortId ? `-${shortId}` : ""}.path.json`;
      return [
        { ...m, pi: { ...m.pi, deriving: true }, error: null },
        {
          type: "invoke",
          name: "derive_pi",
          args: { projectPath: path, sessionIds, includeThinking: false },
          onOk: (doc) => ({
            t: "DeriveSucceeded",
            doc: doc as import("./types").Document,
            source: `pi.dev: ${displayName}`,
            filename,
          }),
          onErr: (e) => ({ t: "DeriveFailed", error: e }),
        },
      ];
    }

    // --- Git ------------------------------------------------------------
    case "GitSetRepoPath":
      return [{ ...m, git: { ...m.git, repoPath: msg.value } }, null];
    case "GitPickRepo":
      return [
        m,
        {
          type: "fn",
          run: async (dispatch) => {
            const { open } = await import("@tauri-apps/plugin-dialog");
            try {
              const picked = await open({ directory: true, multiple: false });
              if (typeof picked === "string" && picked) {
                dispatch({ t: "GitSetRepoPath", value: picked });
                dispatch({ t: "GitLoadBranches" });
              }
            } catch (e) {
              dispatch({ t: "Error", error: e });
            }
          },
        },
      ];
    case "GitLoadBranches": {
      if (!m.git.repoPath) return [m, null];
      return [
        { ...m, git: { ...m.git, loading: true, branches: null, selected: null }, error: null },
        {
          type: "invoke",
          name: "list_git_branches",
          args: { repoPath: m.git.repoPath },
          onOk: (list) => ({ t: "GitBranchesLoaded", list: list as import("./types").GitBranch[] }),
          onErr: (e) => ({ t: "Error", error: e }),
        },
      ];
    }
    case "GitBranchesLoaded": {
      const selected = msg.list.length ? msg.list[0].name : null;
      return [{ ...m, git: { ...m.git, loading: false, branches: msg.list, selected } }, null];
    }
    case "GitSelectBranch":
      return [{ ...m, git: { ...m.git, selected: msg.name } }, null];
    case "GitDerive": {
      const g = m.git;
      if (!g.selected || !g.repoPath) return [m, null];
      const repoName = g.repoPath.split("/").filter(Boolean).pop() ?? "repo";
      const slug = g.selected.replace(/[^a-z0-9]+/gi, "-");
      return [
        { ...m, git: { ...g, deriving: true }, error: null },
        {
          type: "invoke",
          name: "derive_git",
          args: { repoPath: g.repoPath, branch: g.selected, base: null },
          onOk: (doc) => ({
            t: "DeriveSucceeded",
            doc: doc as import("./types").Document,
            source: `Git: ${repoName}@${g.selected}`,
            filename: `${repoName}-${slug}.path.json`,
          }),
          onErr: (e) => ({ t: "DeriveFailed", error: e }),
        },
      ];
    }

    // --- GitHub ---------------------------------------------------------
    case "GithubSetUrl":
      return [{ ...m, github: { ...m.github, url: msg.value } }, null];
    case "GithubTokenStatus":
      return [{ ...m, github: { ...m.github, hasToken: msg.hasToken } }, null];
    case "GithubEditToken":
      return [
        { ...m, github: { ...m.github, editingToken: msg.on, tokenInput: msg.on ? "" : m.github.tokenInput } },
        null,
      ];
    case "GithubSetTokenInput":
      return [{ ...m, github: { ...m.github, tokenInput: msg.value } }, null];
    case "GithubSaveToken": {
      const t = m.github.tokenInput.trim();
      if (!t) return [m, null];
      return [
        { ...m, github: { ...m.github, savingToken: true }, error: null },
        {
          type: "invoke",
          name: "github_set_token",
          args: { token: t },
          onOk: () => ({ t: "GithubTokenSaved" }),
          onErr: (e) => ({ t: "Error", error: e }),
        },
      ];
    }
    case "GithubTokenSaved":
      return [
        { ...m, github: { ...m.github, savingToken: false, editingToken: false, tokenInput: "", hasToken: true } },
        null,
      ];
    case "GithubClearToken":
      return [
        m,
        {
          type: "invoke",
          name: "github_clear_token",
          onOk: () => ({ t: "GithubTokenCleared" }),
          onErr: (e) => ({ t: "Error", error: e }),
        },
      ];
    case "GithubTokenCleared":
      return [{ ...m, github: { ...m.github, hasToken: false } }, null];
    case "GithubToggleIncludeCi":
      return [{ ...m, github: { ...m.github, includeCi: !m.github.includeCi } }, null];
    case "GithubToggleComments":
      return [{ ...m, github: { ...m.github, includeComments: !m.github.includeComments } }, null];
    case "GithubDerive": {
      const gh = m.github;
      if (!gh.url || !gh.hasToken) return [m, null];
      const prMatch = gh.url.match(/github\.com\/([^/]+)\/([^/]+)\/pull\/(\d+)/);
      const sourceLabel = prMatch ? `${prMatch[1]}/${prMatch[2]}#${prMatch[3]}` : gh.url;
      const filename = prMatch
        ? `${prMatch[1]}-${prMatch[2]}-pr-${prMatch[3]}.path.json`
        : "github-pr.path.json";
      return [
        { ...m, github: { ...gh, deriving: true }, error: null },
        {
          type: "invoke",
          name: "derive_github",
          args: { prUrl: gh.url, includeCi: gh.includeCi, includeComments: gh.includeComments },
          onOk: (doc) => ({
            t: "DeriveSucceeded",
            doc: doc as import("./types").Document,
            source: `GitHub: ${sourceLabel}`,
            filename,
          }),
          onErr: (e) => ({ t: "DeriveFailed", error: e }),
        },
      ];
    }

    case "DeriveSucceeded":
      return [
        {
          ...m,
          route: "preview",
          claude: { ...m.claude, deriving: false },
          pi: { ...m.pi, deriving: false },
          git: { ...m.git, deriving: false },
          github: { ...m.github, deriving: false },
          preview: {
            doc: msg.doc,
            source: msg.source,
            filename: msg.filename,
            selectedStep: null,
            selectedActors: null,
            expandedBranches: {},
            showTs: false,
            showFiles: false,
            vizEpoch: 0,
            exporting: false,
            uploading: false,
          },
        },
        null,
      ];
    case "DeriveFailed":
      return [
        {
          ...m,
          claude: { ...m.claude, deriving: false },
          pi: { ...m.pi, deriving: false },
          git: { ...m.git, deriving: false },
          github: { ...m.github, deriving: false },
          error: fmtErr(msg.error),
        },
        null,
      ];

    // --- Preview --------------------------------------------------------
    case "PreviewToggle": {
      if (!m.preview) return [m, null];
      return [
        {
          ...m,
          preview: { ...m.preview, [msg.key]: !m.preview[msg.key], vizEpoch: m.preview.vizEpoch + 1 },
        },
        null,
      ];
    }
    case "PreviewToggleBranch": {
      if (!m.preview) return [m, null];
      const cur = m.preview.expandedBranches;
      const next = { ...cur };
      if (next[msg.nodeId]) delete next[msg.nodeId];
      else next[msg.nodeId] = true;
      return [
        {
          ...m,
          preview: { ...m.preview, expandedBranches: next, vizEpoch: m.preview.vizEpoch + 1 },
        },
        null,
      ];
    }
    case "PreviewSelectStep":
      if (!m.preview) return [m, null];
      return [
        { ...m, preview: { ...m.preview, selectedStep: msg.step, selectedActors: msg.actors } },
        null,
      ];
    case "PreviewExport": {
      if (!m.preview) return [m, null];
      const pv = m.preview;
      return [
        { ...m, preview: { ...pv, exporting: true }, error: null },
        {
          type: "fn",
          run: async (dispatch) => {
            try {
              const { save } = await import("@tauri-apps/plugin-dialog");
              const picked = await save({
                defaultPath: pv.filename,
                filters: [{ name: "Toolpath JSON", extensions: ["json"] }],
              });
              if (!picked) {
                dispatch({ t: "PreviewExportDone" });
                return;
              }
              const saved = await invoke<string>("save_document", {
                document: pv.doc,
                outPath: picked,
              });
              dispatch({ t: "ExportSucceeded", path: saved });
            } catch (e) {
              dispatch({ t: "Error", error: e });
              dispatch({ t: "PreviewExportDone" });
            }
          },
        },
      ];
    }
    case "PreviewExportDone":
      return [m.preview ? { ...m, preview: { ...m.preview, exporting: false } } : m, null];
    case "ExportSucceeded":
      return [
        {
          ...m,
          route: "result",
          preview: m.preview ? { ...m.preview, exporting: false } : null,
          result: { kind: "export", path: msg.path, source: m.preview?.source ?? "" },
        },
        null,
      ];

    case "PreviewUpload":
      if (!m.preview) return [m, null];
      return [
        { ...m, preview: { ...m.preview, uploading: true }, error: null },
        {
          type: "invoke",
          name: "upload_to_pathbase",
          args: { document: m.preview.doc },
          onOk: (res) => ({ t: "UploadSucceeded", res: res as import("./types").UploadResult }),
          onErr: (e) => ({ t: "Error", error: e }),
        },
      ];
    case "UploadSucceeded":
      return [
        {
          ...m,
          route: "result",
          preview: m.preview ? { ...m.preview, uploading: false } : null,
          result: { kind: "upload", url: msg.res.url, stub: msg.res.stub, source: m.preview?.source ?? "" },
        },
        null,
      ];
  }
}

function fmtErr(e: unknown): string {
  if (e == null) return "unknown error";
  if (typeof e === "string") return e;
  if (typeof e === "object" && e !== null) {
    const anyE = e as { message?: string; code?: string };
    if (anyE.message) return anyE.message;
    try { return JSON.stringify(e); } catch { return String(e); }
  }
  return String(e);
}

type AgentList = import("./types").AgentSummary[];
