import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { api } from "./api";
import type { ServerEvent, Task } from "./types";

export function useLiveEvents(): boolean | null {
  const queryClient = useQueryClient();
  const [online, setOnline] = useState<boolean | null>(null);

  useEffect(() => {
    let source: EventSource | null = null;

    const connect = () => {
      source?.close();
      setOnline(null);
      source = new EventSource(api.eventsUrl());

      source.onopen = () => {
        setOnline(true);
        void queryClient.invalidateQueries({ queryKey: ["tasks"] });
        void queryClient.invalidateQueries({ queryKey: ["projects"] });
      };

      source.onmessage = (message) => {
        let event: ServerEvent;
        try {
          event = JSON.parse(message.data) as ServerEvent;
        } catch {
          return;
        }

        if (event.type === "task") {
          queryClient.setQueryData<Task[]>(["tasks"], (current) => {
            const rest = current
              ? current.filter((task) => task.id !== event.task.id)
              : [];
            return [event.task, ...rest].sort((a, b) =>
              b.createdAt.localeCompare(a.createdAt),
            );
          });
          queryClient.setQueryData(["task", event.task.id], event.task);
        } else if (event.type === "project") {
          void queryClient.invalidateQueries({ queryKey: ["projects"] });
        } else if (event.type === "project_deleted") {
          void queryClient.invalidateQueries({ queryKey: ["projects"] });
        } else if (event.type === "log") {
          window.dispatchEvent(
            new CustomEvent("oni:log", { detail: event }),
          );
        }
      };

      source.onerror = () => {
        setOnline(false);
        // EventSource reconnects on its own. The state stays honest until then.
      };
    };

    connect();

    const onTokenChanged = () => connect();
    window.addEventListener("oni:token-changed", onTokenChanged);

    return () => {
      source?.close();
      window.removeEventListener("oni:token-changed", onTokenChanged);
    };
  }, [queryClient]);

  return online;
}
