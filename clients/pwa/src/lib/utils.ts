import type { AgentPreset, TaskState } from "./types";

export function cn(
  ...values: Array<string | false | null | undefined>
): string {
  return values.filter(Boolean).join(" ");
}

export function relativeTime(iso: string): string {
  const date = new Date(iso).getTime();
  if (Number.isNaN(date)) return "";
  const seconds = Math.round((Date.now() - date) / 1000);
  if (seconds < 45) return "just now";
  if (seconds < 90) return "1 min ago";
  if (seconds < 3600) return `${Math.round(seconds / 60)} min ago`;
  if (seconds < 7200) return "1 h ago";
  if (seconds < 86_400) return `${Math.round(seconds / 3600)} h ago`;
  if (seconds < 172_800) return "yesterday";
  return new Intl.DateTimeFormat("en", {
    month: "short",
    day: "numeric",
  }).format(new Date(iso));
}

export function formatDuration(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  return `${hours}h ${minutes}m`;
}

export function stateLabel(state: TaskState): string {
  switch (state) {
    case "queued":
      return "Queued";
    case "acting":
      return "Acting";
    case "verifying":
      return "Verifying";
    case "retrying":
      return "Retrying";
    case "review":
      return "Review";
    case "merged":
      return "Merged";
    case "discarded":
      return "Discarded";
    case "failed":
      return "Failed";
    case "stopped":
      return "Stopped";
  }
}

export function stateTone(
  state: TaskState,
): "active" | "review" | "done" | "danger" {
  switch (state) {
    case "queued":
    case "acting":
    case "verifying":
    case "retrying":
      return "active";
    case "review":
      return "review";
    case "merged":
    case "discarded":
      return "done";
    case "failed":
    case "stopped":
      return "danger";
  }
}

export function agentLabel(agent: AgentPreset): string {
  switch (agent) {
    case "opencode":
      return "OpenCode";
    case "claude":
      return "Claude Code";
    case "custom":
      return "Custom";
  }
}

export function truncateMiddle(value: string, max = 42): string {
  if (value.length <= max) return value;
  const half = Math.floor((max - 1) / 2);
  return `${value.slice(0, half)}…${value.slice(-half)}`;
}

export function pluralize(
  count: number,
  singular: string,
  plural = `${singular}s`,
): string {
  return count === 1 ? singular : plural;
}

export async function copyText(value: string): Promise<boolean> {
  try {
    if (navigator.clipboard && window.isSecureContext) {
      await navigator.clipboard.writeText(value);
      return true;
    }
  } catch {
    // Fall through to the legacy path (LAN over plain HTTP).
  }

  try {
    const area = document.createElement("textarea");
    area.value = value;
    area.style.position = "fixed";
    area.style.opacity = "0";
    document.body.appendChild(area);
    area.focus();
    area.select();
    const copied = document.execCommand("copy");
    document.body.removeChild(area);
    return copied;
  } catch {
    return false;
  }
}
