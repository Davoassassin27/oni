export type AgentPreset = "opencode" | "claude" | "custom";

export type TaskState =
  | "queued"
  | "acting"
  | "verifying"
  | "retrying"
  | "review"
  | "merged"
  | "discarded"
  | "failed"
  | "stopped";

export interface Project {
  id: string;
  name: string;
  path: string;
  baseBranch: string;
  verifyCommand: string;
  agent: AgentPreset;
  customCommand: string | null;
  createdAt: string;
}

export interface FileChange {
  path: string;
  insertions: number;
  deletions: number;
}

export interface DiffSummary {
  stat: string;
  files: FileChange[];
  insertions: number;
  deletions: number;
}

export interface DiffFilePatch {
  path: string;
  patch: string;
  insertions: number;
  deletions: number;
}

export interface DiffPatch {
  files: DiffFilePatch[];
  truncated: boolean;
}

export interface VerificationGuard {
  tampered: boolean;
  files: string[];
  acceptedByHuman: boolean;
}

export interface Task {
  id: string;
  projectId: string;
  projectName: string;
  repoPath: string;
  baseBranch: string;
  title: string;
  prompt: string;
  acceptance: string;
  state: TaskState;
  branch: string;
  worktreePath: string;
  iteration: number;
  maxIterations: number;
  agent: AgentPreset;
  customCommand: string | null;
  verifyCommand: string;
  createdAt: string;
  updatedAt: string;
  startedAt: string | null;
  finishedAt: string | null;
  lastError: string | null;
  diff: DiffSummary | null;
  verification?: VerificationGuard;
}

export interface Health {
  ok: boolean;
  version: string;
  uptimeSecs: number;
  projects: number;
  tasks: number;
  dataDir: string;
  authRequired: boolean;
}

export interface NewProjectInput {
  name: string;
  path: string;
  baseBranch: string;
  verifyCommand: string;
  agent: AgentPreset;
  customCommand?: string | null;
}

export interface NewTaskInput {
  projectId: string;
  title: string;
  prompt: string;
  acceptance?: string;
  agent?: AgentPreset;
  customCommand?: string | null;
  maxIterations?: number;
}

export type RouterIntent =
  | "spawn_agent"
  | "query_status"
  | "merge_worktree"
  | "retry_task"
  | "cancel_task"
  | "discard_task"
  | "list_projects"
  | "help"
  | "unknown";

export interface RouterDecision {
  intent: RouterIntent;
  agent_target: "opencode" | "claude" | "custom" | "auto";
  model_override: string | null;
  worktree_strategy: "isolated_branch" | "existing_branch";
  branch_name: string | null;
  project_ref: string | null;
  task_ref: string | null;
  task_prompt: string | null;
  loop_status: "single_shot" | "continue_loop" | "completed" | "needs_human";
  max_iterations: number;
  success_criterion: string | null;
  reasoning: string;
}

export interface RouterRouteInput {
  text: string;
  projectId?: string | null;
  taskId?: string | null;
}

export interface RouterRouteResponse {
  decision: RouterDecision;
  routing: {
    source: "juniperus" | "fallback";
    latencyMs: number;
    degraded: boolean;
    error: string | null;
    backend?: string | null;
    model?: string | null;
  };
  context: { projects: number; tasks: number };
}

export interface RouterStatus {
  enabled: boolean;
  url: string | null;
  reachable: boolean;
  detail?: string;
  sidecar?: Record<string, unknown>;
}

export type ServerEvent =
  | { type: "task"; task: Task }
  | { type: "log"; taskId: string; chunk: string }
  | { type: "project"; project: Project }
  | { type: "project_deleted"; projectId: string };

export const ACTIVE_STATES: TaskState[] = [
  "queued",
  "acting",
  "verifying",
  "retrying",
];

export const DONE_STATES: TaskState[] = [
  "review",
  "merged",
  "discarded",
  "failed",
  "stopped",
];
