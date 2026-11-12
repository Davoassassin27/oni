import { useEffect, useState } from "react";
import { api, type LogEvent } from "./api";

const MAX_LOG_CHARS = 240_000;

function append(current: string, chunk: string): string {
  const next = current + chunk;
  if (next.length <= MAX_LOG_CHARS) return next;
  return `… earlier output truncated …\n${next.slice(next.length - MAX_LOG_CHARS)}`;
}

export function useTaskLog(taskId: string | undefined): {
  log: string;
  loaded: boolean;
} {
  const [log, setLog] = useState("");
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    if (!taskId) return;
    setLog("");
    setLoaded(false);

    let cancelled = false;
    let fetched = false;
    let pending = "";

    const handler = (event: Event) => {
      const detail = (event as CustomEvent<LogEvent>).detail;
      if (detail.taskId !== taskId) return;
      if (!fetched) {
        pending += detail.chunk;
        return;
      }
      setLog((current) => append(current, detail.chunk));
    };

    window.addEventListener("oni:log", handler);

    api
      .log(taskId)
      .then((text) => {
        if (cancelled) return;
        fetched = true;
        const merged =
          pending && !text.endsWith(pending) ? text + pending : text;
        setLog(append("", merged));
        setLoaded(true);
      })
      .catch(() => {
        if (cancelled) return;
        fetched = true;
        setLog(append("", pending));
        setLoaded(true);
      });

    return () => {
      cancelled = true;
      window.removeEventListener("oni:log", handler);
    };
  }, [taskId]);

  return { log, loaded };
}
