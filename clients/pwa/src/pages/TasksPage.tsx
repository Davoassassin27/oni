import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../lib/api";
import { ACTIVE_STATES, type TaskState } from "../lib/types";
import { cn } from "../lib/utils";
import { PlusIcon } from "../components/icons";
import { NewTaskSheet } from "../components/NewTaskSheet";
import { TaskCard } from "../components/TaskCard";
import { Button } from "../components/ui";

type Filter = "all" | "active" | "review" | "done";

const filters: Array<{ id: Filter; label: string }> = [
  { id: "all", label: "All" },
  { id: "active", label: "Active" },
  { id: "review", label: "Review" },
  { id: "done", label: "Done" },
];

const doneStates: TaskState[] = ["merged", "discarded", "failed", "stopped"];

function matchesFilter(state: TaskState, filter: Filter): boolean {
  if (filter === "all") return true;
  if (filter === "active") return ACTIVE_STATES.includes(state);
  if (filter === "review") return state === "review";
  return doneStates.includes(state);
}

export default function TasksPage({ online }: { online: boolean | null }) {
  const [filter, setFilter] = useState<Filter>("all");
  const [sheetOpen, setSheetOpen] = useState(false);

  const tasks = useQuery({
    queryKey: ["tasks"],
    queryFn: api.tasks,
    refetchInterval: (query) =>
      query.state.data?.some((task) => ACTIVE_STATES.includes(task.state))
        ? 4_000
        : false,
  });

  const allTasks = tasks.data ?? [];
  const visible = allTasks.filter((task) => matchesFilter(task.state, filter));
  const activeCount = allTasks.filter((task) =>
    ACTIVE_STATES.includes(task.state),
  ).length;
  const reviewCount = allTasks.filter((task) => task.state === "review").length;

  const subtitle =
    activeCount > 0
      ? `${activeCount} running${reviewCount ? ` · ${reviewCount} in review` : ""}`
      : reviewCount > 0
        ? `${reviewCount} waiting for review`
        : "Dispatch work. Review what lands.";

  return (
    <div className="animate-rise">
      <header className="mb-8">
        <p className="descriptor">Agentic Development Environment</p>
        <div className="mt-3 flex items-end justify-between gap-4">
          <div>
            <h1 className="text-[28px] font-semibold leading-none tracking-[-0.03em] text-hueso md:text-[32px]">
              Tasks
            </h1>
            <p className="mt-2 text-sm text-ceniza">{subtitle}</p>
          </div>
          <div className="hidden shrink-0 md:block">
            <Button
              variant="primary"
              onClick={() => setSheetOpen(true)}
            >
              <PlusIcon className="h-4 w-4" />
              New task
            </Button>
          </div>
        </div>
      </header>

      {online === false ? (
        <div className="mb-5 flex flex-wrap items-center justify-between gap-3 rounded-lg border border-vermilion/30 bg-vermilion/5 px-4 py-3">
          <p className="text-xs leading-relaxed text-vermilion">
            Daemon unreachable. Check that <code>oni-daemon</code> is running on
            this machine.
          </p>
          <button
            type="button"
            className="font-mono text-[10px] uppercase tracking-[0.14em] text-vermilion underline underline-offset-4"
            onClick={() => void tasks.refetch()}
          >
            Retry
          </button>
        </div>
      ) : null}

      <div className="mb-5 flex gap-2 overflow-x-auto pb-1">
        {filters.map((item) => {
          const count =
            item.id === "all"
              ? allTasks.length
              : item.id === "active"
                ? activeCount
                : item.id === "review"
                  ? reviewCount
                  : allTasks.filter((task) => doneStates.includes(task.state))
                      .length;
          return (
            <button
              key={item.id}
              type="button"
              onClick={() => setFilter(item.id)}
              className={cn(
                "flex h-9 shrink-0 items-center gap-2 rounded-full border px-3.5 font-mono text-[10px] uppercase tracking-[0.12em] transition-colors",
                filter === item.id
                  ? "border-kintsugi/50 bg-kintsugi/10 text-kintsugi"
                  : "border-hueso/12 text-ceniza hover:border-hueso/25 hover:text-hueso",
              )}
            >
              {item.label}
              <span
                className={cn(
                  "rounded-sm px-1 py-0.5 text-[9px]",
                  filter === item.id
                    ? "bg-kintsugi/15 text-kintsugi"
                    : "bg-hueso/5 text-ceniza",
                )}
              >
                {count}
              </span>
            </button>
          );
        })}
      </div>

      {tasks.isLoading ? (
        <div className="space-y-3">
          {[0, 1, 2].map((index) => (
            <div
              key={index}
              className="h-[124px] animate-pulse rounded-xl border border-hueso/8 bg-raised/60 motion-reduce:animate-none"
            />
          ))}
        </div>
      ) : visible.length === 0 ? (
        <div className="flex flex-col items-center rounded-2xl border border-dashed border-hueso/15 px-6 py-14 text-center">
          <img
            src="/brand/oni-hannya.svg"
            alt=""
            className="h-24 w-24 opacity-95"
          />
          <h2 className="mt-6 text-lg font-semibold tracking-tight text-hueso">
            {filter === "all" ? "The workbench is clear" : "Nothing here"}
          </h2>
          <p className="mt-2 max-w-xs text-pretty text-sm leading-relaxed text-ceniza">
            {filter === "all"
              ? "Connect a repository, dispatch a task and let the agent work in its own worktree."
              : "No tasks match this filter right now."}
          </p>
          {filter === "all" ? (
            <Button
              variant="primary"
              className="mt-6"
              onClick={() => setSheetOpen(true)}
            >
              <PlusIcon className="h-4 w-4" />
              Dispatch a task
            </Button>
          ) : (
            <button
              type="button"
              className="mt-5 font-mono text-[10px] uppercase tracking-[0.14em] text-kintsugi underline underline-offset-4"
              onClick={() => setFilter("all")}
            >
              Clear filter
            </button>
          )}
        </div>
      ) : (
        <div className="grid gap-3 lg:grid-cols-2">
          {visible.map((task, index) => (
            <div
              key={task.id}
              className="animate-rise"
              style={{ animationDelay: `${Math.min(index, 6) * 35}ms` }}
            >
              <TaskCard task={task} />
            </div>
          ))}
        </div>
      )}

      <button
        type="button"
        onClick={() => setSheetOpen(true)}
        aria-label="New task"
        className="fab-above-nav fixed right-4 z-40 flex h-14 w-14 items-center justify-center rounded-full bg-kintsugi text-sumi transition-colors hover:bg-[#d4ad3a] md:hidden"
      >
        <PlusIcon className="h-6 w-6" />
      </button>

      <NewTaskSheet open={sheetOpen} onClose={() => setSheetOpen(false)} />
    </div>
  );
}
