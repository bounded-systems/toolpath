// Shared domain + message types. The IPC shapes match the Rust serde
// definitions in `crates/toolpath-desktop/src/commands/*.rs`.

export type Route =
  | "home"
  | "browse-agents"
  | "browse-claude"
  | "browse-pi"
  | "browse-git"
  | "browse-github"
  | "preview"
  | "result";

export type AgentStatus = "available" | "unavailable" | "coming-soon";

export interface AgentSummary {
  id: string;
  name: string;
  tagline: string;
  status: AgentStatus;
  reason: string | null;
  data_path: string | null;
}

export interface ClaudeProject {
  project_path: string;
  display_name: string;
  session_count: number;
}

export interface ClaudeSession {
  project_path: string;
  session_id: string;
  turn_count: number;
  started_at: string | null;
  last_activity: string | null;
}

export interface PiProject {
  project_path: string;
  display_name: string;
  session_count: number;
}

export interface PiSession {
  project_path: string;
  session_id: string;
  entry_count: number;
  timestamp: string;
}

export interface GitBranch {
  name: string;
  head_short: string;
  subject: string;
  author: string;
  timestamp: string;
}

export interface UploadResult {
  url: string;
  status: string;
  stub: boolean;
}

// The Document is passed through as opaque JSON from the backend; we only
// read well-known fields when rendering. Keep the shape minimal.
export interface StepRef {
  step: {
    id: string;
    actor: string;
    timestamp?: string;
    parents?: string[];
  };
  change?: Record<string, { raw?: string; structural?: unknown }>;
  meta?: { intent?: string };
}

export interface DocPath {
  path: { id: string; head: string; base?: { uri: string; ref?: string } };
  steps: StepRef[];
  meta?: { title?: string; actors?: Record<string, ActorDef> };
}

export interface ActorDef {
  name?: string;
  provider?: string;
  model?: string;
  identities?: { system: string; id: string }[];
}

export type Document =
  | { Step: StepRef & { meta?: { actors?: Record<string, ActorDef> } } }
  | { Path: DocPath }
  | { Graph: { graph: { id: string }; paths: Array<DocPath | { $ref: string }>; meta?: { actors?: Record<string, ActorDef> } } };

// ── Model ───────────────────────────────────────────────────────────────

export interface Model {
  route: Route;
  error: string | null;
  agents: AgentsSlice;
  claude: ClaudeSlice;
  pi: PiSlice;
  git: GitSlice;
  github: GithubSlice;
  preview: PreviewSlice | null;
  result: ResultSlice | null;
}

export interface AgentsSlice {
  loading: boolean;
  list: AgentSummary[] | null; // null = never fetched
}

export interface ClaudeSlice {
  loadingProjects: boolean;
  projects: ClaudeProject[];
  projectsDone: boolean;
  expanded: string | null;
  sessionsByPath: Record<string, ClaudeSession[]>;
  sessionsLoading: Record<string, boolean>;
  titles: Record<string, string>;        // `${path}|${sid}` → title
  selected: Record<string, Record<string, true>>; // path → sid set
  deriving: boolean;
}

export interface PiSlice {
  loadingProjects: boolean;
  projects: PiProject[];
  projectsDone: boolean;
  expanded: string | null;
  sessionsByPath: Record<string, PiSession[]>;
  sessionsLoading: Record<string, boolean>;
  selected: Record<string, Record<string, true>>;
  deriving: boolean;
}

export interface GitSlice {
  repoPath: string;
  branches: GitBranch[] | null;
  loading: boolean;
  selected: string | null;
  deriving: boolean;
}

export interface GithubSlice {
  url: string;
  hasToken: boolean | null; // null = probing
  editingToken: boolean;
  tokenInput: string;
  includeCi: boolean;
  includeComments: boolean;
  savingToken: boolean;
  deriving: boolean;
}

export interface PreviewSlice {
  doc: Document;
  source: string;
  filename: string;
  selectedStep: StepRef | null;
  selectedActors: Record<string, ActorDef> | null;
  /** HEAD-ancestor node ids whose dead subtrees are expanded. */
  expandedBranches: Record<string, true>;
  showTs: boolean;
  showFiles: boolean;
  vizEpoch: number;
  exporting: boolean;
  uploading: boolean;
}

export type ResultSlice =
  | { kind: "export"; path: string; source: string }
  | { kind: "upload"; url: string; stub: boolean; source: string };

// ── Msg ────────────────────────────────────────────────────────────────
// Tagged union; `t` is the discriminator.

export type Msg =
  | { t: "NavigateTo"; screen: Route }
  | { t: "Error"; error: unknown }
  | { t: "ClearError" }

  // Agents
  | { t: "AgentsLoaded"; list: AgentSummary[] }
  | { t: "AgentsSelect"; agent: AgentSummary }

  // Claude
  | { t: "ClaudeEnsureProjects" }
  | { t: "ClaudeProjectReceived"; project: ClaudeProject }
  | { t: "ClaudeProjectsDone" }
  | { t: "ClaudeProjectsError"; error: string }
  | { t: "ClaudeExpandProject"; path: string }
  | { t: "ClaudeSessionReceived"; session: ClaudeSession }
  | { t: "ClaudeSessionsDone"; path: string }
  | { t: "ClaudeToggleSession"; path: string; sid: string }
  | { t: "ClaudeTitleLoaded"; path: string; sid: string; title: string | null }
  | { t: "ClaudeDerive" }

  // Pi
  | { t: "PiEnsureProjects" }
  | { t: "PiProjectReceived"; project: PiProject }
  | { t: "PiProjectsDone" }
  | { t: "PiProjectsError"; error: string }
  | { t: "PiExpandProject"; path: string }
  | { t: "PiSessionReceived"; session: PiSession }
  | { t: "PiSessionsDone"; path: string }
  | { t: "PiToggleSession"; path: string; sid: string }
  | { t: "PiDerive" }

  // Git
  | { t: "GitSetRepoPath"; value: string }
  | { t: "GitPickRepo" }
  | { t: "GitLoadBranches" }
  | { t: "GitBranchesLoaded"; list: GitBranch[] }
  | { t: "GitSelectBranch"; name: string }
  | { t: "GitDerive" }

  // GitHub
  | { t: "GithubEnsureTokenStatus" }
  | { t: "GithubSetUrl"; value: string }
  | { t: "GithubTokenStatus"; hasToken: boolean }
  | { t: "GithubEditToken"; on: boolean }
  | { t: "GithubSetTokenInput"; value: string }
  | { t: "GithubSaveToken" }
  | { t: "GithubTokenSaved" }
  | { t: "GithubClearToken" }
  | { t: "GithubTokenCleared" }
  | { t: "GithubToggleIncludeCi" }
  | { t: "GithubToggleComments" }
  | { t: "GithubDerive" }

  // Derive result
  | { t: "DeriveSucceeded"; doc: Document; source: string; filename: string }
  | { t: "DeriveFailed"; error: unknown }

  // Preview
  | { t: "PreviewToggle"; key: "showTs" | "showFiles" }
  | { t: "PreviewToggleBranch"; nodeId: string }
  | { t: "PreviewSelectStep"; step: StepRef; actors: Record<string, ActorDef> | null }
  | { t: "PreviewExport" }
  | { t: "PreviewExportDone" }
  | { t: "PreviewUpload" }
  | { t: "ExportSucceeded"; path: string }
  | { t: "UploadSucceeded"; res: UploadResult };

// ── Cmd ────────────────────────────────────────────────────────────────

export type Dispatch = (msg: Msg) => void;

export type Cmd =
  | { type: "invoke"; name: string; args?: Record<string, unknown>; onOk?: (r: unknown) => Msg | null; onErr?: (e: unknown) => Msg | null }
  | { type: "emitMsg"; msg: Msg }
  | { type: "batch"; cmds: Cmd[] }
  | { type: "fn"; run: (dispatch: Dispatch) => void | Promise<void> };
