import type {
  DiffPatch,
  Health,
  NewProjectInput,
  NewTaskInput,
  Project,
  RouterRouteInput,
  RouterRouteResponse,
  RouterStatus,
  ServerEvent,
  Task,
} from "./types";

const TOKEN_KEY = "oni.token";

export function getToken(): string {
  try {
    return localStorage.getItem(TOKEN_KEY) ?? "";
  } catch {
    return "";
  }
}

export function setToken(token: string): void {
  try {
    if (token.trim()) {
      localStorage.setItem(TOKEN_KEY, token.trim());
    } else {
      localStorage.removeItem(TOKEN_KEY);
    }
  } catch {
    // localStorage can be unavailable in private browsing.
  }
}

function headers(hasBody: boolean): Record<string, string> {
  const result: Record<string, string> = {};
  if (hasBody) result["Content-Type"] = "application/json";
  const token = getToken();
  if (token) result["Authorization"] = `Bearer ${token}`;
  return result;
}

async function toError(response: Response): Promise<Error> {
  let message = `${response.status} ${response.statusText}`;
  try {
    const payload = (await response.json()) as { error?: string };
    if (payload.error) message = payload.error;
  } catch {
    // Keep the HTTP status when the body is not JSON.
  }
  return new Error(message);
}

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const response = await fetch(`/api${path}`, {
    ...init,
    headers: { ...headers(Boolean(init.body)), ...init.headers },
  });
  if (!response.ok) throw await toError(response);
  if (response.status === 204) return undefined as T;
  return (await response.json()) as T;
}

async function requestText(path: string): Promise<string> {
  const response = await fetch(`/api${path}`, { headers: headers(false) });
  if (!response.ok) throw await toError(response);
  return response.text();
}

export const api = {
  health: () => request<Health>("/health"),
  projects: () => request<Project[]>("/projects"),
  createProject: (input: NewProjectInput) =>
    request<Project>("/projects", {
      method: "POST",
      body: JSON.stringify(input),
    }),
  updateProject: (id: string, input: NewProjectInput) =>
    request<Project>(`/projects/${id}`, {
      method: "PUT",
      body: JSON.stringify(input),
    }),
  deleteProject: (id: string) =>
    request<void>(`/projects/${id}`, { method: "DELETE" }),
  tasks: () => request<Task[]>("/tasks"),
  task: (id: string) => request<Task>(`/tasks/${id}`),
  createTask: (input: NewTaskInput) =>
    request<Task>("/tasks", { method: "POST", body: JSON.stringify(input) }),
  log: (id: string) => requestText(`/tasks/${id}/log`),
  patch: (id: string) => request<DiffPatch>(`/tasks/${id}/patch`),
  stop: (id: string) => request<Task>(`/tasks/${id}/stop`, { method: "POST" }),
  retry: (id: string) =>
    request<Task>(`/tasks/${id}/retry`, { method: "POST" }),
  merge: (id: string) =>
    request<Task>(`/tasks/${id}/merge`, { method: "POST" }),
  acceptVerification: (id: string) =>
    request<Task>(`/tasks/${id}/accept-verification`, { method: "POST" }),
  discard: (id: string) =>
    request<Task>(`/tasks/${id}/discard`, { method: "POST" }),
  routeIntent: (input: RouterRouteInput) =>
    request<RouterRouteResponse>("/router/route", {
      method: "POST",
      body: JSON.stringify(input),
    }),
  routerStatus: () => request<RouterStatus>("/router/status"),
  eventsUrl: () => {
    const token = getToken();
    return `/api/events${token ? `?token=${encodeURIComponent(token)}` : ""}`;
  },
};

export type LogEvent = Extract<ServerEvent, { type: "log" }>;
